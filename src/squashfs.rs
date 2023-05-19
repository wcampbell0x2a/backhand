//! Read from on-disk image

use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{Seek, SeekFrom};
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
use crate::filesystem::node::{InnerNode, Nodes};
use crate::fragment::Fragment;
use crate::inode::{Inode, InodeId, InodeInner};
use crate::kinds::{Kind, LE_V4_0};
use crate::reader::{BufReadSeek, SquashFsReader, SquashfsReaderWithOffset};
use crate::superblock::NOT_SET;
use crate::{
    metadata, FilesystemReader, Node, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir,
    SquashfsFileReader, SquashfsSymlink, SuperBlockTrait, SuperBlock_V4_0,
};

/// 128KiB
pub const DEFAULT_BLOCK_SIZE: u32 = 0x20000;

/// 4KiB
pub const DEFAULT_PAD_LEN: u32 = 0x1000;

/// log2 of 128KiB
pub const DEFAULT_BLOCK_LOG: u16 = 0x11;

/// 1MiB
pub const MAX_BLOCK_SIZE: u32 = byte_unit::n_mib_bytes!(1) as u32;

/// 4KiB
pub const MIN_BLOCK_SIZE: u32 = byte_unit::n_kb_bytes(4) as u32;

/// NFS export support
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "type_endian", ctx = "type_endian: deku::ctx::Endian")]
pub struct Export {
    pub num: u64,
}

/// 32 bit user and group IDs
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(endian = "type_endian", ctx = "type_endian: deku::ctx::Endian")]
pub struct Id {
    pub num: u32,
}

impl Id {
    pub const SIZE: usize = (u32::BITS / 8) as usize;

    pub fn new(num: u32) -> Id {
        Id { num }
    }

