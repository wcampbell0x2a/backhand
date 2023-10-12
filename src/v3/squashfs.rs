//! Read from on-disk image

use std::ffi::OsString;
use std::io::{Cursor, Seek, SeekFrom};
use std::os::unix::prelude::OsStringExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use deku::prelude::*;
use rustc_hash::FxHashMap;
use tracing::{error, info, instrument, trace};

use crate::bufread::BufReadSeek;
use crate::compressor::{CompressionOptions, Compressor};
use crate::error::BackhandError;
use crate::flags::Flags;
use crate::kinds::{Kind, LE_V4_0};
use crate::v3::dir::{Dir, DirInodeId};
use crate::v3::filesystem::node::{InnerNode, Nodes};
use crate::v3::fragment::Fragment;
use crate::v3::inode::{Inode, InodeId, InodeInner};
use crate::v3::reader::{SquashFsReader, SquashfsReaderWithOffset};
use crate::v3::{
    Export, FilesystemReader, Id, Node, NodeHeader, SquashfsBlockDevice, SquashfsCharacterDevice,
    SquashfsDir, SquashfsFileReader, SquashfsSymlink,
};

/// 128KiB
pub const DEFAULT_BLOCK_SIZE: u32 = 0x20000;

/// 4KiB
pub const DEFAULT_PAD_LEN: u32 = 0x1000;

/// log2 of 128KiB
const DEFAULT_BLOCK_LOG: u16 = 0x11;

/// 1MiB
pub const MAX_BLOCK_SIZE: u32 = byte_unit::n_mib_bytes(1) as u32;

/// 4KiB
pub const MIN_BLOCK_SIZE: u32 = byte_unit::n_kb_bytes(4) as u32;

/// Contains important information about the archive, including the locations of other sections
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(
    endian = "ctx_type_endian",
    ctx = "ctx_magic: [u8; 4], ctx_version_major: u16, ctx_version_minor: u16, ctx_type_endian: deku::ctx::Endian"
)]
pub struct SuperBlock {
    #[deku(assert_eq = "ctx_magic")]
    pub magic: [u8; 4],
    pub inode_count: u32,
    pub bytes_used_2: u32,
    pub uid_start_2: u32,
    pub guid_start_2: u32,
    pub inode_table_start_2: u32,
    pub directory_table_start_2: u32,
    pub s_major: u16,
    pub s_minor: u16,
    pub block_size_1: u16,
    pub block_log: u16,
    pub flags: u8,
    pub no_uids: u8,
    pub no_guids: u8,
    pub mkfs_time: u32,
    pub root_inode: u64,
    pub block_size: u32,
    pub fragments: u32,
    pub fragment_table_start_2: u32,
    pub bytes_used: u64,
    pub uid_start: u64,
    pub guid_start: u64,
    pub inode_table_start: u64,
    pub directory_table_start: u64,
    pub fragment_table_start: u64,
    pub unused: u64,
}

pub const NOT_SET: u64 = 0xffff_ffff_ffff_ffff;

impl SuperBlock {
    /// flag value
    pub fn inodes_uncompressed(&self) -> bool {
        u16::from(self.flags) & Flags::InodesStoredUncompressed as u16 != 0
    }

    /// flag value
    pub fn data_block_stored_uncompressed(&self) -> bool {
        u16::from(self.flags) & Flags::DataBlockStoredUncompressed as u16 != 0
    }

    /// flag value
    pub fn fragments_stored_uncompressed(&self) -> bool {
        u16::from(self.flags) & Flags::FragmentsStoredUncompressed as u16 != 0
    }

    /// flag value
    pub fn fragments_are_not_used(&self) -> bool {
        u16::from(self.flags) & Flags::FragmentsAreNotUsed as u16 != 0
    }

    /// flag value
    pub fn fragments_are_always_generated(&self) -> bool {
        u16::from(self.flags) & Flags::FragmentsAreAlwaysGenerated as u16 != 0
    }

    /// flag value
    pub fn duplicate_data_removed(&self) -> bool {
        u16::from(self.flags) & Flags::DataHasBeenDeduplicated as u16 != 0
    }

    /// flag value
    pub fn nfs_export_table_exists(&self) -> bool {
        u16::from(self.flags) & Flags::NFSExportTableExists as u16 != 0
    }
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
pub struct Squashfs<'b> {
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
    /// Id Lookup Table V4
    pub id: Option<Vec<Id>>,
    /// Uid Lookup Table V3
    pub uid: Option<Vec<u16>>,
    /// Gid Lookup Table V3
    pub guid: Option<Vec<u16>>,
    //file reader
    file: Box<dyn BufReadSeek + 'b>,
}

