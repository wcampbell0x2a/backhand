//! Read from on-disk image

use std::cell::RefCell;
use std::ffi::OsString;
use std::io::SeekFrom;
use std::os::unix::prelude::OsStringExt;
use std::path::{Path, PathBuf};

use deku::bitvec::{BitVec, BitView, Msb0};
use deku::prelude::*;
use rustc_hash::FxHashMap;
use tracing::{error, info, instrument, trace};

use crate::compressor::{CompressionOptions, Compressor};
use crate::dir::Dir;
use crate::error::SquashfsError;
use crate::fragment::Fragment;
use crate::inode::{Inode, InodeId, InodeInner};
use crate::reader::{ReadSeek, SquashFsReader, SquashfsReaderWithOffset};
use crate::{
    metadata, FilesystemReader, InnerNode, Node, SquashfsBlockDevice, SquashfsCharacterDevice,
    SquashfsDir, SquashfsFileReader, SquashfsSymlink,
};

/// Kind Magic
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Magic {
    Little,
    Big,
}

impl Magic {
    fn magic(self) -> [u8; 4] {
        match self {
            Self::Little => *b"hsqs",
            Self::Big => *b"sqsh",
        }
    }
}

/// Kind Endian
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

/// Version of SquashFS, also supporting custom changes to SquashFS seen in 3rd-party firmware
///
/// See [Kind Constants](`crate::kind#constants`) for a list of custom Kinds
///
/// TODO: we probably want a `from_reader` for this, so they can get a `Kind` from the magic bytes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Kind {
    /// Magic at the beginning of the image
    pub(crate) magic: [u8; 4],
    /// Endian used for all data types
    pub(crate) type_endian: deku::ctx::Endian,
    /// Endian used for Metadata Lengths
    pub(crate) data_endian: deku::ctx::Endian,
    /// Major version
    pub(crate) version_major: u16,
    /// Minor version
    pub(crate) version_minor: u16,
}

impl Kind {
    /// Create with default Kind: [`LE_V4_0`]
    pub fn new() -> Self {
        LE_V4_0
    }

    /// Set magic type at the beginning of the image
    pub fn with_magic(mut self, magic: Magic) -> Self {
        self.magic = magic.magic();
        self
    }

    /// Set endian used for data types
    pub fn with_type_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                self.type_endian = deku::ctx::Endian::Little;
            },
            Endian::Big => {
                self.type_endian = deku::ctx::Endian::Big;
            },
        }
        self
    }

    /// Set endian used for Metadata lengths
    pub fn with_data_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                self.data_endian = deku::ctx::Endian::Little;
            },
            Endian::Big => {
                self.data_endian = deku::ctx::Endian::Big;
            },
        }
        self
    }

    /// Set both type and data endian
    pub fn with_all_endian(mut self, endian: Endian) -> Self {
        match endian {
            Endian::Little => {
                self.type_endian = deku::ctx::Endian::Little;
                self.data_endian = deku::ctx::Endian::Little;
            },
            Endian::Big => {
                self.type_endian = deku::ctx::Endian::Big;
                self.data_endian = deku::ctx::Endian::Big;
            },
        }
        self
    }

    /// Set major and minor version
    pub fn with_version(mut self, major: u16, minor: u16) -> Self {
        self.version_major = major;
        self.version_minor = minor;
        self
    }
}

impl Default for Kind {
    fn default() -> Self {
        Self::new()
    }
}

/// Default `Kind` for linux kernel and squashfs-tools/mksquashfs. Little-Endian v4.0
pub const LE_V4_0: Kind = Kind {
    magic: *b"hsqs",
    type_endian: deku::ctx::Endian::Little,
    data_endian: deku::ctx::Endian::Little,
    version_major: 4,
    version_minor: 0,
};

/// Big-Endian Superblock v4.0
pub const BE_V4_0: Kind = Kind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Big,
    version_major: 4,
    version_minor: 0,
};

