//! Read from on-disk image

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::SeekFrom;
use std::os::unix::prelude::OsStringExt;
use std::path::PathBuf;
use std::rc::Rc;

use deku::bitvec::{BitVec, BitView, Msb0};
use deku::prelude::*;
use rustc_hash::FxHashMap;
use tracing::{error, info, instrument, trace};

use crate::compressor::{CompressionOptions, Compressor};
use crate::dir::Dir;
use crate::error::BackhandError;
use crate::filesystem::node::InnerNode;
use crate::fragment::Fragment;
use crate::inode::{Inode, InodeId, InodeInner};
use crate::kinds::{Kind, LE_V4_0};
use crate::reader::{ReadSeek, SquashFsReader, SquashfsReaderWithOffset};
use crate::{
    metadata, FilesystemReader, Node, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir,
    SquashfsFileReader, SquashfsSymlink,
};

/// 128KiB
pub const DEFAULT_BLOCK_SIZE: u32 = 0x20000;

/// 4KiB
pub const DEFAULT_PAD_LEN: u32 = 0x1000;

/// log2 of 128KiB
const DEFAULT_BLOCK_LOG: u16 = 0x11;

/// 1MiB
pub const MAX_BLOCK_SIZE: u32 = byte_unit::n_mib_bytes!(1) as u32;

/// 4KiB
pub const MIN_BLOCK_SIZE: u32 = byte_unit::n_kb_bytes(4) as u32;

/// NFS export support
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "kind.type_endian", ctx = "kind: Kind")]
pub struct Export(pub u64);

/// 32 bit user and group IDs
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "kind.type_endian", ctx = "kind: Kind")]
pub struct Id(pub u32);

impl Id {
    pub fn root() -> Vec<Id> {
        vec![Id(0)]
    }
}

/// Contains important information about the archive, including the locations of other sections
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "kind.type_endian", ctx = "kind: Kind")]
pub struct SuperBlock {
    /// Must be set to 0x73717368 ("hsqs" on disk).
    #[deku(assert_eq = "kind.magic")]
    pub magic: [u8; 4],
    /// The number of inodes stored in the archive.
    pub inode_count: u32,
    /// Last modification time of the archive. Count seconds since 00:00, Jan 1st 1970 UTC (not counting leap seconds).
    /// This is unsigned, so it expires in the year 2106 (as opposed to 2038).
    pub mod_time: u32,
    /// The size of a data block in bytes. Must be a power of two between 4096 (4k) and 1048576 (1 MiB).
    pub block_size: u32,
    /// The number of entries in the fragment table.
    pub frag_count: u32,
    /// Compressor used for data
    pub compressor: Compressor,
    /// The log2 of the block size. If the two fields do not agree, the archive is considered corrupted.
    pub block_log: u16,
    /// Bit wise OR of the flag bits
    pub flags: u16,
    /// The number of entries in the ID lookup table.
    pub id_count: u16,
    #[deku(assert_eq = "kind.version_major")]
    /// Major version of the format. Must be set to 4.
    pub version_major: u16,
    #[deku(assert_eq = "kind.version_minor")]
    /// Minor version of the format. Must be set to 0.
    pub version_minor: u16,
    /// A reference to the inode of the root directory.
    pub root_inode: u64,
    /// The number of bytes used by the archive.
    /// Because SquashFS archives must be padded to a multiple of the underlying device block size, this can be less than the actual file size.
    pub bytes_used: u64,
    pub id_table: u64,
    //TODO: add read into Squashfs
    pub xattr_table: u64,
    pub inode_table: u64,
    pub dir_table: u64,
    pub frag_table: u64,
    //TODO: add read into Squashfs
    pub export_table: u64,
}

impl SuperBlock {
    pub const NOT_SET: u64 = 0xffff_ffff_ffff_ffff;