impl<'b> Squashfs<'b> {
    /// Read Superblock and Compression Options at current `reader` offset without parsing inodes
    /// and dirs
    ///
    /// Used for unsquashfs (extraction and --stat)
    pub fn superblock_and_compression_options(
        reader: &mut Box<dyn BufReadSeek + 'b>,
        kind: &Kind,
    ) -> Result<(SuperBlock, Option<CompressionOptions>), BackhandError> {
        // Parse SuperBlock
        let mut container = Reader::new(reader);
        let superblock = SuperBlock::from_reader_with_ctx(
            &mut container,
            (
                kind.inner.magic,
                kind.inner.version_major,
                kind.inner.version_minor,
                kind.inner.type_endian,
            ),
        )?;
        trace!("{:02x?}", superblock);

        let block_size = superblock.block_size;
        let power_of_two = block_size != 0 && (block_size & (block_size - 1)) == 0;
        if !(MIN_BLOCK_SIZE..=MAX_BLOCK_SIZE).contains(&block_size) || !power_of_two {
            error!("block_size({:#02x}) invalid", superblock.block_size);
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        if (superblock.block_size as f32).log2() != superblock.block_log as f32 {
            error!("block size.log2() != block_log");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        let compression_options = None;
        Ok((superblock, compression_options))
    }

    /// Create `Squashfs` from `Read`er, with the resulting squashfs having read all fields needed
    /// to regenerate the original squashfs and interact with the fs in memory without needing to
    /// read again from `Read`er. `reader` needs to start with the beginning of the Image.
    pub fn from_reader(reader: impl BufReadSeek + 'b) -> Result<Self, BackhandError> {
        Self::from_reader_with_offset(reader, 0)
    }

    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before Reading
    ///
    /// Uses default [`Kind`]: [`LE_V4_0`]
    pub fn from_reader_with_offset(
        reader: impl BufReadSeek + 'b,
        offset: u64,
    ) -> Result<Self, BackhandError> {
        Self::from_reader_with_offset_and_kind(
            reader,
            offset,
            Kind {
                inner: Arc::new(LE_V4_0),
            },
        )
    }

    /// Same as [`Self::from_reader_with_offset`], but including custom `kind`
    pub fn from_reader_with_offset_and_kind(
        reader: impl BufReadSeek + 'b,
        offset: u64,
        kind: Kind,
    ) -> Result<Self, BackhandError> {
        let reader: Box<dyn BufReadSeek + 'b> = if offset == 0 {
            Box::new(reader)
        } else {
            let reader = SquashfsReaderWithOffset::new(reader, offset)?;
            Box::new(reader)
        };
        Self::inner_from_reader_with_offset_and_kind(reader, kind)
    }

    fn inner_from_reader_with_offset_and_kind(
        mut reader: Box<dyn BufReadSeek + 'b>,
        kind: Kind,
    ) -> Result<Self, BackhandError> {
        let (superblock, compression_options) =
            Self::superblock_and_compression_options(&mut reader, &kind)?;

        // Check if legal image
        let total_length = reader.seek(SeekFrom::End(0))?;
        reader.rewind()?;
        if u64::from(superblock.bytes_used) > total_length {
            error!("corrupted or invalid bytes_used");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // check required fields
        if u64::from(superblock.uid_start) > total_length {
            error!("corrupted or invalid xattr_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if u64::from(superblock.inode_table_start) > total_length {
            error!("corrupted or invalid inode_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if u64::from(superblock.directory_table_start) > total_length {
            error!("corrupted or invalid dir_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // check optional fields
        // if superblock.xattr_table != NOT_SET && superblock.xattr_table > total_length {
        //     error!("corrupted or invalid frag_table");
        //     return Err(BackhandError::CorruptedOrInvalidSquashfs);
        // }
        if u64::from(superblock.fragment_table_start) != NOT_SET
            && u64::from(superblock.fragment_table_start) > total_length
        {
            error!("corrupted or invalid frag_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        // if superblock.export_table != NOT_SET && superblock.export_table > total_length {
        //     error!("corrupted or invalid export_table");
        //     return Err(BackhandError::CorruptedOrInvalidSquashfs);
        // }

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Inodes @ {:02x?}", superblock.inode_table_start);
        let inodes = reader.inodes(&superblock, &kind)?;

        info!("Reading Root Inode");
        let root_inode = reader.root_inode(&superblock, &kind)?;

        info!("Reading Fragments");
        let fragments = reader.fragments(&superblock, &kind)?;
        let fragment_ptr = fragments.as_ref().map(|frag| frag.0);
        let fragment_table = fragments.map(|a| a.1);

        info!("Reading Exports");
        let export = reader.export(&superblock, &kind)?;
        let export_ptr = export.as_ref().map(|export| export.0);
        let export_table = export.map(|a| a.1);

        info!("Reading Uids");
        let uid_table = reader.uid(&superblock, &kind)?;

        info!("Reading Guids");
        let guid_table = reader.guid(&superblock, &kind)?;

        // let last_dir_position = if let Some(fragment_ptr) = fragment_ptr {
        //     trace!("using fragment for end of dir");
        //     fragment_ptr
        // } else if let Some(export_ptr) = export_ptr {
        //     trace!("using export for end of dir");
        //     export_ptr
        // } else {
        //     trace!("using id for end of dir");
        //     id_ptr
        // };

        info!("Reading Dirs");
        let dir_blocks = reader.dir_blocks(&superblock, superblock.fragment_table_start, &kind)?;

        let squashfs = Squashfs {
            kind,
            superblock,
            compression_options,
            inodes,
            root_inode,
            dir_blocks,
            fragments: fragment_table,
            export: export_table,
            id: None,
            uid: Some(uid_table),
            guid: Some(guid_table),
            file: reader,
        };

        // show info about flags
        // if superblock.inodes_uncompressed() {
        //     info!("flag: inodes uncompressed");
        // }

        // if superblock.data_block_stored_uncompressed() {
        //     info!("flag: data blocks stored uncompressed");
        // }

        // if superblock.fragments_stored_uncompressed() {
        //     info!("flag: fragments stored uncompressed");
        // }

        // if superblock.fragments_are_not_used() {
        //     info!("flag: fragments are not used");
        // }

        // if superblock.fragments_are_always_generated() {
        //     info!("flag: fragments are always generated");
        // }

        // if superblock.data_has_been_duplicated() {
        //     info!("flag: data has been duplicated");
        // }

        // if superblock.nfs_export_table_exists() {
        //     info!("flag: nfs export table exists");
        // }

        // if superblock.xattrs_are_stored_uncompressed() {
        //     info!("flag: xattrs are stored uncompressed");
        // }

        // if superblock.compressor_options_are_present() {
        //     info!("flag: compressor options are present");
        // }

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
        offset: u32,
    ) -> Result<Option<Vec<Dir>>, BackhandError> {
        trace!("- block index : {:02x?}", block_index);
        trace!("- file_size   : {:02x?}", file_size);
        trace!("- offset      : {:02x?}", offset);
        // if file_size < 4 {
        //     return Ok(None);
        // }

        // ignore blocks before our block_index, grab all the rest of the bytes
        // TODO: perf
        let block: Vec<u8> = self
            .dir_blocks
            .iter()
            .filter(|(a, _)| *a >= block_index)
            .flat_map(|(_, b)| b.iter())
            .copied()
            .collect();

        //let bytes = &block[offset as usize..];
        let bytes = &block;
        trace!("bytes: {block:02x?}");
        let mut dirs = vec![];
        // Read until we fail to turn bytes into `T`
        let mut cursor = Cursor::new(bytes);
        let mut container = Reader::new(&mut cursor);
        loop {
            match Dir::from_reader_with_ctx(
                &mut container,
                (
                    self.kind.inner.type_endian,
                    self.kind.inner.bit_order.unwrap(),
                ),
            ) {
                Ok(t) => {
                    log::trace!("{:02x?}", t);
                    dirs.push(t);
                }
                Err(e) => {
                    // don't error, altough I think it should error if we have our offsets
                    // all correct
                    //panic!("{e}");
                    break;
                }
            }
        }

        trace!("finish: {dirs:?}");
        Ok(Some(dirs))
    }

    #[instrument(skip_all)]
    fn extract_dir(
        &self,
        fullpath: &mut PathBuf,
        root: &mut Nodes<SquashfsFileReader>,
        dir_inode: &Inode,
        uid_table: &[u16],
        guid_table: &[u16],
    ) -> Result<(), BackhandError> {
        let dirs = match &dir_inode.inner {
            InodeInner::BasicDirectory(basic_dir) => {
                trace!("BASIC_DIR inodes: {:02x?}", basic_dir);
                self.dir_from_index(
                    basic_dir.start_block.try_into().unwrap(),
                    basic_dir.file_size.try_into().unwrap(),
                    basic_dir.offset.try_into().unwrap(),
                )?
            }
            InodeInner::ExtendedDirectory(ext_dir) => {
                todo!();
                // trace!("EXT_DIR: {:#02x?}", ext_dir);
                // self.dir_from_index(
                //     ext_dir.block_index.try_into().unwrap(),
                //     ext_dir.file_size,
                //     ext_dir.block_offset as usize,
                // )?
            }
            _ => return Err(BackhandError::UnexpectedInode),
        };
        if let Some(dirs) = dirs {
            for d in &dirs {
                trace!("extracing entry: {:#?}", d.dir_entries);
                for entry in &d.dir_entries {
                    let inode_key = (d.inode_num as i32 + entry.inode_offset as i32)
                        .try_into()
                        .unwrap();
                    let found_inode = &self.inodes[&inode_key];
                    let header = found_inode.header;
                    fullpath.push(entry.name()?);

                    let inner: InnerNode<SquashfsFileReader> = match entry.t {
                        // BasicDirectory, ExtendedDirectory
                        DirInodeId::BasicDirectory | DirInodeId::ExtendedDirectory => {
                            // its a dir, extract all children inodes
                            self.extract_dir(
                                fullpath,
                                root,
                                found_inode,
                                self.uid.as_ref().unwrap(),
                                self.guid.as_ref().unwrap(),
                            )?;
                            InnerNode::Dir(SquashfsDir::default())
                        }
                        // BasicFile
                        DirInodeId::BasicFile => {
                            trace!("before_file: {:#02x?}", entry);
                            let basic = match &found_inode.inner {
                                InodeInner::BasicFile(file) => file.clone(),
                                InodeInner::ExtendedFile(file) => todo!(), //file.into(),
                                _ => return Err(BackhandError::UnexpectedInode),
                            };
                            InnerNode::File(SquashfsFileReader { basic })
                        }
                        // Basic Symlink
                        DirInodeId::BasicSymlink => {
                            let link = self.symlink(found_inode)?;
                            InnerNode::Symlink(SquashfsSymlink { link })
                        }
                        // Basic CharacterDevice
                        DirInodeId::BasicCharacterDevice => {
                            let device_number = self.char_device(found_inode)?;
                            InnerNode::CharacterDevice(SquashfsCharacterDevice { device_number })
                        }
                        // Basic CharacterDevice
                        DirInodeId::BasicBlockDevice => {
                            let device_number = self.block_device(found_inode)?;
                            InnerNode::BlockDevice(SquashfsBlockDevice { device_number })
                        }
                        DirInodeId::ExtendedFile => return Err(BackhandError::UnsupportedInode),
                    };
                    let node = Node::new(
                        fullpath.clone(),
                        NodeHeader::from_inode(header, guid_table, uid_table),
                        inner,
                    );
                    root.nodes.push(node);
                    fullpath.pop();
                }
            }
        }
        //TODO: todo!("verify all the paths are valid");
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

    /// Convert into [`FilesystemReader`] by extracting all file bytes and converting into a filesystem
    /// like structure in-memory
    #[instrument(skip_all)]
    pub fn into_filesystem_reader(self) -> Result<FilesystemReader<'b>, BackhandError> {
        info!("creating fs tree");
        let mut root = Nodes::new_root(NodeHeader::from_inode(
            self.root_inode.header,
            &self.uid.as_ref().unwrap(),
            &self.guid.as_ref().unwrap(),
        ));
        self.extract_dir(
            &mut PathBuf::from("/"),
            &mut root,
            &self.root_inode,
            &self.guid.as_ref().unwrap(),
            &self.uid.as_ref().unwrap(),
        )?;
        root.nodes.sort();

        info!("created fs tree");
        let filesystem = FilesystemReader {
            kind: self.kind,
            block_size: self.superblock.block_size_1 as u32,
            block_log: self.superblock.block_log,
            compressor: Compressor::Gzip,
            compression_options: self.compression_options,
            mod_time: self.superblock.mkfs_time,
            id_table: None,
            uid_table: self.uid,
            guid_table: self.guid,
            fragments: self.fragments,
            root,
            reader: Mutex::new(Box::new(self.file)),
            cache: Mutex::new(Cache::default()),
        };
        Ok(filesystem)
    }
}
