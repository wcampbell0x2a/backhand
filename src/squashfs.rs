use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use deku::prelude::*;
use tracing::instrument;
use xz2::read::XzDecoder;

use crate::error::SquashfsError;
use crate::fragment::FRAGMENT_SIZE;
use crate::{
    BasicDirectory, BasicFile, CompressionOptions, Compressor, Dir, DirEntry, Fragment, Inode,
    Metadata,
};

enum Flags {
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

pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

pub struct Squashfs {
    io: Box<dyn ReadSeek>,
    pub superblock: SuperBlock,
    pub compression_options: Option<CompressionOptions>,
}

/// public
impl Squashfs {
    #[instrument(skip_all)]
    pub fn from_reader<R: ReadSeek + 'static>(mut reader: R) -> Result<Self, SquashfsError> {
        // Size of metadata + optional compression options metadata block
        let mut superblock = [0u8; 96 + 8];
        reader.read_exact(&mut superblock)?;

        let (_, superblock) = SuperBlock::from_bytes((&superblock, 0))?;

        // TODO: parse compression options for compression
        //let compression_options = if superblock.compressor != Compressor::None {
        //    // into metadata first
        //    println!("{:02x?}", rest.0);
        //    let bv = BitVec::from_slice(rest.0).unwrap();
        //    let (_, m) = Metadata::read(&bv, ()).unwrap();

        //    // data -> compression options
        //    let bv = BitVec::from_slice(&m.data).unwrap();
        //    let (_, c) =
        //        CompressionOptions::read(&bv, (deku::ctx::Endian::Little, superblock.compressor))
        //            .unwrap();
        //    Some(c)
        //} else {
        //    None
        //};