    pub fn new(compressor: Compressor, kind: Kind) -> Self {
        Self {
            magic: kind.magic,
            inode_count: 0,
            mod_time: 0,
            block_size: DEFAULT_BLOCK_SIZE,
            frag_count: 0,
            compressor,
            block_log: DEFAULT_BLOCK_LOG,
            flags: 0,
            id_count: 0,
            version_major: kind.version_major,
            version_minor: kind.version_minor,
            root_inode: 0,
            bytes_used: 0,
            id_table: 0,
            xattr_table: Self::NOT_SET,
            inode_table: 0,
            dir_table: 0,
            frag_table: Self::NOT_SET,
            export_table: Self::NOT_SET,
        }
    }

    /// Extract size of optional compression options
    fn compression_options_size(&self) -> Option<usize> {
        if self.compressor_options_are_present() {
            let size = match self.compressor {
                Compressor::Lzma => 0,
                Compressor::Gzip => 8,
                Compressor::Lzo => 8,
                Compressor::Xz => 8,
                Compressor::Lz4 => 8,
                Compressor::Zstd => 4,
                Compressor::None => 0,
            };
            Some(size)
        } else {
            None
        }
    }

    /// flag value
    pub fn inodes_uncompressed(&self) -> bool {
        self.flags & Flags::InodesStoredUncompressed as u16 != 0
    }

    /// flag value
    pub fn data_block_stored_uncompressed(&self) -> bool {
        self.flags & Flags::DataBlockStoredUncompressed as u16 != 0
    }

    /// flag value
    pub fn fragments_stored_uncompressed(&self) -> bool {
        self.flags & Flags::FragmentsStoredUncompressed as u16 != 0
    }

    /// flag value
    pub fn fragments_are_not_used(&self) -> bool {
        self.flags & Flags::FragmentsAreNotUsed as u16 != 0
    }

    /// flag value
    pub fn fragments_are_always_generated(&self) -> bool {
        self.flags & Flags::FragmentsAreAlwaysGenerated as u16 != 0
    }

    /// flag value
    pub fn data_has_been_duplicated(&self) -> bool {
        self.flags & Flags::DataHasBeenDeduplicated as u16 != 0
    }

    /// flag value
    pub fn nfs_export_table_exists(&self) -> bool {
        self.flags & Flags::NFSExportTableExists as u16 != 0
    }

    /// flag value
    pub fn xattrs_are_stored_uncompressed(&self) -> bool {
        self.flags & Flags::XattrsAreStoredUncompressed as u16 != 0
    }

    /// flag value
    pub fn no_xattrs_in_archive(&self) -> bool {
        self.flags & Flags::NoXattrsInArchive as u16 != 0
    }

    /// flag value
    pub fn compressor_options_are_present(&self) -> bool {
        self.flags & Flags::CompressorOptionsArePresent as u16 != 0
    }
}

#[rustfmt::skip]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub(crate) enum Flags {
    InodesStoredUncompressed    = 0b0000_0000_0000_0001,
    DataBlockStoredUncompressed = 0b0000_0000_0000_0010,
    Unused                      = 0b0000_0000_0000_0100,
    FragmentsStoredUncompressed = 0b0000_0000_0000_1000,
    FragmentsAreNotUsed         = 0b0000_0000_0001_0000,
    FragmentsAreAlwaysGenerated = 0b0000_0000_0010_0000,
    DataHasBeenDeduplicated     = 0b0000_0000_0100_0000,
    NFSExportTableExists        = 0b0000_0000_1000_0000,
    XattrsAreStoredUncompressed = 0b0000_0001_0000_0000,
    NoXattrsInArchive           = 0b0000_0010_0000_0000,
    CompressorOptionsArePresent = 0b0000_0100_0000_0000,
}

#[derive(Default, Clone, Debug)]
pub(crate) struct Cache {
    /// The first time a fragment bytes is read, those bytes are added to this map with the key
    /// representing the start position
    pub(crate) fragment_cache: FxHashMap<u64, Vec<u8>>,
}

