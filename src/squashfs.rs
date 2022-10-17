//! Module containing `Squashfs`
//!
//! # `Squashfs`
//! Contains both the `SuperBlock` and the parsed fields/sections that are stored in a squashfs image
//! and are populated with a call to `Squashfs::from_reader(..)`.

use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::PathBuf;

use deku::bitvec::BitVec;
use deku::prelude::*;
use tracing::instrument;
use xz2::read::XzDecoder;

use crate::error::SquashfsError;
use crate::fragment::FRAGMENT_SIZE;
use crate::{
    BasicDirectory, BasicFile, CompressionOptions, Compressor, Dir, DirEntry, Fragment, Inode,
    Metadata,
};

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct SuperBlock {
    // Superblock
    #[deku(assert_eq = "0x73717368")]
    magic: u32,
    inode_count: u32,
    mod_time: u32,
    // TODO: remove assert, see BasicFile::count()
    #[deku(assert_eq = "0x20000")]
    block_size: u32,
    frag_count: u32,
    compressor: Compressor,
    // TODO: remove assert, see BasicFile::count()
    #[deku(assert_eq = "0x11")]
    block_log: u16,
    //TODO: read as u16, then map to flags
    flags: u16,
    id_count: u16,
    //TODO: add assert
    version_major: u16,
    //TODO: add assert
    version_minor: u16,
    root_inode: u64,
    bytes_used: u64,
    id_table: u64,
    //TODO: add read into Squashfs
    xattr_table: u64,
    inode_table: u64,
    dir_table: u64,
    frag_table: u64,
    //TODO: add read into Squashfs
    export_table: u64,
}