        Ok(Self {
            io: Box::new(reader),
            superblock,
            compression_options: None,
        })
    }

    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    #[instrument(skip_all)]
    pub fn inodes(&mut self) -> Result<Vec<(usize, Inode)>, SquashfsError> {
        self.metadatas::<Inode>(
            SeekFrom::Start(self.superblock.inode_table),
            self.superblock.dir_table - self.superblock.inode_table,
        )
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    #[instrument(skip_all)]
    pub fn root_inode(
        &mut self,
        inodes: &[(usize, Inode)],
    ) -> Result<BasicDirectory, SquashfsError> {
        let (_, root_inode) = inodes
            .iter()
            .find(|(pos, _)| *pos == self.superblock.root_inode as usize)
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
    pub fn dir_blocks(&mut self, inodes: &Vec<Inode>) -> Result<Vec<Vec<u8>>, SquashfsError> {
        let mut max_metadata = 0;
        for inode in inodes {
            if let Inode::BasicDirectory(basic_dir) = inode {
                if basic_dir.block_index > max_metadata {
                    max_metadata = basic_dir.block_index;
                }
            }
        }

        let offset = self.superblock.dir_table;
        let seek = SeekFrom::Start(offset);
        self.metadata_blocks(seek, u64::from(max_metadata) + 1, true)
    }

    /// Parse Fragment Table
    #[instrument(skip_all)]
    pub fn fragments(&mut self) -> Result<Option<Vec<Fragment>>, SquashfsError> {
        if self.superblock.frag_count == 0 {
            return Ok(None);
        }
        let fragment = self.lookup_table::<Fragment>(
            SeekFrom::Start(self.superblock.frag_table),
            u64::from(self.superblock.frag_count) * FRAGMENT_SIZE as u64,
        )?;

        Ok(Some(fragment))
    }

    /// Extract all files
    pub fn extract_all_files(
        &mut self,
        dir_blocks: &[Vec<u8>],
        inodes: &[Inode],
        fragments: &Option<Vec<Fragment>>,
        root_inode: &BasicDirectory,
    ) -> Result<Vec<(PathBuf, Vec<u8>)>, SquashfsError> {
        let mut ret = vec![];
        for inode in inodes {
            if let Inode::BasicDirectory(basic_dir) = inode {
                let block = &dir_blocks[basic_dir.block_index as usize];
                let bytes = &block[basic_dir.block_offset as usize..];
                let (_, dir) = Dir::from_bytes((bytes, 0))?;
                for entry in &dir.dir_entries {
                    // TODO: use type enum
                    if entry.t == 2 {
                        tracing::debug!("{dir:?}");
                        ret.push(self.file(
                            inode,
                            dir.inode_num,
                            entry,
                            dir_blocks,
                            inodes,
                            fragments,
                            root_inode,
                        )?);
                    }
                }
            }
        }

        Ok(ret)
    }

    /// Given a file_name from a squashfs filepath, extract the file and the filepath
    #[instrument(skip_all)]
    pub fn extract_file(
        &mut self,
        name: &str,
        dir_blocks: &[Vec<u8>],
        inodes: &[Inode],
        fragments: &Option<Vec<Fragment>>,
        root_inode: &BasicDirectory,
    ) -> Result<(PathBuf, Vec<u8>), SquashfsError> {
        // Search through inodes and parse directory table at the specified location
        // searching for first file name that matches
        for inode in inodes {
            tracing::trace!("Searching following inode for filename: {inode:#02x?}");
            if let Inode::BasicDirectory(basic_dir) = inode {
                let block = &dir_blocks[basic_dir.block_index as usize];
                let bytes = &block[basic_dir.block_offset as usize..];
                let (_, dir) = Dir::from_bytes((bytes, 0))?;
                tracing::trace!("Searching following dir for filename: {dir:#02x?}");
                for entry in &dir.dir_entries {
                    // TODO: use type enum
                    if entry.t == 2 {
                        let entry_name = std::str::from_utf8(&entry.name)?;
                        tracing::debug!(entry_name);
                        if name == entry_name {
                            let file = self.file(
                                inode,
                                dir.inode_num,
                                entry,
                                dir_blocks,
                                inodes,
                                fragments,
                                root_inode,
                            )?;
                            return Ok(file);
                        }
                    }
                }
            }
        }
        Err(SquashfsError::FileNotFound)
    }
}

/// private
impl Squashfs {
    /// Parse Lookup Table
    #[instrument(skip_all)]
    fn lookup_table<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
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

        self.metadata_with_count::<T>(SeekFrom::Start(u64::from(ptr)), block_count, false)
    }

    /// Parse multiple `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadatas<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
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
                    self.decompress(m.data)?
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

    /// Parse count of `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadata_with_count<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
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
                self.decompress(buf)?
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
                self.decompress(buf)?
            } else {
                buf
            };
            all_bytes.push(bytes);
        }

        Ok(all_bytes)
    }

    #[instrument(skip_all)]
    fn data(
        &mut self,
        basic_file: &BasicFile,
        fragments: &Option<Vec<Fragment>>,
    ) -> Result<Vec<u8>, SquashfsError> {
        tracing::debug!("extracting: {basic_file:#02x?}");
        let start_of_data = u64::from(basic_file.blocks_start);

        // seek to start of data
        self.io.seek(SeekFrom::Start(start_of_data))?;

        // Add data
        let mut data_bytes = vec![];
        for block_size in &basic_file.block_sizes {
            // TODO: use deku for this?
            let uncompressed = block_size & (1 << 24) != 0;
            let size = block_size & !(1 << 24);
            let mut data = vec![0u8; size as usize];
            self.io.read_exact(&mut data)?;

            let mut bytes = if uncompressed {
                data
            } else {
                self.decompress(data)?
            };
            data_bytes.append(&mut bytes);
        }

        // Add fragments
        // TODO: this should be constant
        if basic_file.frag_index != 0xffffffff {
            if let Some(fragments) = fragments {
                let frag = fragments[basic_file.frag_index as usize];
                self.io.seek(SeekFrom::Start(frag.start))?;

                let uncompressed = frag.size & (1 << 24) != 0;
                let size = frag.size & !(1 << 24);

                let mut buf = vec![0u8; size as usize];
                self.io.read_exact(&mut buf)?;

                let mut bytes = if uncompressed {
                    buf
                } else {
                    self.decompress(buf)?
                };
                data_bytes.append(&mut bytes);
            }
        }

        data_bytes = data_bytes[basic_file.block_offset as usize..]
            [..basic_file.file_size as usize]
            .to_vec();

        Ok(data_bytes)
    }

    /// Using the current compressor from the superblock, decompress bytes
    #[instrument(skip_all)]
    fn decompress(&self, bytes: Vec<u8>) -> Result<Vec<u8>, SquashfsError> {
        let mut out = vec![];
        match self.superblock.compressor {
            Compressor::Gzip => {
                let mut decoder = flate2::read::ZlibDecoder::new(std::io::Cursor::new(bytes));
                decoder.read_to_end(&mut out)?;
            },
            Compressor::Xz => {
                let mut decoder = XzDecoder::new(std::io::Cursor::new(bytes));
                decoder.read_to_end(&mut out)?;
            },
            _ => {
                return Err(SquashfsError::UnsupportedCompression(
                    self.superblock.compressor,
                ))
            },
        }
        Ok(out)
    }

    #[instrument(skip_all)]
    fn file(
        &mut self,
        found_inode: &Inode,
        found_inode_num: u32,
        found_entry: &DirEntry,
        dir_blocks: &[Vec<u8>],
        inodes: &[Inode],
        fragments: &Option<Vec<Fragment>>,
        root_inode: &BasicDirectory,
    ) -> Result<(PathBuf, Vec<u8>), SquashfsError> {
        //  TODO: remove
        let dir_inode = found_inode;
        let base_inode = found_inode_num;
        let entry = found_entry;
        let entry_name = std::str::from_utf8(&entry.name)?;
        let dir_inode = dir_inode.expect_dir();
        // Used for searching for the file later when we want to extract the bytes
        let looking_inode = base_inode as i16 + entry.inode_offset;

        let root_inode = root_inode.header.inode_number;
        tracing::debug!("searching for dir path to root inode: {:#02x?}", root_inode);

        let mut path_inodes = vec![];
        // first check if the dir inode of this file is the root inode
        if dir_inode.header.inode_number != root_inode {
            // check every inode and find the matching inode to the current dir_nodes parent
            // inode, when we find a new inode, check if it's the root inode, and continue on if it
            // isn't
            let mut next_inode = dir_inode.parent_inode;
            'outer: loop {
                for inode in inodes {
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
            let block = &dir_blocks[basic_dir_with_location.block_index as usize];
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
        for inode in inodes {
            if let Inode::BasicFile(basic_file) = inode {
                if basic_file.header.inode_number == looking_inode as u32 {
                    return Ok((pathbuf, self.data(basic_file, fragments)?));
                }
            }
        }

        Err(SquashfsError::FileNotFound)
    }
}

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
    flags: u16,
    id_count: u16,
    version_major: u16,
    version_minor: u16,
    pub root_inode: u64,
    bytes_used: u64,
    pub id_table: u64,
    pub xattr_table: u64,
    pub inode_table: u64,
    pub dir_table: u64,
    pub frag_table: u64,
    pub export_table: u64,
}