/// Squashfs Image initial read information
///
/// See [`FilesystemReader`] for a representation with the data extracted and uncompressed.
pub struct Squashfs<R: ReadSeek> {
    pub kind: Kind,
    pub superblock: SuperBlock,
    /// Compression options that are used for the Compressor located after the Superblock
    pub compression_options: Option<CompressionOptions>,
    // All Inodes
    pub inodes: FxHashMap<u32, Inode>,
    /// Root Inode
    pub root_inode: Inode,
    /// Bytes containing Directory Table
    pub dir_blocks: Vec<(u64, Vec<u8>)>,
    /// Fragments Lookup Table
    pub fragments: Option<Vec<Fragment>>,
    /// Export Lookup Table
    pub export: Option<Vec<Export>>,
    /// Id Lookup Table
    pub id: Vec<Id>,
    //file reader
    file: R,
}

impl<R: ReadSeek> Squashfs<R> {
    /// Create `Squashfs` from `Read`er, with the resulting squashfs having read all fields needed
    /// to regenerate the original squashfs and interact with the fs in memory without needing to
    /// read again from `Read`er. `reader` needs to start with the beginning of the Image.
    pub fn from_reader(reader: R) -> Result<Squashfs<R>, BackhandError> {
        Self::inner_from_reader(reader)
    }
}

impl<R: ReadSeek> Squashfs<SquashfsReaderWithOffset<R>> {
    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before Reading
    ///
    /// Uses default [`Kind`]: [`LE_V4_0`]
    pub fn from_reader_with_offset(
        reader: R,
        offset: u64,
    ) -> Result<Squashfs<SquashfsReaderWithOffset<R>>, BackhandError> {
        Self::inner_from_reader(SquashfsReaderWithOffset::new(reader, offset)?)
    }

    /// Same as [`Self::from_reader_with_offset`], but including custom `kind`
    pub fn from_reader_with_offset_and_kind(
        reader: R,
        offset: u64,
        kind: Kind,
    ) -> Result<Squashfs<SquashfsReaderWithOffset<R>>, BackhandError> {
        Self::inner_from_reader_with_kind(SquashfsReaderWithOffset::new(reader, offset)?, kind)
    }
}

impl<R: ReadSeek> Squashfs<R> {
    fn inner_from_reader(reader: R) -> Result<Squashfs<R>, BackhandError> {
        Self::inner_from_reader_with_kind(reader, LE_V4_0)
    }