impl SuperBlock {
    // Extract size of optional compression options
    pub fn compression_options_size(&self) -> Option<usize> {
        if (self.flags & Flags::CompressorOptionsArePresent as u16) != 0 {
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
}

// TODO: is_flagname() for all of these in `SuperBlock`
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

pub struct Squashfs {
    pub superblock: SuperBlock,
    pub compression_options: Option<CompressionOptions>,

    /// Section containing Data and Fragments.
    ///
    /// This also contains the superblock and option bytes b/c that is how BasicFile uses its
    /// blocks_starts.
    /// TODO: maybe bzero those bytes just in case?
    pub data_and_fragments: Vec<u8>,
    pub inodes: Vec<Inode>,
    pub root_inode: BasicDirectory,
    pub dir_blocks: Vec<Vec<u8>>,
    pub fragments: Option<Vec<Fragment>>,
}

/// public
impl Squashfs {
    /// Create `Squashfs` from `Read`er, with the resulting squashfs having read all fields needed
    /// to regenerate the original squashfs and interact with the fs in memory without needing to
    /// read again from `Read`er.
    #[instrument(skip_all)]
    pub fn from_reader<R: ReadSeek + 'static>(mut reader: R) -> Result<Squashfs, SquashfsError> {
        // Size of metadata + optional compression options metadata block
        let mut superblock = [0u8; 96];
        reader.read_exact(&mut superblock)?;

        // Parse SuperBlock
        let (_, superblock) = SuperBlock::from_bytes((&superblock, 0))?;

        // Parse Compression Options, if any
        let compression_options = if superblock.compressor != Compressor::None {
            if let Some(size) = superblock.compression_options_size() {
                let mut options = vec![0u8; size];
                reader.read_exact(&mut options)?;

                let bv = BitVec::from_slice(&options).unwrap();
                let (_, m) = Metadata::read(&bv, ()).unwrap();

                // data -> compression options
                let bv = BitVec::from_slice(&m.data).unwrap();
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

        // Create SquashfsReader
        let mut squashfs_io = SquashfsReader {
            io: Box::new(reader),
        };

        // Read all fields from filesystem to make a Squashfs
        let data_and_fragments = squashfs_io.data_and_fragments(&superblock)?;
        let pos_and_inodes = squashfs_io.inodes(&superblock)?;
        let root_inode = squashfs_io.root_inode(&superblock, &pos_and_inodes)?;
        let inodes = squashfs_io.discard_pos(&pos_and_inodes);
        let dir_blocks = squashfs_io.dir_blocks(&superblock, &inodes)?;
        let fragments = squashfs_io.fragments(&superblock)?;

        let squashfs = Squashfs {
            data_and_fragments,
            superblock,
            compression_options,
            root_inode,
            inodes,
            dir_blocks,
            fragments,
        };

        Ok(squashfs)
    }

    //pub fn to_bytes(&self, data_and_fragments: Vec<u8>) -> Vec<u8> {
    //    let mut v = vec![];

    //    v.append(&mut self.superblock.to_bytes().unwrap());
    //    // TODO: compression options
    //    v.append(&mut self.inodes.iter().map(|a| a.to_bytes().unwrap()).flatten().collect());
    //    v.append(&mut self.dir_blocks.iter().flatten().cloned().collect());
    //    if let Some(fragments) = &self.fragments {
    //        v.append(&mut fragments.iter().map(|a| a.to_bytes().unwrap()).flatten().collect());
    //    }

    //    v
    //}

    /// Extract all files
    pub fn extract_all_files(
        &self,
        squashfs: &Squashfs,
    ) -> Result<Vec<(PathBuf, Vec<u8>)>, SquashfsError> {
        let mut ret = vec![];
        for inode in &squashfs.inodes {
            if let Inode::BasicDirectory(basic_dir) = inode {
                let block = &squashfs.dir_blocks[basic_dir.block_index as usize];
                let bytes = &block[basic_dir.block_offset as usize..];
                let (_, dir) = Dir::from_bytes((bytes, 0))?;
                for entry in &dir.dir_entries {
                    // TODO: use type enum
                    if entry.t == 2 {
                        tracing::debug!("{dir:?}");
                        ret.push(squashfs.file(inode, dir.inode_num, entry)?);
                    }
                }
            }
        }

        Ok(ret)
    }

    /// Given a file_name from a squashfs filepath, extract the file and the filepath from the
    /// internal squashfs stored in the fields
    #[instrument(skip_all)]
    pub fn extract_file(
        &self,
        squashfs: &Squashfs,
        name: &str,
    ) -> Result<(PathBuf, Vec<u8>), SquashfsError> {
        // Search through inodes and parse directory table at the specified location
        // searching for first file name that matches
        for inode in &squashfs.inodes {
            tracing::trace!("Searching following inode for filename: {inode:#02x?}");
            if let Inode::BasicDirectory(basic_dir) = inode {
                let block = &squashfs.dir_blocks[basic_dir.block_index as usize];
                let bytes = &block[basic_dir.block_offset as usize..];
                let (_, dir) = Dir::from_bytes((bytes, 0))?;
                tracing::trace!("Searching following dir for filename: {dir:#02x?}");
                for entry in &dir.dir_entries {
                    // TODO: use type enum
                    if entry.t == 2 {
                        let entry_name = std::str::from_utf8(&entry.name)?;
                        tracing::debug!(entry_name);
                        if name == entry_name {
                            let file = self.file(inode, dir.inode_num, entry)?;
                            return Ok(file);
                        }
                    }
                }
            }
        }
        Err(SquashfsError::FileNotFound)
    }

    #[instrument(skip_all)]
    fn data(&self, basic_file: &BasicFile) -> Result<Vec<u8>, SquashfsError> {
        tracing::debug!("extracting: {basic_file:#02x?}");
        let mut bytes = Cursor::new(&self.data_and_fragments[basic_file.blocks_start as usize..]);

        // Add data
        let mut data_bytes = vec![];
        for block_size in &basic_file.block_sizes {
            // TODO: use deku for this?
            let uncompressed = block_size & (1 << 24) != 0;
            let size = block_size & !(1 << 24);
            let mut data = vec![0u8; size as usize];
            bytes.read_exact(&mut data)?;

            let mut bytes = if uncompressed {
                data
            } else {
                decompress(data, self.superblock.compressor)?
            };
            data_bytes.append(&mut bytes);
        }

        // Add fragments
        // TODO: this should be constant
        if basic_file.frag_index != 0xffffffff {
            if let Some(fragments) = &self.fragments {
                let frag = fragments[basic_file.frag_index as usize];
                let mut bytes = Cursor::new(&self.data_and_fragments[frag.start as usize..]);

                let uncompressed = frag.size & (1 << 24) != 0;
                let size = frag.size & !(1 << 24);

                let mut buf = vec![0u8; size as usize];
                bytes.read_exact(&mut buf)?;

                let mut bytes = if uncompressed {
                    buf
                } else {
                    decompress(buf, self.superblock.compressor)?
                };
                data_bytes.append(&mut bytes);
            }
        }

        data_bytes = data_bytes[basic_file.block_offset as usize..]
            [..basic_file.file_size as usize]
            .to_vec();

        Ok(data_bytes)
    }

    #[instrument(skip_all)]
    fn file(
        &self,
        found_inode: &Inode,
        found_inode_num: u32,
        found_entry: &DirEntry,
    ) -> Result<(PathBuf, Vec<u8>), SquashfsError> {
        //  TODO: remove
        let dir_inode = found_inode;
        let base_inode = found_inode_num;
        let entry = found_entry;
        let entry_name = std::str::from_utf8(&entry.name)?;
        let dir_inode = dir_inode.expect_dir();
        // Used for searching for the file later when we want to extract the bytes
        let looking_inode = base_inode as i16 + entry.inode_offset;

        let root_inode = self.root_inode.header.inode_number;
        tracing::debug!("searching for dir path to root inode: {:#02x?}", root_inode);

        let mut path_inodes = vec![];
        // first check if the dir inode of this file is the root inode
        if dir_inode.header.inode_number != root_inode {
            // check every inode and find the matching inode to the current dir_nodes parent
            // inode, when we find a new inode, check if it's the root inode, and continue on if it
            // isn't
            let mut next_inode = dir_inode.parent_inode;
            'outer: loop {
                for inode in &self.inodes {
                    if let Inode::BasicDirectory(basic_dir) = inode {
                        if basic_dir.header.inode_number == next_inode {
                            path_inodes.push(basic_dir);
                            if basic_dir.header.inode_number == root_inode {
                                break 'outer;
                            }
                            next_inode = basic_dir.parent_inode;
                        }
                    }
                }
            }
        }

        // Insert the first basic file
        path_inodes.insert(0, dir_inode);

        let mut paths = vec![];
        // Now we use n as the basic_dir with the inode, and look for that inode in the next
        // dir at the path directed from the path_inodes, when it matches, save it to the paths
        for n in 0..path_inodes.len() - 1 {
            let basic_dir_with_inode = path_inodes[n];
            let search_inode = basic_dir_with_inode.header.inode_number;

            let basic_dir_with_location = path_inodes[n + 1];
            let block = &self.dir_blocks[basic_dir_with_location.block_index as usize];
            let bytes = &block[basic_dir_with_location.block_offset as usize..];

            let (_, dir) = Dir::from_bytes((bytes, 0))?;
            let base_inode = dir.inode_num;

            for entry in &dir.dir_entries {
                let entry_name = String::from_utf8(entry.name.clone())?;
                if base_inode as i16 + entry.inode_offset == search_inode as i16 {
                    paths.push(entry_name);
                }
            }
        }

        // reverse the order, since we are looking at the file to the parent dirs
        let paths: Vec<&String> = paths.iter().rev().collect();

        // create PathBufs
        let mut pathbuf = PathBuf::new();
        for path in paths {
            pathbuf.push(path);
        }
        pathbuf.push(entry_name);
        tracing::debug!("path: {}", pathbuf.display());

        // look through basic file inodes in search of the one true basic_inode and extract the
        // bytes from the data and fragment sections
        for inode in &self.inodes {
            if let Inode::BasicFile(basic_file) = inode {
                if basic_file.header.inode_number == looking_inode as u32 {
                    return Ok((pathbuf, self.data(&basic_file)?));
                }
            }
        }

        Err(SquashfsError::FileNotFound)
    }
}

pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

/// Private struct containing logic to read the `Squashfs` section from a file
struct SquashfsReader {
    io: Box<dyn ReadSeek>,
}

impl SquashfsReader {
    /// Read in entire data and fragments
    #[instrument(skip_all)]
    pub fn data_and_fragments(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<Vec<u8>, SquashfsError> {
        //let option_size = superblock.compression_options_size() as usize;
        //self.io.seek(SeekFrom::Start(96 + option_size as u64))?;
        self.io.seek(SeekFrom::Start(0))?;
        let mut buf = vec![0u8; superblock.inode_table as usize];
        self.io.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    #[instrument(skip_all)]
    pub fn inodes(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<Vec<(usize, Inode)>, SquashfsError> {
        self.metadatas::<Inode>(
            superblock,
            SeekFrom::Start(superblock.inode_table),
            superblock.dir_table - superblock.inode_table,
        )
    }

    /// Parse multiple `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadatas<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        superblock: &SuperBlock,
        seek: SeekFrom,
        size: u64,
    ) -> Result<Vec<(usize, T)>, SquashfsError> {
        tracing::debug!("Metadata: seek {:02x?}, size: {:02x?}", seek, size);
        self.io.seek(seek)?;

        // The directory inodes store the total, uncompressed size of the entire listing, including headers.
        // Using this size, a SquashFS reader can determine if another header with further entries
        // should be following once it reaches the end of a run.

        // TODO: with capacity?
        let mut ret_bytes = vec![];
        let mut all_read = 0;

        while all_read <= size {
            // parse into metadata
            let mut buf = vec![0u8; (size - all_read) as usize];
            self.io.read_exact(&mut buf)?;
            if let Ok((_, m)) = Metadata::from_bytes((&buf, 0)) {
                // decompress
                let mut bytes = if Metadata::is_compressed(m.len) {
                    decompress(m.data, superblock.compressor)?
                } else {
                    m.data
                };

                all_read += bytes.len() as u64;
                ret_bytes.append(&mut bytes);
            }
        }

        // TODO: with capacity?
        let mut ret_vec = vec![];
        let mut total_read = 0;
        // Read until we fail to turn bytes into `T`, keeping track of position of read
        while let Ok(((rest, _), t)) = T::from_bytes((&ret_bytes, 0)) {
            // Push the new T to the return, with the position this was read from
            ret_vec.push((total_read, t));
            total_read += ret_bytes.len() - rest.len();
            ret_bytes = rest.to_vec();
        }

        Ok(ret_vec)
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    #[instrument(skip_all)]
    pub fn root_inode(
        &mut self,
        superblock: &SuperBlock,
        inodes: &[(usize, Inode)],
    ) -> Result<BasicDirectory, SquashfsError> {
        let (_, root_inode) = inodes
            .iter()
            .find(|(pos, _)| *pos == superblock.root_inode as usize)
            .unwrap();
        let root_inode = root_inode.expect_dir();
        Ok(root_inode.clone())
    }

    /// From `Vec<usize, Inode>` from `inodes`, return `Vec<Inode>`
    #[instrument(skip_all)]
    pub fn discard_pos(&mut self, inodes: &[(usize, Inode)]) -> Vec<Inode> {
        inodes.iter().cloned().map(|(_pos, inode)| inode).collect()
    }

    /// Parse required number of `Metadata`s uncompressed blocks required for `Dir`s
    #[instrument(skip_all)]
    pub fn dir_blocks(
        &mut self,
        superblock: &SuperBlock,
        inodes: &Vec<Inode>,
    ) -> Result<Vec<Vec<u8>>, SquashfsError> {
        let mut max_metadata = 0;
        for inode in inodes {
            if let Inode::BasicDirectory(basic_dir) = inode {
                if basic_dir.block_index > max_metadata {
                    max_metadata = basic_dir.block_index;
                }
            }
        }

        let offset = superblock.dir_table;
        let seek = SeekFrom::Start(offset);
        self.metadata_blocks(superblock, seek, u64::from(max_metadata) + 1, true)
    }

    /// Parse Fragment Table
    #[instrument(skip_all)]
    pub fn fragments(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<Option<Vec<Fragment>>, SquashfsError> {
        if superblock.frag_count == 0 {
            return Ok(None);
        }
        let fragment = self.lookup_table::<Fragment>(
            superblock,
            SeekFrom::Start(superblock.frag_table),
            u64::from(superblock.frag_count) * FRAGMENT_SIZE as u64,
        )?;

        Ok(Some(fragment))
    }
}

/// private
impl SquashfsReader {
    /// Parse Lookup Table
    #[instrument(skip_all)]
    fn lookup_table<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        superblock: &SuperBlock,
        seek: SeekFrom,
        size: u64,
    ) -> Result<Vec<T>, SquashfsError> {
        tracing::debug!(
            "Lookup Table: seek {:02x?}, metadata size: {:02x?}",
            seek,
            size
        );
        // find the pointer at the initial offset
        self.io.seek(seek)?;
        let mut buf = [0u8; 4];
        self.io.read_exact(&mut buf)?;
        let ptr = u32::from_le_bytes(buf);

        let block_count = (size as f32 / 8192_f32).ceil() as u64;

        self.metadata_with_count::<T>(
            superblock,
            SeekFrom::Start(u64::from(ptr)),
            block_count,
            false,
        )
    }

    /// Parse count of `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadata_with_count<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        superblock: &SuperBlock,
        seek: SeekFrom,
        count: u64,
        can_be_compressed: bool,
    ) -> Result<Vec<T>, SquashfsError> {
        tracing::debug!(
            "Metadata with count: seek {:02x?}, count: {:02x?}",
            seek,
            count
        );
        self.io.seek(seek)?;

        let mut all_bytes = vec![];
        // in order to grab a `count` of Metadatas, we can't use Deku for usage of std::io::Read
        for _ in 0..count {
            let mut buf = [0u8; 2];
            self.io.read_exact(&mut buf)?;
            let metadata_len = u16::from_le_bytes(buf);

            let byte_len = Metadata::len(metadata_len);
            let mut buf = vec![0u8; byte_len as usize];
            self.io.read_exact(&mut buf)?;

            let mut bytes = if can_be_compressed && Metadata::is_compressed(metadata_len) {
                decompress(buf, superblock.compressor)?
            } else {
                buf
            };
            all_bytes.append(&mut bytes);
        }

        // TODO: with capacity?
        let mut ret_vec = vec![];
        // Read until we fail to turn bytes into `T`
        while let Ok(((rest, _), t)) = T::from_bytes((&all_bytes, 0)) {
            ret_vec.push(t);
            all_bytes = rest.to_vec();
        }

        Ok(ret_vec)
    }

    /// Parse into Metadata uncompressed blocks
    #[instrument(skip_all)]
    fn metadata_blocks(
        &mut self,
        superblock: &SuperBlock,
        seek: SeekFrom,
        count: u64,
        can_be_compressed: bool,
    ) -> Result<Vec<Vec<u8>>, SquashfsError> {
        tracing::debug!("Seeking to 0x{seek:02x?}");
        self.io.seek(seek)?;

        let mut all_bytes = vec![];
        // in order to grab a `count` of Metadatas, we can't use Deku for usage of std::io::Read
        for _ in 0..count {
            let mut buf = [0u8; 2];
            self.io.read_exact(&mut buf)?;
            let metadata_len = u16::from_le_bytes(buf);

            let byte_len = Metadata::len(metadata_len);
            let mut buf = vec![0u8; byte_len as usize];
            self.io.read_exact(&mut buf)?;

            let bytes = if can_be_compressed && Metadata::is_compressed(metadata_len) {
                decompress(buf, superblock.compressor)?
            } else {
                buf
            };
            all_bytes.push(bytes);
        }

        Ok(all_bytes)
    }
}

/// Using the current compressor from the superblock, decompress bytes
#[instrument(skip_all)]
fn decompress(bytes: Vec<u8>, compressor: Compressor) -> Result<Vec<u8>, SquashfsError> {
    let mut out = vec![];
    match compressor {
        Compressor::Gzip => {
            let mut decoder = flate2::read::ZlibDecoder::new(std::io::Cursor::new(bytes));
            decoder.read_to_end(&mut out)?;
        },
        Compressor::Xz => {
            let mut decoder = XzDecoder::new(std::io::Cursor::new(bytes));
            decoder.read_to_end(&mut out)?;
        },
        _ => return Err(SquashfsError::UnsupportedCompression(compressor)),
    }
    Ok(out)
}
