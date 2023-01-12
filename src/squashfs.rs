//! Read from on-disk image

use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::io::{Cursor, Read, SeekFrom};
use std::path::{Path, PathBuf};

use deku::bitvec::BitVec;
use deku::prelude::*;
use tracing::{error, info, instrument, trace};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::data::DATA_STORED_UNCOMPRESSED;
use crate::dir::{Dir, DirEntry};
use crate::error::SquashfsError;
use crate::filesystem::{
    Filesystem, InnerNode, Node, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsFile,
    SquashfsPath, SquashfsSymlink,
};
use crate::fragment::Fragment;
use crate::inode::{BasicFile, Inode, InodeHeader, InodeId, InodeInner};
use crate::metadata;
use crate::reader::{ReadSeek, SquashfsReader};

/// NFS export support
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "little")]
pub struct Export(pub u64);

/// 32 bit user and group IDs
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "little")]
pub struct Id(pub u32);

/// Contains important information about the archive, including the locations of other sections
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct SuperBlock {
    /// Must be set to 0x73717368 ("hsqs" on disk).
    #[deku(assert_eq = "*Self::MAGIC")]
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
    #[deku(assert_eq = "4")]
    /// Major version of the format. Must be set to 4.
    pub version_major: u16,
    #[deku(assert_eq = "0")]
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
    const MAGIC: &'static [u8; 4] = b"hsqs";
    const DEFAULT_BLOCK_SIZE: u32 = 0x20000;
    const DEFAULT_BLOCK_LOG: u16 = 0x11;
    const VERSION_MAJ: u16 = 4;
    const VERSION_MIN: u16 = 0;
    const NOT_SET: u64 = 0xffff_ffff_ffff_ffff;

    pub fn new(compressor: Compressor) -> Self {
        Self {
            magic: *Self::MAGIC,
            inode_count: 0,
            mod_time: 0,
            block_size: Self::DEFAULT_BLOCK_SIZE,
            frag_count: 0,
            compressor,
            block_log: Self::DEFAULT_BLOCK_LOG,
            flags: 0,
            id_count: 0,
            version_major: Self::VERSION_MAJ,
            version_minor: Self::VERSION_MIN,
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

    // Extract size of optional compression options
    pub fn compression_options_size(&self) -> Option<usize> {
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

    pub fn inodes_uncompressed(&self) -> bool {
        self.flags & Flags::InodesStoredUncompressed as u16 != 0
    }

    pub fn data_block_stored_uncompressed(&self) -> bool {
        self.flags & Flags::DataBlockStoredUncompressed as u16 != 0
    }

    pub fn fragments_stored_uncompressed(&self) -> bool {
        self.flags & Flags::FragmentsStoredUncompressed as u16 != 0
    }

    pub fn fragments_are_not_used(&self) -> bool {
        self.flags & Flags::FragmentsAreNotUsed as u16 != 0
    }

    pub fn fragments_are_always_generated(&self) -> bool {
        self.flags & Flags::FragmentsAreAlwaysGenerated as u16 != 0
    }

    pub fn data_has_been_duplicated(&self) -> bool {
        self.flags & Flags::DataHasBeenDeduplicated as u16 != 0
    }

    pub fn nfs_export_table_exists(&self) -> bool {
        self.flags & Flags::NFSExportTableExists as u16 != 0
    }

    pub fn xattrs_are_stored_uncompressed(&self) -> bool {
        self.flags & Flags::XattrsAreStoredUncompressed as u16 != 0
    }

    pub fn no_xattrs_in_archive(&self) -> bool {
        self.flags & Flags::NoXattrsInArchive as u16 != 0
    }

    pub fn compressor_options_are_present(&self) -> bool {
        self.flags & Flags::CompressorOptionsArePresent as u16 != 0
    }
}

pub enum Flags {
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

#[derive(Default)]
struct Cache {
    /// The first time a fragment bytes is read, those bytes are added to this map with the key
    /// representing the start position
    pub fragment_cache: HashMap<u64, Vec<u8>, BuildHasherDefault<twox_hash::XxHash64>>,
}

/// Representation of Squashfs image in memory. Tables such as Inodes and Dirs are extracted into
/// the struct types, while data stays compressed.
///
/// See [`Filesystem`] for a representation with the data extracted and uncompressed.
pub struct Squashfs {
    pub superblock: SuperBlock,
    /// Compression options that are used for the Compressor located after the Superblock
    pub compression_options: Option<CompressionOptions>,
    /// Section containing compressed/uncompressed file data and fragments.
    ///
    /// This also contains the superblock and option bytes b/c that is how [`BasicFile`] uses its
    /// `blocks_starts`.
    pub data_and_fragments: Vec<u8>,
    // All Inodes
    pub inodes: HashMap<u32, Inode, BuildHasherDefault<twox_hash::XxHash64>>,
    /// Root Inode
    pub root_inode: Inode,
    /// Bytes containing Directory Table, this is converted into [`Dir`]'s with [`Squashfs::all_dirs`].
    pub dir_blocks: Vec<(u64, Vec<u8>)>,
    /// Fragments Lookup Table
    pub fragments: Option<Vec<Fragment>>,
    /// Export Lookup Table
    pub export: Option<Vec<Export>>,
    /// Id Lookup Table
    pub id: Option<Vec<Id>>,
}

impl Squashfs {
    /// Create `Squashfs` from `Read`er, with the resulting squashfs having read all fields needed
    /// to regenerate the original squashfs and interact with the fs in memory without needing to
    /// read again from `Read`er. `reader` needs to start with the beginning of the Image.
    #[instrument(skip_all)]
    pub fn from_reader<R: ReadSeek + 'static>(reader: R) -> Result<Squashfs, SquashfsError> {
        Self::from_reader_with_offset(reader, 0)
    }

    /// Same as `from_reader`, but with a starting `offset` to the image in the `reader`
    pub fn from_reader_with_offset<R: ReadSeek + 'static>(
        mut reader: R,
        offset: u64,
    ) -> Result<Squashfs, SquashfsError> {
        // do the initial seek from the start of `reader`
        reader.seek(SeekFrom::Start(offset))?;

        // Size of metadata + optional compression options metadata block
        let mut superblock = [0u8; 96];
        reader.read_exact(&mut superblock)?;

        // Parse SuperBlock
        let (_, superblock) = SuperBlock::from_bytes((&superblock, 0))?;
        info!("{superblock:#08x?}");

        // Parse Compression Options, if any
        info!("Reading Compression options");
        let compression_options = if superblock.compressor != Compressor::None {
            if let Some(size) = superblock.compression_options_size() {
                let bytes = metadata::read_block(&mut reader, &superblock)?;

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
                let (_, c) = CompressionOptions::read(
                    &bv,
                    (deku::ctx::Endian::Little, superblock.compressor),
                )?;
                Some(c)
            } else {
                None
            }
        } else {
            None
        };
        info!("compression_options: {compression_options:02x?}");

        // Create SquashfsReader
        let mut squashfs_reader = SquashfsReader::new(reader, offset);

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Data and Fragments");
        let data_and_fragments = squashfs_reader.data_and_fragments(&superblock)?;

        info!("Reading Inodes");
        let inodes = squashfs_reader.inodes(&superblock)?;

        info!("Reading Root Inode");
        let root_inode = squashfs_reader.root_inode(&superblock)?;

        info!("Reading Fragments");
        let fragments = squashfs_reader.fragments(&superblock)?;
        let fragment_ptr = fragments.clone().map(|a| a.0);
        let fragment_table = fragments.map(|a| a.1);

        info!("Reading Exports");
        let export = squashfs_reader.export(&superblock)?;
        let export_ptr = export.clone().map(|a| a.0);
        let export_table = export.map(|a| a.1);

        info!("Reading Ids");
        let id = squashfs_reader.id(&superblock)?;
        let id_ptr = id.clone().map(|a| a.0);
        let id_table = id.map(|a| a.1);

        let last_dir_position = if let Some(fragment_ptr) = fragment_ptr {
            trace!("using fragment for end of dir");
            fragment_ptr
        } else if let Some(export_ptr) = export_ptr {
            trace!("using export for end of dir");
            export_ptr
        } else if let Some(id_ptr) = id_ptr {
            trace!("using id for end of dir");
            id_ptr
        } else {
            unreachable!();
        };

        info!("Reading Dirs");
        let dir_blocks = squashfs_reader.dir_blocks(&superblock, last_dir_position)?;

        let squashfs = Squashfs {
            superblock,
            compression_options,
            data_and_fragments,
            inodes,
            root_inode,
            dir_blocks,
            fragments: fragment_table,
            export: export_table,
            id: id_table,
        };

        // show info about flags
        if superblock.inodes_uncompressed() {
            info!("flags: inodes uncompressed");
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

    /// Convert `self.dir_blocks` into `Vec<Dir>`
    pub fn all_dirs(&self) -> Result<Vec<Dir>, SquashfsError> {
        let bytes: Vec<u8> = self
            .dir_blocks
            .iter()
            .flat_map(|(_, b)| b.clone())
            .collect();

        let mut dirs = vec![];
        let mut rest = bytes;
        while !rest.is_empty() {
            let ((r, _), dir) = Dir::from_bytes((&rest, 0))?;
            rest = r.to_vec();
            dirs.push(dir);
        }

        Ok(dirs)
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
    ) -> Result<Option<Vec<Dir>>, SquashfsError> {
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
            .filter(|(a, _)| a >= &block_index)
            .map(|(_, b)| b.clone())
            .collect::<Vec<Vec<u8>>>()
            .iter()
            .flatten()
            .copied()
            .collect();

        // Parse into Dirs
        let mut bytes = &block[block_offset..][..file_size as usize - 3];
        let mut dirs = vec![];
        while !bytes.is_empty() {
            let (rest, dir) = Dir::from_bytes((bytes, 0))?;
            bytes = rest.0;
            dirs.push(dir);
        }

        Ok(Some(dirs))
    }

    /// Convert into [`Filesystem`] by extracting all file bytes and converting into a filesystem
    /// like structure in-memory
    #[instrument(skip_all)]
    pub fn into_filesystem(&self) -> Result<Filesystem, SquashfsError> {
        let mut cache = Cache::default();
        let mut nodes = Vec::with_capacity(self.superblock.inode_count as usize);
        let path: PathBuf = "/".into();

        self.extract_dir(&mut cache, &mut nodes, &self.root_inode, &path)?;

        let root_inode = SquashfsPath {
            header: self.root_inode.header.into(),
        };

        let filesystem = Filesystem {
            block_size: self.superblock.block_size,
            block_log: self.superblock.block_log,
            compressor: self.superblock.compressor,
            compression_options: self.compression_options,
            mod_time: self.superblock.mod_time,
            id_table: self.id.clone(),
            root_inode,
            nodes: nodes.to_vec(),
        };
        Ok(filesystem)
    }

    #[instrument(skip_all)]
    fn extract_dir(
        &self,
        cache: &mut Cache,
        nodes: &mut Vec<Node>,
        dir_inode: &Inode,
        path: &Path,
    ) -> Result<(), SquashfsError> {
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
            _ => panic!(),
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
                    let mut new_path = path.to_path_buf();
                    new_path.push(entry.name());

                    match entry.t {
                        // BasicDirectory, ExtendedDirectory
                        InodeId::BasicDirectory | InodeId::ExtendedDirectory => {
                            let path = new_path.clone();
                            let inner = InnerNode::Path(SquashfsPath {
                                header: header.into(),
                            });
                            let node = Node::new(path, inner);
                            nodes.push(node);

                            // its a dir, extract all inodes
                            self.extract_dir(cache, nodes, found_inode, &new_path)?;
                        },
                        // BasicFile
                        InodeId::BasicFile => {
                            trace!("before_file: {:#02x?}", entry);
                            let (file_header, bytes) = self.file(cache, found_inode)?;
                            let path = new_path.clone();
                            let inner = InnerNode::File(SquashfsFile {
                                header: file_header.into(),
                                bytes,
                            });
                            let node = Node::new(path, inner);
                            nodes.push(node);
                        },
                        // Basic Symlink
                        InodeId::BasicSymlink => {
                            let (original, link) = self.symlink(found_inode, entry)?;
                            let path = new_path;
                            let inner = InnerNode::Symlink(SquashfsSymlink {
                                header: header.into(),
                                original,
                                link,
                            });
                            let node = Node::new(path, inner);
                            nodes.push(node);
                        },
                        // Basic CharacterDevice
                        InodeId::BasicCharacterDevice => {
                            let device_number = self.char_device(found_inode)?;
                            let path = new_path;
                            let inner = InnerNode::CharacterDevice(SquashfsCharacterDevice {
                                header: header.into(),
                                device_number,
                            });
                            let node = Node::new(path, inner);
                            nodes.push(node);
                        },
                        // Basic CharacterDevice
                        InodeId::BasicBlockDevice => {
                            let device_number = self.block_device(found_inode)?;
                            let path = new_path;
                            let inner = InnerNode::BlockDevice(SquashfsBlockDevice {
                                header: header.into(),
                                device_number,
                            });
                            let node = Node::new(path, inner);
                            nodes.push(node);
                        },
                        _ => panic!("{entry:?}"),
                    }
                }
            }
        }

        Ok(())
    }

    // From `basic_file`, extract total data including from data blocks and data fragments
    #[instrument(skip_all)]
    fn data(&self, cache: &mut Cache, basic_file: &BasicFile) -> Result<Vec<u8>, SquashfsError> {
        trace!("extracting: {:#02x?}", basic_file);

        // Add data
        trace!("extracting data @ offset {:02x?}", basic_file.blocks_start);

        let mut data_bytes = Vec::with_capacity(basic_file.file_size as usize);

        // Extract Data
        if !basic_file.block_sizes.is_empty() {
            let mut reader =
                Cursor::new(&self.data_and_fragments[basic_file.blocks_start as usize..]);
            for block_size in &basic_file.block_sizes {
                let mut bytes = self.read_data(&mut reader, *block_size as usize)?;
                data_bytes.append(&mut bytes);
            }
        }

        trace!("data bytes: {:02x?}", data_bytes.len());

        // Extract Fragment
        // TODO: this should be constant
        if basic_file.frag_index != 0xffffffff {
            if let Some(fragments) = &self.fragments {
                let frag = fragments[basic_file.frag_index as usize];

                // use fragment cache if possible
                match cache.fragment_cache.get(&(frag.start)) {
                    Some(cache_bytes) => {
                        let bytes = &cache_bytes.clone();
                        let bytes = &bytes[basic_file.block_offset as usize..];
                        data_bytes.append(&mut bytes.to_vec());
                    },
                    None => {
                        let mut reader =
                            Cursor::new(&self.data_and_fragments[frag.start as usize..]);
                        let mut bytes = self.read_data(&mut reader, frag.size as usize)?;
                        cache.fragment_cache.insert(frag.start, bytes.clone());
                        bytes = bytes[basic_file.block_offset as usize..].to_vec();
                        data_bytes.append(&mut bytes);
                    },
                }
            }
        }

        data_bytes = data_bytes[..basic_file.file_size as usize].to_vec();
        Ok(data_bytes)
    }

    /// Read from either Data blocks or Fragments blocks
    fn read_data<R: Read>(&self, reader: &mut R, size: usize) -> Result<Vec<u8>, SquashfsError> {
        let uncompressed = size & (DATA_STORED_UNCOMPRESSED as usize) != 0;
        let size = size & !(DATA_STORED_UNCOMPRESSED as usize);
        let mut buf = vec![0u8; size];
        reader.read_exact(&mut buf)?;

        let bytes = if uncompressed {
            buf
        } else {
            compressor::decompress(buf, self.superblock.compressor)?
        };
        Ok(bytes)
    }

    /// Symlink Details
    ///
    /// # Returns
    /// `Ok(original, link)
    #[instrument(skip_all)]
    fn symlink(&self, inode: &Inode, entry: &DirEntry) -> Result<(String, String), SquashfsError> {
        if let InodeInner::BasicSymlink(basic_sym) = &inode.inner {
            return Ok((
                String::from_utf8(entry.name.clone())?,
                String::from_utf8(basic_sym.target_path.clone())?,
            ));
        }

        error!("symlink not found");
        Err(SquashfsError::FileNotFound)
    }

    /// Char Device Details
    ///
    /// # Returns
    /// `Ok(dev_num)`
    #[instrument(skip_all)]
    fn char_device(&self, inode: &Inode) -> Result<u32, SquashfsError> {
        if let InodeInner::BasicCharacterDevice(spc_file) = &inode.inner {
            return Ok(spc_file.device_number);
        }

        error!("char dev not found");
        Err(SquashfsError::FileNotFound)
    }

    /// Block Device Details
    ///
    /// # Returns
    /// `Ok(dev_num)`
    #[instrument(skip_all)]
    fn block_device(&self, inode: &Inode) -> Result<u32, SquashfsError> {
        if let InodeInner::BasicBlockDevice(spc_file) = &inode.inner {
            return Ok(spc_file.device_number);
        }

        error!("block dev not found");
        Err(SquashfsError::FileNotFound)
    }

    /// From file details, extract (PathBuf, FileBytes)
    #[instrument(skip_all)]
    fn file(
        &self,
        cache: &mut Cache,
        inode: &Inode,
    ) -> Result<(InodeHeader, Vec<u8>), SquashfsError> {
        // look through basic file inodes in search of the one true basic_inode and extract the
        // bytes from the data and fragment sections
        match &inode.inner {
            InodeInner::BasicFile(basic_file) => {
                return Ok((inode.header, self.data(cache, basic_file)?));
            },
            InodeInner::ExtendedFile(ext_file) => {
                let basic_file = BasicFile::from(ext_file);
                return Ok((inode.header, self.data(cache, &basic_file)?));
            },
            _ => (),
        }

        error!("file not found");
        Err(SquashfsError::FileNotFound)
    }
}