    fn inner_from_reader_with_kind(
        mut reader: R,
        kind: Kind,
    ) -> Result<Squashfs<R>, BackhandError> {
        reader.rewind()?;

        // Size of metadata + optional compression options metadata block
        let mut superblock = [0u8; 96];
        reader.read_exact(&mut superblock)?;

        // Parse SuperBlock
        let bs = superblock.view_bits::<deku::bitvec::Msb0>();
        let (_, superblock) = SuperBlock::read(bs, kind)?;
        info!("{superblock:#08x?}");

        let power_of_two = superblock.block_size != 0
            && (superblock.block_size & (superblock.block_size - 1)) == 0;
        if (superblock.block_size > MAX_BLOCK_SIZE)
            || (superblock.block_size < MIN_BLOCK_SIZE)
            || !power_of_two
        {
            error!("block_size({:#02x}) invalid", superblock.block_size);
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        if (superblock.block_size as f32).log2() != superblock.block_log as f32 {
            error!("block size.log2() != block_log");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Parse Compression Options, if any
        info!("Reading Compression options");
        let compression_options = if superblock.compressor != Compressor::None {
            match superblock.compression_options_size() {
                Some(size) => {
                    let bytes = metadata::read_block(&mut reader, &superblock, kind)?;

                    // Some firmware (such as openwrt) that uses XZ compression has an extra 4 bytes.
                    // squashfs-tools/unsquashfs complains about this also
                    if bytes.len() != size {
                        tracing::warn!(
                            "Non standard compression options! CompressionOptions might be incorrect: {:02x?}",
                            bytes
                        );
                    }
                    // data -> compression options
                    let bv = BitVec::from_slice(&bytes);
                    match CompressionOptions::read(
                        &bv,
                        (deku::ctx::Endian::Little, superblock.compressor),
                    ) {
                        Ok(co) => Some(co.1),
                        Err(e) => {
                            error!("invalid compression options: {e:?}[{bytes:02x?}], not using");
                            None
                        },
                    }
                },
                None => None,
            }
        } else {
            None
        };
        info!("compression_options: {compression_options:02x?}");

        // Check if legal image
        let total_length = reader.seek(SeekFrom::End(0))?;
        reader.rewind()?;
        if superblock.bytes_used > total_length {
            error!("corrupted or invalid bytes_used");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // check required fields
        if superblock.id_table > total_length {
            error!("corrupted or invalid xattr_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.inode_table > total_length {
            error!("corrupted or invalid inode_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.dir_table > total_length {
            error!("corrupted or invalid dir_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // check optional fields
        if superblock.xattr_table != SuperBlock::NOT_SET && superblock.xattr_table > total_length {
            error!("corrupted or invalid frag_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.frag_table != SuperBlock::NOT_SET && superblock.frag_table > total_length {
            error!("corrupted or invalid frag_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.export_table != SuperBlock::NOT_SET && superblock.export_table > total_length
        {
            error!("corrupted or invalid export_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Inodes");
        let inodes = reader.inodes(&superblock, kind)?;

        info!("Reading Root Inode");
        let root_inode = reader.root_inode(&superblock, kind)?;

        info!("Reading Fragments");
        let fragments = reader.fragments(&superblock, kind)?;
        let fragment_ptr = fragments.clone().map(|a| a.0);
        let fragment_table = fragments.map(|a| a.1);

        info!("Reading Exports");
        let export = reader.export(&superblock, kind)?;
        let export_ptr = export.clone().map(|a| a.0);
        let export_table = export.map(|a| a.1);

        info!("Reading Ids");
        let id = reader.id(&superblock, kind)?;
        let id_ptr = id.0;
        let id_table = id.1;

        let last_dir_position = if let Some(fragment_ptr) = fragment_ptr {
            trace!("using fragment for end of dir");
            fragment_ptr
        } else if let Some(export_ptr) = export_ptr {
            trace!("using export for end of dir");
            export_ptr
        } else {
            trace!("using id for end of dir");
            id_ptr
        };

        info!("Reading Dirs");
        let dir_blocks = reader.dir_blocks(&superblock, last_dir_position, kind)?;

        let squashfs = Squashfs {
            kind,
            superblock,
            compression_options,
            inodes,
            root_inode,
            dir_blocks,
            fragments: fragment_table,
            export: export_table,
            id: id_table,
            file: reader,
        };

        // show info about flags
        if superblock.inodes_uncompressed() {
            info!("flag: inodes uncompressed");
        }

        if superblock.data_block_stored_uncompressed() {
            info!("flag: data blocks stored uncompressed");
        }

        if superblock.fragments_stored_uncompressed() {
            info!("flag: fragments stored uncompressed");
        }

        if superblock.fragments_are_not_used() {
            info!("flag: fragments are not used");
        }

        if superblock.fragments_are_always_generated() {
            info!("flag: fragments are always generated");
        }

        if superblock.data_has_been_duplicated() {
            info!("flag: data has been duplicated");
        }

        if superblock.nfs_export_table_exists() {
            info!("flag: nfs export table exists");
        }

        if superblock.xattrs_are_stored_uncompressed() {
            info!("flag: xattrs are stored uncompressed");
        }

        if superblock.compressor_options_are_present() {
            info!("flag: compressor options are present");
        }

        info!("Successful Read");
        Ok(squashfs)
    }

    /// # Returns
    /// - `Ok(Some(Vec<Dir>))` when found dir
    /// - `Ok(None)`           when empty dir
    #[instrument(skip_all)]
    pub(crate) fn dir_from_index(
        &self,
        block_index: u64,
        file_size: u32,
        block_offset: usize,
    ) -> Result<Option<Vec<Dir>>, BackhandError> {
        trace!("- block index : {:02x?}", block_index);
        trace!("- file_size   : {:02x?}", file_size);
        trace!("- block offset: {:02x?}", block_offset);
        if file_size < 4 {
            return Ok(None);
        }

        // ignore blocks before our block_index, grab all the rest of the bytes
        // TODO: perf
        let block: Vec<u8> = self
            .dir_blocks
            .iter()
            .filter(|(a, _)| *a >= block_index)
            .flat_map(|(_, b)| b.iter())
            .copied()
            .collect();

        let bytes = &block[block_offset..][..file_size as usize - 3];
        let mut dirs = vec![];
        let mut all_bytes = bytes.view_bits::<Msb0>();
        // Read until we fail to turn bytes into `T`
        while let Ok((rest, t)) = Dir::read(all_bytes, self.kind) {
            dirs.push(t);
            all_bytes = rest;
        }

        trace!("finish");
        Ok(Some(dirs))
    }

    /// Convert into [`FilesystemReader`] by extracting all file bytes and converting into a filesystem
    /// like structure in-memory
    #[instrument(skip_all)]
    pub fn into_filesystem_reader(self) -> Result<FilesystemReader<R>, BackhandError> {
        let mut root = Node::new_root(self.root_inode.header.into());
        self.extract_dir(
            &mut PathBuf::from("/"),
            root.mut_inner_nodes().unwrap(),
            &self.root_inode,
        )?;

        let filesystem = FilesystemReader {
            kind: self.kind,
            block_size: self.superblock.block_size,
            block_log: self.superblock.block_log,
            compressor: self.superblock.compressor,
            compression_options: self.compression_options,
            mod_time: self.superblock.mod_time,
            id_table: self.id.clone(),
            fragments: self.fragments,
            root,
            reader: RefCell::new(self.file),
            cache: RefCell::new(Cache::default()),
        };
        Ok(filesystem)
    }

    #[instrument(skip_all)]
    fn extract_dir(
        &self,
        fullpath: &mut PathBuf,
        nodes: &mut HashMap<Rc<OsStr>, Node<SquashfsFileReader>>,
        dir_inode: &Inode,
    ) -> Result<(), BackhandError> {
        let dirs = match &dir_inode.inner {
            InodeInner::BasicDirectory(basic_dir) => {
                trace!("BASIC_DIR inodes: {:02x?}", basic_dir);
                self.dir_from_index(
                    basic_dir.block_index as u64,
                    basic_dir.file_size as u32,
                    basic_dir.block_offset as usize,
                )?
            },
            InodeInner::ExtendedDirectory(ext_dir) => {
                trace!("EXT_DIR: {:#02x?}", ext_dir);
                self.dir_from_index(
                    ext_dir.block_index as u64,
                    ext_dir.file_size,
                    ext_dir.block_offset as usize,
                )?
            },
            _ => return Err(BackhandError::UnexpectedInode(dir_inode.inner.clone())),
        };
        if let Some(dirs) = dirs {
            trace!("extracing dir: {dirs:#?}");
            for d in &dirs {
                trace!("extracing entry: {:#?}", d.dir_entries);
                for entry in &d.dir_entries {
                    let inode_key = (d.inode_num as i32 + entry.inode_offset as i32) as u32;
                    trace!("extracing inode: {inode_key}");
                    let found_inode = &self.inodes[&inode_key];
                    trace!("extracing inode: {found_inode:?}");
                    let header = found_inode.header;
                    let new_path = entry.name();
                    fullpath.push(&new_path);

                    match entry.t {
                        // BasicDirectory, ExtendedDirectory
                        InodeId::BasicDirectory | InodeId::ExtendedDirectory => {
                            let path = Rc::from(new_path.as_os_str());
                            let mut children = HashMap::new();
                            // its a dir, extract all children inodes
                            self.extract_dir(fullpath, &mut children, found_inode)?;

                            let inner = InnerNode::Dir(SquashfsDir { children });
                            let node =
                                Node::new(fullpath.clone(), Rc::clone(&path), header.into(), inner);
                            if let Some(_dup_file) = nodes.insert(Rc::clone(&path), node) {
                                return Err(BackhandError::DuplicatedFileName);
                            }
                        },
                        // BasicFile
                        InodeId::BasicFile => {
                            trace!("before_file: {:#02x?}", entry);
                            let path = Rc::from(new_path.as_os_str());
                            let header = header.into();
                            let basic = match &found_inode.inner {
                                InodeInner::BasicFile(file) => file.clone(),
                                InodeInner::ExtendedFile(file) => file.into(),
                                _ => {
                                    return Err(BackhandError::UnexpectedInode(
                                        found_inode.inner.clone(),
                                    ))
                                },
                            };
                            let inner = InnerNode::File(SquashfsFileReader { basic });
                            let node = Node::new(fullpath.clone(), Rc::clone(&path), header, inner);
                            if let Some(_dup_file) = nodes.insert(path, node) {
                                return Err(BackhandError::DuplicatedFileName);
                            }
                        },
                        // Basic Symlink
                        InodeId::BasicSymlink => {
                            let link = self.symlink(found_inode)?;
                            let path = Rc::from(new_path.as_os_str());
                            let inner = InnerNode::Symlink(SquashfsSymlink { link });
                            let node =
                                Node::new(fullpath.clone(), Rc::clone(&path), header.into(), inner);
                            if let Some(_dup_file) = nodes.insert(path, node) {
                                return Err(BackhandError::DuplicatedFileName);
                            }
                        },
                        // Basic CharacterDevice
                        InodeId::BasicCharacterDevice => {
                            let device_number = self.char_device(found_inode)?;
                            let path = Rc::from(new_path.as_os_str());
                            let inner = InnerNode::CharacterDevice(SquashfsCharacterDevice {
                                device_number,
                            });
                            let node =
                                Node::new(fullpath.clone(), Rc::clone(&path), header.into(), inner);
                            if let Some(_dup_file) = nodes.insert(path, node) {
                                return Err(BackhandError::DuplicatedFileName);
                            }
                        },
                        // Basic CharacterDevice
                        InodeId::BasicBlockDevice => {
                            let device_number = self.block_device(found_inode)?;
                            let path = Rc::from(new_path.as_os_str());
                            let inner =
                                InnerNode::BlockDevice(SquashfsBlockDevice { device_number });
                            let node =
                                Node::new(fullpath.clone(), Rc::clone(&path), header.into(), inner);
                            if let Some(_dup_file) = nodes.insert(path, node) {
                                return Err(BackhandError::DuplicatedFileName);
                            }
                        },
                        InodeId::ExtendedFile => {
                            return Err(BackhandError::UnsupportedInode(found_inode.inner.clone()))
                        },
                    }
                    fullpath.pop();
                }
            }
        }

        Ok(())
    }

    /// Symlink Details
    ///
    /// # Returns
    /// `Ok(original, link)
    #[instrument(skip_all)]
    fn symlink(&self, inode: &Inode) -> Result<PathBuf, BackhandError> {
        if let InodeInner::BasicSymlink(basic_sym) = &inode.inner {
            let path = OsString::from_vec(basic_sym.target_path.clone());
            return Ok(PathBuf::from(path));
        }

        error!("symlink not found");
        Err(BackhandError::FileNotFound)
    }

    /// Char Device Details
    ///
    /// # Returns
    /// `Ok(dev_num)`
    #[instrument(skip_all)]
    fn char_device(&self, inode: &Inode) -> Result<u32, BackhandError> {
        if let InodeInner::BasicCharacterDevice(spc_file) = &inode.inner {
            return Ok(spc_file.device_number);
        }

        error!("char dev not found");
        Err(BackhandError::FileNotFound)
    }

    /// Block Device Details
    ///
    /// # Returns
    /// `Ok(dev_num)`
    #[instrument(skip_all)]
    fn block_device(&self, inode: &Inode) -> Result<u32, BackhandError> {
        if let InodeInner::BasicBlockDevice(spc_file) = &inode.inner {
            return Ok(spc_file.device_number);
        }

        error!("block dev not found");
        Err(BackhandError::FileNotFound)
    }
}
