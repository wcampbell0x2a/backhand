//! Read from on-disk image

use no_std_io2::io::Seek;
use std::ffi::OsString;
use std::io::{Cursor, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use deku::prelude::*;
use solana_nohash_hasher::IntMap;
use tracing::{error, info, instrument, trace, warn};

use super::export::Export;
use super::filesystem::node::{
    InnerNode, Node, NodeHeader, Nodes, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir,
    SquashfsFileReader, SquashfsSymlink,
};
use super::filesystem::reader::FilesystemReader;
use super::id::Id;
use crate::Flags;
use crate::error::BackhandError;
use crate::kinds::{Kind, LE_V4_0};
use crate::v3::dir::{Dir, DirInodeId};
use crate::v3::fragment::Fragment;
use crate::v3::inode::{Inode, InodeInner};
use crate::v3::reader::{SquashFsReader, SquashfsReaderWithOffset};
use crate::v3::unix_string::OsStringExt;
use crate::v4::reader::BufReadSeek;

/// 128KiB
pub const DEFAULT_BLOCK_SIZE: u32 = 0x20000;

/// 4KiB
pub const DEFAULT_PAD_LEN: u32 = 0x1000;

/// 1MiB
pub const MAX_BLOCK_SIZE: u32 = 0x10_0000;

/// 4KiB
pub const MIN_BLOCK_SIZE: u32 = 0x1000;

#[derive(Debug, Copy, Clone, DekuRead, DekuSize, PartialEq, Eq)]
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
    pub version_major: u16,
    pub version_minor: u16,
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
    pub const SIZE: usize = Self::SIZE_BYTES.unwrap();

    pub fn new(kind: Kind) -> Self {
        Self {
            magic: kind.inner.magic,
            inode_count: 0,
            bytes_used_2: 0,
            uid_start_2: 0,
            guid_start_2: 0,
            inode_table_start_2: 0,
            directory_table_start_2: 0,
            version_major: kind.inner.version_major,
            version_minor: kind.inner.version_minor,
            block_size_1: 0,
            block_log: 0,
            flags: 0,
            no_uids: 0,
            no_guids: 0,
            mkfs_time: 0,
            root_inode: 0,
            block_size: 0,
            fragments: 0,
            fragment_table_start_2: 0,
            bytes_used: 0,
            uid_start: 0,
            guid_start: 0,
            inode_table_start: 0,
            directory_table_start: 0,
            fragment_table_start: 0,
            unused: 0,
        }
    }

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

    /// If set to true, metadata blocks have a leading check byte
    pub fn check_data(&self) -> bool {
        u16::from(self.flags) & Flags::Unused as u16 != 0
    }
}

#[derive(Default, Clone, Debug)]
pub(crate) struct Cache {
    /// The first time a fragment bytes is read, those bytes are added to this map with the key
    /// representing the start position
    pub(crate) fragment_cache: IntMap<u64, Vec<u8>>,
}

/// Squashfs Image initial read information
///
/// See [`FilesystemReader`] for a representation with the data extracted and uncompressed.
pub struct Squashfs<'b> {
    pub kind: Kind,
    pub superblock: SuperBlock,
    // All Inodes
    pub inodes: IntMap<u32, Inode>,
    /// Root Inode
    pub root_inode: Inode,
    /// Bytes containing Directory Table
    pub dir_blocks: (IntMap<u64, u64>, Vec<u8>),
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
    pub file: Box<dyn BufReadSeek + 'b>,
}

