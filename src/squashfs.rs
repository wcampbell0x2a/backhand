//! [`Squashfs`], [`Id`], and [`Export`]

use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::io::{Cursor, Read, SeekFrom};
use std::path::{Path, PathBuf};

use deku::bitvec::BitVec;
use deku::prelude::*;
use tracing::{debug, error, info, instrument, trace};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::dir::{Dir, DirEntry};
use crate::error::SquashfsError;
use crate::filesystem::{Filesystem, Node, SquashfsFile, SquashfsPath, SquashfsSymlink};
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
    #[deku(assert_eq = "Self::MAGIC")]
    pub magic: u32,
    pub inode_count: u32,
    pub mod_time: u32,
    pub block_size: u32,
    pub frag_count: u32,
    pub compressor: Compressor,
    pub block_log: u16,
    pub flags: u16,
    pub id_count: u16,
    #[deku(assert_eq = "4")]
    pub version_major: u16,
    #[deku(assert_eq = "0")]
    pub version_minor: u16,
    pub root_inode: u64,
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
    const MAGIC: u32 = 0x73717368;
    const BLOCK_SIZE: u32 = 0x20000;
    const BLOCK_LOG: u16 = 0x11;
    const VERSION_MAJ: u16 = 4;
    const VERSION_MIN: u16 = 0;
    const NOT_SET: u64 = 0xffffffffffffffff;

    pub fn new(compressor: Compressor) -> Self {
        Self {
            magic: Self::MAGIC,
            inode_count: 0,
            mod_time: 0,
            block_size: Self::BLOCK_SIZE, // use const
            frag_count: 0,
            compressor,
            block_log: Self::BLOCK_LOG,
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
            // add metadata size
            Some(size + 2)
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

// TODO: add data cache?
#[derive(Default)]
struct Cache {
    pub fragment_cache: HashMap<u64, Vec<u8>, BuildHasherDefault<twox_hash::XxHash64>>,
}

/// Container for a Squashfs Image stored in memory
pub struct Squashfs {
    pub superblock: SuperBlock,
    pub compression_options: Option<CompressionOptions>,
    /// Section containing Data and Fragments.
    ///
    /// This also contains the superblock and option bytes b/c that is how BasicFile uses its
    /// blocks_starts.
    pub data_and_fragments: Vec<u8>,
    // All Inodes
    pub inodes: HashMap<u32, Inode, BuildHasherDefault<twox_hash::XxHash64>>,
    /// Root Inode
    pub root_inode: Inode,
    /// Bytes containing Directory Table
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
                // ingore the size above, that is just a warning that some other information is in
                // the Option that isn't in the squashfs spec.
                //
                // This appears in TP-Link rootfs squashfs.
                let bytes = metadata::read_block(&mut reader, &superblock)?;

                // TODO: test if this is correct
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
        trace!("compression_options: {compression_options:08x?}");

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
        let fragments = squashfs_reader.fragments(&superblock).unwrap();
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

        info!("Successful Read");
        Ok(squashfs)
    }

    pub fn all_dirs(&self) -> Result<Vec<Dir>, SquashfsError> {
        let bytes: Vec<u8> = self
            .dir_blocks
            .iter()
            .flat_map(|(_, b)| b.clone())
            .collect();

        let mut dirs = vec![];
        let mut rest = bytes;
        while !rest.is_empty() {
            let ((r, _), dir) = Dir::from_bytes((&rest, 0)).unwrap();
            rest = r.to_vec();
            dirs.push(dir);
        }

        Ok(dirs)
    }

    ///
    ///
    ///
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

    /// Extract all files(File/Symlink/Path) from image
    #[instrument(skip_all)]
    pub fn into_filesystem(&self) -> Result<Filesystem, SquashfsError> {
        let mut cache = Cache::default();
        let mut nodes = Vec::with_capacity(self.superblock.inode_count as usize);
        let path: PathBuf = "/".into();

        self.extract_dir(&mut cache, &mut nodes, &self.root_inode, &path)?;

        let root_inode = SquashfsPath {
            header: self.root_inode.header.into(),
            path: "/".into(),
        };

        let filesystem = Filesystem {
            compressor: self.superblock.compressor,
            compression_options: self.compression_options,
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
                    let inode_key = d.inode_num + entry.inode_offset as u32;
                    trace!("extracing inode: {inode_key}");
                    let found_inode = &self.inodes[&inode_key];
                    trace!("extracing inode: {found_inode:?}");
                    let header = found_inode.header;
                    let mut new_path = path.to_path_buf();
                    new_path.push(entry.name());

                    match entry.t {
                        // BasicDirectory, ExtendedDirectory
                        InodeId::BasicDirectory | InodeId::ExtendedDirectory => {
                            nodes.push(Node::Path(SquashfsPath {
                                header: header.into(),
                                path: new_path.clone(),
                            }));

                            // its a dir, extract all inodes
                            self.extract_dir(cache, nodes, found_inode, &new_path)?;
                        },
                        // BasicFile
                        InodeId::BasicFile => {
                            trace!("before_file: {:#02x?}", entry);
                            let (header, bytes) = self.file(cache, found_inode)?;
                            let file = Node::File(SquashfsFile {
                                header: header.into(),
                                path: new_path,
                                bytes,
                            });
                            nodes.push(file);
                        },
                        // Basic Symlink
                        InodeId::BasicSymlink => {
                            let (original, link) = self.symlink(found_inode, entry)?;
                            let symlink = Node::Symlink(SquashfsSymlink {
                                header: header.into(),
                                path: new_path,
                                original,
                                link,
                            });
                            nodes.push(symlink);
                        },
                        _ => (),
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
            let og_position = reader.position();
            debug!("og: {:02x?}", og_position);
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
                debug!("Extracting frag: {:02x?}", frag);

                // use fragment cache if possible
                match cache.fragment_cache.get(&(frag.start)) {
                    Some(cache_bytes) => {
                        data_bytes.append(&mut cache_bytes.clone());
                    },
                    None => {
                        let mut reader =
                            Cursor::new(&self.data_and_fragments[frag.start as usize..]);
                        let mut bytes = self.read_data(&mut reader, frag.size as usize)?;
                        cache.fragment_cache.insert(frag.start, bytes.clone());
                        data_bytes.append(&mut bytes);
                    },
                }
            }
        }

        data_bytes = data_bytes[basic_file.block_offset as usize..]
            [..basic_file.file_size as usize]
            .to_vec();

        Ok(data_bytes)
    }

    /// Read from either Data blocks or Fragments blocks
    fn read_data<R: Read>(&self, reader: &mut R, size: usize) -> Result<Vec<u8>, SquashfsError> {
        let uncompressed = size & (1 << 24) != 0;
        let size = size & !(1 << 24);

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
        debug!("{:#?}", inode);
        if let InodeInner::BasicSymlink(basic_sym) = &inode.inner {
            return Ok((
                String::from_utf8(entry.name.clone())?,
                String::from_utf8(basic_sym.target_path.clone())?,
            ));
        }

        error!("symlink not found");
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