/// AVM Fritz!OS firmware support. Tested with: <https://github.com/dnicolodi/squashfs-avm-tools>
pub const AVM_BE_V4_0: Kind = Kind {
    magic: *b"sqsh",
    type_endian: deku::ctx::Endian::Big,
    data_endian: deku::ctx::Endian::Little,
    version_major: 4,
    version_minor: 0,
};

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
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
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
    const DEFAULT_BLOCK_LOG: u16 = 0x11;
    const DEFAULT_BLOCK_SIZE: u32 = 0x20000;
    pub const NOT_SET: u64 = 0xffff_ffff_ffff_ffff;
    const VERSION_MAJ: u16 = 4;
    const VERSION_MIN: u16 = 0;

    pub fn new(compressor: Compressor, kind: Kind) -> Self {
        Self {
            magic: kind.magic,
            inode_count: 0,
            mod_time: 0,
            block_size: Self::DEFAULT_BLOCK_SIZE,
            frag_count: 0,
            compressor,
            block_log: Self::DEFAULT_BLOCK_LOG,
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
    /// Section containing compressed/uncompressed file data and fragments.
    ///
    /// This also contains the superblock and option bytes for file offset reasons.
    pub data_and_fragments: Vec<u8>,
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
    pub fn from_reader(reader: R) -> Result<Squashfs<R>, SquashfsError> {
        Self::inner_from_reader(reader)
    }
}

impl<R: ReadSeek> Squashfs<SquashfsReaderWithOffset<R>> {
    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before Reading
    ///
    /// Uses default [`Kind`]: [`LITTLE_ENDIAN_V4_0`]
    pub fn from_reader_with_offset(
        reader: R,
        offset: u64,
    ) -> Result<Squashfs<SquashfsReaderWithOffset<R>>, SquashfsError> {
        Self::inner_from_reader(SquashfsReaderWithOffset::new(reader, offset)?)
    }

    /// Same as [`Self::from_reader_with_offset`], but including custom `kind`
    pub fn from_reader_with_offset_and_kind(
        reader: R,
        offset: u64,
        kind: Kind,
    ) -> Result<Squashfs<SquashfsReaderWithOffset<R>>, SquashfsError> {
        Self::inner_from_reader_with_kind(SquashfsReaderWithOffset::new(reader, offset)?, kind)
    }
}

impl<R: ReadSeek> Squashfs<R> {
    fn inner_from_reader(reader: R) -> Result<Squashfs<R>, SquashfsError> {
        Self::inner_from_reader_with_kind(reader, LE_V4_0)
    }

    fn inner_from_reader_with_kind(
        mut reader: R,
        kind: Kind,
    ) -> Result<Squashfs<R>, SquashfsError> {
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
        if (superblock.block_size > byte_unit::n_mib_bytes!(1) as u32)
            || (superblock.block_size < byte_unit::n_kb_bytes(4) as u32)
            || !power_of_two
        {
            error!("block_size({:#02x}) invalid", superblock.block_size);
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
        }

        if (superblock.block_size as f32).log2() != superblock.block_log as f32 {
            error!("block size.log2() != block_log");
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
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
                }
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
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
        }

        // check required fields
        if superblock.id_table > total_length {
            error!("corrupted or invalid xattr_table");
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
        }
        if superblock.inode_table > total_length {
            error!("corrupted or invalid inode_table");
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
        }
        if superblock.dir_table > total_length {
            error!("corrupted or invalid dir_table");
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
        }

        // check optional fields
        if superblock.xattr_table != SuperBlock::NOT_SET && superblock.xattr_table > total_length {
            error!("corrupted or invalid frag_table");
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
        }
        if superblock.frag_table != SuperBlock::NOT_SET && superblock.frag_table > total_length {
            error!("corrupted or invalid frag_table");
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
        }
        if superblock.export_table != SuperBlock::NOT_SET && superblock.export_table > total_length
        {
            error!("corrupted or invalid export_table");
            return Err(SquashfsError::CorruptedOrInvalidSquashfs);
        }

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Data and Fragments");
        let data_and_fragments = reader.data_and_fragments(&superblock)?;

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
            data_and_fragments,
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
    pub fn into_filesystem_reader(self) -> Result<FilesystemReader<R>, SquashfsError> {
        let mut nodes = Vec::with_capacity(self.superblock.inode_count as usize);
        let path: PathBuf = "/".into();

        self.extract_dir(&mut nodes, &self.root_inode, &path)?;

        let root_inode = SquashfsDir {
            header: self.root_inode.header.into(),
        };

        let filesystem = FilesystemReader {
            kind: self.kind,
            block_size: self.superblock.block_size,
            block_log: self.superblock.block_log,
            compressor: self.superblock.compressor,
            compression_options: self.compression_options,
            mod_time: self.superblock.mod_time,
            id_table: self.id.clone(),
            fragments: self.fragments,
            root_inode,
            nodes,
            reader: RefCell::new(self.file),
            cache: RefCell::new(Cache::default()),
        };
        Ok(filesystem)
    }

    #[instrument(skip_all)]
    fn extract_dir(
        &self,
        nodes: &mut Vec<Node<SquashfsFileReader>>,
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
            _ => return Err(SquashfsError::UnexpectedInode(dir_inode.inner.clone())),
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
                            let inner = InnerNode::Dir(SquashfsDir {
                                header: header.into(),
                            });
                            let node = Node::new(path, inner);
                            nodes.push(node);

                            // its a dir, extract all inodes
                            self.extract_dir(nodes, found_inode, &new_path)?;
                        },
                        // BasicFile
                        InodeId::BasicFile => {
                            trace!("before_file: {:#02x?}", entry);
                            let path = new_path.clone();
                            let header = header.into();
                            let basic = match &found_inode.inner {
                                InodeInner::BasicFile(file) => file.clone(),
                                InodeInner::ExtendedFile(file) => file.into(),
                                _ => {
                                    return Err(SquashfsError::UnexpectedInode(
                                        found_inode.inner.clone(),
                                    ))
                                },
                            };
                            let inner = InnerNode::File(SquashfsFileReader { header, basic });
                            let node = Node::new(path, inner);
                            nodes.push(node);
                        },
                        // Basic Symlink
                        InodeId::BasicSymlink => {
                            let link = self.symlink(found_inode)?;
                            let path = new_path;
                            let inner = InnerNode::Symlink(SquashfsSymlink {
                                header: header.into(),
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
                        InodeId::ExtendedFile => {
                            return Err(SquashfsError::UnsupportedInode(found_inode.inner.clone()))
                        },
                    }
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
    fn symlink(&self, inode: &Inode) -> Result<PathBuf, SquashfsError> {
        if let InodeInner::BasicSymlink(basic_sym) = &inode.inner {
            let path = OsString::from_vec(basic_sym.target_path.clone());
            return Ok(PathBuf::from(path));
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
}