impl<'b> Squashfs<'b> {
    /// Read Superblock at current `reader` offset without parsing inodes and dirs
    ///
    /// Used for unsquashfs (extraction and --stat)
    pub fn superblock_and_compression_options(
        reader: &mut Box<dyn BufReadSeek + 'b>,
        kind: &Kind,
    ) -> Result<(SuperBlock, Option<()>), BackhandError> {
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

        if superblock.block_size.ilog2() != superblock.block_log as u32 {
            error!("block size.log2() != block_log");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // v3 doesn't have compression options - always uses gzip
        Ok((superblock, None))
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
        Self::from_reader_with_offset_and_kind(reader, offset, Kind { inner: Arc::new(LE_V4_0) })
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
        let (superblock, _) = Self::superblock_and_compression_options(&mut reader, &kind)?;

        // Check if legal image
        let total_length = reader.seek(SeekFrom::End(0))?;
        reader.rewind()?;
        if superblock.bytes_used > total_length {
            error!("corrupted or invalid bytes_used");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // check required fields
        if superblock.uid_start > total_length {
            error!("corrupted or invalid xattr_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.inode_table_start > total_length {
            error!("corrupted or invalid inode_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.directory_table_start > total_length {
            error!("corrupted or invalid dir_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        if superblock.fragment_table_start != NOT_SET
            && superblock.fragment_table_start > total_length
        {
            error!("corrupted or invalid frag_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Inodes @ {:02x?}", superblock.inode_table_start);
        let inodes = reader.inodes(&superblock, &kind)?;

        info!("Reading Root Inode");
        let root_inode = reader.root_inode(&superblock, &kind)?;

        info!("Reading Fragments");
        let fragments = reader.fragments(&superblock, &kind)?;
        let _fragment_ptr = fragments.as_ref().map(|frag| frag.0);
        let fragment_table = fragments.map(|a| a.1);

        info!("Reading Exports");
        let export = reader.export(&superblock, &kind)?;
        let _export_ptr = export.as_ref().map(|export| export.0);
        let export_table = export.map(|a| a.1);

        info!("Reading Uids");
        let uid_table = reader.uid(&superblock, &kind)?;

        info!("Reading Guids");
        let guid_table = reader.guid(&superblock, &kind)?;

        info!("Reading Dirs");
        let dir_blocks = reader.dir_blocks(&superblock, superblock.fragment_table_start, &kind)?;

        let squashfs = Squashfs {
            kind,
            superblock,
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
        offset: u16,
    ) -> Result<Option<Vec<Dir>>, BackhandError> {
        trace!("- block index : {:02x?}", block_index);
        trace!("- file_size   : {:02x?}", file_size);
        trace!("- offset      : {:02x?}", offset);

        if file_size < 4 {
            return Ok(None);
        }

        let (block_map, dir_data) = &self.dir_blocks;

        let abs_byte_pos = block_map.get(&block_index).copied().ok_or_else(|| {
            error!("Could not find metadata block at compressed offset {}", block_index);
            BackhandError::CorruptedOrInvalidSquashfs
        })?;

        let start_byte_pos = abs_byte_pos + offset as u64;

        let mut cursor = Cursor::new(dir_data);
        let mut dirs = vec![];

        // Seek to the calculated absolute position
        if start_byte_pos as usize >= dir_data.len() {
            error!(
                "Start position {} is beyond directory table length {}",
                start_byte_pos,
                dir_data.len()
            );
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        cursor.set_position(start_byte_pos);
        let start_pos = cursor.position() as usize;

        let actual_dir_size = if file_size <= 3 {
            trace!("Empty directory (file_size={})", file_size);
            return Ok(None);
        } else {
            file_size - 3
        };

        let end_pos = if actual_dir_size > 65536 {
            trace!(
                "ExtendedDirectory detected (actual_dir_size={}), reading multiple Dir blocks",
                actual_dir_size
            );
            core::cmp::min(start_pos + actual_dir_size as usize, dir_data.len())
        } else {
            trace!(
                "Regular directory (actual_dir_size={}), reading single Dir block",
                actual_dir_size
            );
            core::cmp::min(start_pos + actual_dir_size as usize, dir_data.len())
        };
        while cursor.position() < end_pos as u64 {
            let _current_pos = cursor.position() as usize;

            let mut container = Reader::new(&mut cursor);
            match Dir::from_reader_with_ctx(
                &mut container,
                (self.kind.inner.type_endian, self.kind.inner.bit_order.unwrap()),
            ) {
                Ok(dir) => {
                    dirs.push(dir);
                }
                Err(_e) => {
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
        current_path: &mut PathBuf,
        root: &mut Nodes<SquashfsFileReader>,
        dir_inode: &Inode,
        _uid_table: &[u16],
        _guid_table: &[u16],
        visited_inodes: &mut std::collections::HashSet<u32>,
    ) -> Result<(), BackhandError> {
        let current_inode_num = dir_inode.header.inode_number;

        // Cycle detection: skip already-visited inodes to prevent infinite recursion
        // (don't error - squashfs may reuse directory structures)
        if visited_inodes.contains(&current_inode_num) {
            return Ok(());
        }
        visited_inodes.insert(current_inode_num);

        let dirs = match &dir_inode.inner {
            InodeInner::BasicDirectory(basic_dir) => self.dir_from_index(
                u64::from(basic_dir.start_block),
                basic_dir.file_size,
                basic_dir.offset,
            )?,
            InodeInner::ExtendedDirectory(ext_dir) => self.dir_from_index(
                u64::from(ext_dir.start_block),
                ext_dir.file_size,
                ext_dir.block_offset as u16,
            )?,
            _ => return Err(BackhandError::UnexpectedInode),
        };

        if let Some(dirs) = dirs {
            for d in &dirs {
                for entry in &d.dir_entries {
                    let entry_name_path = entry.name()?;
                    let entry_name = entry_name_path.to_string_lossy();
                    if entry_name == "." || entry_name == ".." {
                        continue;
                    }

                    let Ok(inode_key) = (d.inode_num + entry.inode_offset as i32).try_into() else {
                        return Err(BackhandError::CorruptedOrInvalidSquashfs);
                    };
                    let Some(found_inode) = &self.inodes.get(&inode_key) else {
                        error!("inode_key: {inode_key} not found");
                        return Err(BackhandError::CorruptedOrInvalidSquashfs);
                    };

                    let header = found_inode.header;
                    current_path.push(entry_name_path);

                    let inner: InnerNode<SquashfsFileReader> = match entry.t {
                        DirInodeId::BasicDirectory | DirInodeId::ExtendedDirectory => {
                            if found_inode.header.inode_number == dir_inode.header.inode_number {
                                // Skip self-referential
                            } else {
                                self.extract_dir(
                                    current_path,
                                    root,
                                    found_inode,
                                    _uid_table,
                                    _guid_table,
                                    visited_inodes,
                                )?;
                            }
                            InnerNode::Dir(SquashfsDir::default())
                        }
                        // BasicFile
                        DirInodeId::BasicFile => {
                            let inner = match &found_inode.inner {
                                InodeInner::BasicFile(file) => {
                                    SquashfsFileReader::Basic(file.clone())
                                }
                                InodeInner::ExtendedFile(file) => {
                                    SquashfsFileReader::Extended(file.clone())
                                }
                                _ => {
                                    error!("BasicFile: file not found");
                                    return Err(BackhandError::FileNotFound);
                                }
                            };
                            InnerNode::File(inner)
                        }
                        // ExtendedFile
                        DirInodeId::ExtendedFile => {
                            let inner = match &found_inode.inner {
                                InodeInner::ExtendedFile(file) => {
                                    SquashfsFileReader::Extended(file.clone())
                                }
                                InodeInner::BasicFile(file) => {
                                    SquashfsFileReader::Basic(file.clone())
                                }
                                _ => {
                                    error!("ExtendedFile: file not found");
                                    return Err(BackhandError::FileNotFound);
                                }
                            };
                            InnerNode::File(inner)
                        }
                        // BasicSymlink
                        DirInodeId::BasicSymlink => {
                            let target = self.symlink(found_inode)?;
                            InnerNode::Symlink(SquashfsSymlink { link: target })
                        }
                        // BasicCharacterDevice
                        DirInodeId::BasicCharacterDevice => {
                            let device = self.char_device(found_inode)?;
                            InnerNode::CharacterDevice(SquashfsCharacterDevice {
                                device_number: device,
                            })
                        }
                        // BasicBlockDevice
                        DirInodeId::BasicBlockDevice => {
                            let device = self.block_device(found_inode)?;
                            InnerNode::BlockDevice(SquashfsBlockDevice { device_number: device })
                        }
                        // BasicNamedPipe
                        DirInodeId::BasicNamedPipe => InnerNode::NamedPipe,
                        // BasicSocket
                        DirInodeId::BasicSocket => InnerNode::Socket,
                        _ => {
                            error!("unknown inode type: {:?}", entry.t);
                            return Err(BackhandError::UnexpectedInode);
                        }
                    };

                    let node_header = NodeHeader::from_inode(header, &[])?;
                    let node = Node { header: node_header, inner, fullpath: current_path.clone() };
                    root.nodes.push(node);
                    current_path.pop();
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

    /// Convert into [`FilesystemReader`] by extracting all file bytes and converting into a filesystem
    /// like structure in-memory
    #[instrument(skip_all)]
    pub fn into_filesystem_reader(self) -> Result<FilesystemReader<'b>, BackhandError> {
        info!("creating fs tree");
        let mut root = Nodes::new_root({
            // Create temporary combined id table for v3 compatibility
            let mut id_table = Vec::new();
            for &uid in self.uid.as_ref().unwrap() {
                id_table.push(Id::new(uid as u32));
            }
            NodeHeader::from_inode(self.root_inode.header, &id_table)?
        });
        let mut visited_inodes = std::collections::HashSet::new();
        self.extract_dir(
            &mut PathBuf::from("/"),
            &mut root,
            &self.root_inode,
            self.uid.as_ref().unwrap(),
            self.guid.as_ref().unwrap(),
            &mut visited_inodes,
        )?;
        root.nodes.sort();

        info!("created fs tree");
        let filesystem = FilesystemReader {
            kind: self.kind,
            block_size: self.superblock.block_size_1 as u32,
            block_log: self.superblock.block_log,
            compressor: None,
            mod_time: self.superblock.mkfs_time,
            id_table: {
                // Convert v3 uid table to unified id table format
                let mut id_table = Vec::new();
                if let Some(ref uid_table) = self.uid {
                    for &uid in uid_table {
                        id_table.push(Id::new(uid as u32));
                    }
                }
                id_table
            },
            fragments: self.fragments,
            root,
            reader: Mutex::new(Box::new(self.file)),
            cache: RwLock::new(Cache::default()),
        };
        Ok(filesystem)
    }
}