    pub fn root() -> Vec<Id> {
        vec![Id { num: 0 }]
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
pub struct Squashfs {
    pub kind: Kind,
    pub superblock: Box<dyn SuperBlockTrait>,
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
    file: Box<dyn BufReadSeek>,
}

impl Squashfs {
    /// Read Superblock and Compression Options at current `reader` offset without parsing inodes
    /// and dirs
    ///
    /// Used for unsquashfs --stat
    pub fn superblock_and_compression_options(
        reader: &mut Box<dyn BufReadSeek>,
        kind: &Kind,
    ) -> Result<(Box<dyn SuperBlockTrait>, Option<CompressionOptions>), BackhandError> {
        // Size of metadata + optional compression options metadata block
        let mut superblock = [0u8; 96];
        reader.read_exact(&mut superblock)?;

        // Parse SuperBlock
        let bs = superblock.view_bits::<deku::bitvec::Msb0>();

        // For every version, parse a SuperBlock using the DekuRead::read(..) function
        let (_, superblock) = match (kind.inner.version_major, kind.inner.version_minor) {
            (4, 0) => SuperBlock_V4_0::read(
                bs,
                (
                    kind.inner.magic,
                    kind.inner.version_major,
                    kind.inner.version_minor,
                    kind.inner.type_endian,
                ),
            )?,
            _ => unimplemented!(),
        };

        let block_size = superblock.block_size();
        let power_of_two = block_size != 0 && (block_size & (block_size - 1)) == 0;
        if !(MIN_BLOCK_SIZE..=MAX_BLOCK_SIZE).contains(&block_size) || !power_of_two {
            error!("block_size({:#02x}) invalid", superblock.block_size);
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        if (block_size as f32).log2() != superblock.block_log as f32 {
            error!("block size.log2() != block_log");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Parse Compression Options, if any
        info!("Reading Compression options");
        let compression_options = if superblock.compressor != Compressor::None
            && superblock.compressor_options_are_present()
        {
            let bytes = metadata::read_block(reader, superblock.compressor(), kind)?;
            // data -> compression options
            let bv = BitVec::from_slice(&bytes);
            match CompressionOptions::read(&bv, (kind.inner.type_endian, superblock.compressor)) {
                Ok(co) => {
                    if !co.0.is_empty() {
                        error!("invalid compression options, bytes left over, using");
                    }
                    Some(co.1)
                },
                Err(e) => {
                    error!("invalid compression options: {e:?}[{bytes:02x?}], not using");
                    None
                },
            }
        } else {
            None
        };
        info!("compression_options: {compression_options:02x?}");

        Ok((Box::new(superblock), compression_options))
    }

    /// Create `Squashfs` from `Read`er, with the resulting squashfs having read all fields needed
    /// to regenerate the original squashfs and interact with the fs in memory without needing to
    /// read again from `Read`er. `reader` needs to start with the beginning of the Image.
    pub fn from_reader(reader: impl BufReadSeek + 'static) -> Result<Squashfs, BackhandError> {
        Self::from_reader_with_offset(reader, 0)
    }

    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before Reading
    ///
    /// Uses default [`Kind`]: [`LE_V4_0`]
    pub fn from_reader_with_offset(
        reader: impl BufReadSeek + 'static,
        offset: u64,
    ) -> Result<Squashfs, BackhandError> {
        Self::from_reader_with_offset_and_kind(
            reader,
            offset,
            Kind {
                inner: Rc::new(LE_V4_0),
            },
        )
    }

    /// Same as [`Self::from_reader_with_offset`], but including custom `kind`
    pub fn from_reader_with_offset_and_kind(
        reader: impl BufReadSeek + 'static,
        offset: u64,
        kind: Kind,
    ) -> Result<Squashfs, BackhandError> {
        let reader: Box<dyn BufReadSeek> = if offset == 0 {
            Box::new(reader)
        } else {
            let reader = SquashfsReaderWithOffset::new(reader, offset)?;
            Box::new(reader)
        };
        Self::inner_from_reader_with_offset_and_kind(reader, kind)
    }

    fn inner_from_reader_with_offset_and_kind(
        mut reader: Box<dyn BufReadSeek>,
        kind: Kind,
    ) -> Result<Squashfs, BackhandError> {
        let (superblock, compression_options) =
            Self::superblock_and_compression_options(&mut reader, &kind)?;

        // Check if legal image
        let total_length = reader.seek(SeekFrom::End(0))?;
        reader.rewind()?;
        if superblock.bytes_used() > total_length {
            error!("corrupted or invalid bytes_used");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // check required fields
        if superblock.id_table() > total_length {
            error!("corrupted or invalid xattr_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.inode_table() > total_length {
            error!("corrupted or invalid inode_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.dir_table() > total_length {
            error!("corrupted or invalid dir_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // check optional fields
        if superblock.xattr_table() != NOT_SET && superblock.xattr_table() > total_length {
            error!("corrupted or invalid frag_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.frag_table() != NOT_SET && superblock.frag_table() > total_length {
            error!("corrupted or invalid frag_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        if superblock.export_table() != NOT_SET && superblock.export_table() > total_length {
            error!("corrupted or invalid export_table");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Inodes");
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

        info!("Reading Ids");
        let id = reader.id(&superblock, &kind)?;
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
        let dir_blocks = reader.dir_blocks(&superblock, last_dir_position, &kind)?;

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
        while let Ok((rest, t)) = Dir::read(all_bytes, self.kind.inner.type_endian) {
            dirs.push(t);
            all_bytes = rest;
        }

        trace!("finish");
        Ok(Some(dirs))
    }

    #[instrument(skip_all)]
    fn extract_dir(
        &self,
        fullpath: &mut PathBuf,
        root: &mut Nodes<SquashfsFileReader>,
        dir_inode: &Inode,
    ) -> Result<(), BackhandError> {
        let dirs = match &dir_inode.inner {
            InodeInner::BasicDirectory(basic_dir) => {
                trace!("BASIC_DIR inodes: {:02x?}", basic_dir);
                self.dir_from_index(
                    basic_dir.block_index.try_into().unwrap(),
                    basic_dir.file_size.try_into().unwrap(),
                    basic_dir.block_offset as usize,
                )?
            },
            InodeInner::ExtendedDirectory(ext_dir) => {
                trace!("EXT_DIR: {:#02x?}", ext_dir);
                self.dir_from_index(
                    ext_dir.block_index.try_into().unwrap(),
                    ext_dir.file_size,
                    ext_dir.block_offset as usize,
                )?
            },
            _ => return Err(BackhandError::UnexpectedInode(dir_inode.inner.clone())),
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
                    let new_path = entry.name();
                    fullpath.push(&new_path);

                    let inner: InnerNode<SquashfsFileReader> = match entry.t {
                        // BasicDirectory, ExtendedDirectory
                        InodeId::BasicDirectory | InodeId::ExtendedDirectory => {
                            // its a dir, extract all children inodes
                            self.extract_dir(fullpath, root, found_inode)?;
                            InnerNode::Dir(SquashfsDir::default())
                        },
                        // BasicFile
                        InodeId::BasicFile => {
                            trace!("before_file: {:#02x?}", entry);
                            let basic = match &found_inode.inner {
                                InodeInner::BasicFile(file) => file.clone(),
                                InodeInner::ExtendedFile(file) => file.into(),
                                _ => {
                                    return Err(BackhandError::UnexpectedInode(
                                        found_inode.inner.clone(),
                                    ))
                                },
                            };
                            InnerNode::File(SquashfsFileReader { basic })
                        },
                        // Basic Symlink
                        InodeId::BasicSymlink => {
                            let link = self.symlink(found_inode)?;
                            InnerNode::Symlink(SquashfsSymlink { link })
                        },
                        // Basic CharacterDevice
                        InodeId::BasicCharacterDevice => {
                            let device_number = self.char_device(found_inode)?;
                            InnerNode::CharacterDevice(SquashfsCharacterDevice { device_number })
                        },
                        // Basic CharacterDevice
                        InodeId::BasicBlockDevice => {
                            let device_number = self.block_device(found_inode)?;
                            InnerNode::BlockDevice(SquashfsBlockDevice { device_number })
                        },
                        InodeId::ExtendedFile => {
                            return Err(BackhandError::UnsupportedInode(found_inode.inner.clone()))
                        },
                    };
                    let node = Node::new(fullpath.clone(), header.into(), inner);
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
    pub fn into_filesystem_reader(self) -> Result<FilesystemReader, BackhandError> {
        let mut root = Nodes::new_root(self.root_inode.header.into());
        self.extract_dir(&mut PathBuf::from("/"), &mut root, &self.root_inode)?;
        root.nodes.sort();

        let filesystem = FilesystemReader {
            kind: self.kind,
            block_size: self.superblock.block_size(),
            block_log: self.superblock.block_log(),
            compressor: self.superblock.compressor(),
            compression_options: self.compression_options,
            mod_time: self.superblock.mod_time(),
            id_table: self.id,
            fragments: self.fragments,
            root,
            reader: RefCell::new(Box::new(self.file)),
            cache: RefCell::new(Cache::default()),
        };
        Ok(filesystem)
    }
}
