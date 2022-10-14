use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use deku::prelude::*;
use tracing::instrument;
use xz2::read::XzDecoder;

const FRAGMENT_SIZE: usize =
    std::mem::size_of::<u64>() + std::mem::size_of::<u32>() + std::mem::size_of::<u32>();

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Fragment {
    pub start: u64,
    pub size: u32,
    pub unused: u32,
}

impl Fragment {
    pub fn uncompressed_size(num: u32) -> u32 {
        1 << 24 | num
    }
}

#[derive(Copy, Clone, Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[deku(type = "u16")]
enum Compressor {
    None = 0,
    Gzip = 1,
    Lzo  = 2,
    Lzma = 3,
    Xz   = 4,
    Lz4  = 5,
    Zstd = 6,
}

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

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, compressor: Compressor"
)]
#[deku(id = "compressor")]
pub enum CompressionOptions {
    #[deku(id = "Compressor::Gzip")]
    Gzip(Gzip),

    #[deku(id = "Compressor::Lzo")]
    Lzo(Lzo),

    #[deku(id = "Compressor::Xz")]
    Xz(Xz),

    #[deku(id = "Compressor::Lz4")]
    Lz4(Lz4),

    #[deku(id = "Compressor::Zstd")]
    Zstd(Zstd),

    #[deku(id = "Compressor::Lzma")]
    Lzma,
}

impl CompressionOptions {
    pub fn size(&self) -> u64 {
        match self {
            Self::Gzip(_) => 8,
            Self::Lzo(_) => 8,
            Self::Xz(_) => 8,
            Self::Lz4(_) => 8,
            Self::Zstd(_) => 4,
            Self::Lzma => 0,
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Gzip {
    compression_level: u32,
    window_size: u16,
    // TODO: enum
    strategies: u16,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Lzo {
    // TODO: enum
    algorithm: u32,
    compression_level: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Xz {
    dictionary_size: u32,
    // TODO: enum
    filters: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Lz4 {
    version: u32,
    //TODO: enum
    flags: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Zstd {
    compression_level: u32,
}

const METADATA_COMPRESSED: u16 = 1 << 15;

#[derive(Debug, DekuRead, DekuWrite)]
pub struct Metadata {
    // TODO; use deku to parse METADATA_COMPRESSED?
    len: u16,
    #[deku(count = "Self::len(*len)")]
    pub data: Vec<u8>,
}

impl Metadata {
    /// Check is_compressed bit within raw `len`
    pub fn is_compressed(len: u16) -> bool {
        len & METADATA_COMPRESSED == 0
    }

    /// Get actual length of `data` following `len` from unedited `len`
    pub fn len(len: u16) -> u16 {
        len & !(METADATA_COMPRESSED)
    }
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
    pub fn from_reader<R: ReadSeek + 'static>(mut reader: R) -> Self {
        // Size of metadata + optional compression options metadata block
        let mut superblock = [0u8; 96 + 8];
        reader.read_exact(&mut superblock).unwrap();

        let (_, superblock) = SuperBlock::from_bytes((&superblock, 0)).unwrap();

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

        Self {
            io: Box::new(reader),
            superblock,
            compression_options: None,
        }
    }

    /// Parse Inodes
    #[instrument(skip_all)]
    pub fn inodes(&mut self) -> Vec<(usize, Inode)> {
        self.metadatas::<Inode>(
            SeekFrom::Start(self.superblock.inode_table),
            self.superblock.dir_table - self.superblock.inode_table,
        )
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    #[instrument(skip_all)]
    pub fn root_inode(&mut self, inodes: &Vec<(usize, Inode)>) -> BasicDirectory {
        let (_, root_inode) = inodes
            .iter()
            .find(|(pos, inode)| *pos == self.superblock.root_inode as usize)
            .unwrap();
        let root_inode = root_inode.expect_dir();
        root_inode.clone()
    }

    /// From `Vec<usize, Inode>`, give `Vec<Inode>
    #[instrument(skip_all)]
    pub fn discard_pos(&mut self, inodes: &Vec<(usize, Inode)>) -> Vec<Inode> {
        inodes
            .clone()
            .into_iter()
            .map(|(_pos, inode)| inode)
            .collect()
    }

    /// Parse required number of `Metadata`s uncompressed blocks required for `Dir`s
    #[instrument(skip_all)]
    pub fn dir_blocks(&mut self, inodes: &Vec<Inode>) -> Vec<Vec<u8>> {
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
    pub fn fragments(&mut self) -> Option<Vec<Fragment>> {
        if self.superblock.frag_count == 0 {
            return None;
        }
        let fragment = self.lookup_table::<Fragment>(
            SeekFrom::Start(self.superblock.frag_table),
            self.superblock.frag_count as u64 * FRAGMENT_SIZE as u64,
        );

        Some(fragment)
    }

    /// Give a file_name from a squashfs filepath, extract the file and the filepath
    #[instrument(skip_all)]
    pub fn extract_file(
        &mut self,
        name: &str,
        dir_blocks: &Vec<Vec<u8>>,
        inodes: &[Inode],
        fragments: &Option<Vec<Fragment>>,
        root_inode: &BasicDirectory,
    ) -> (PathBuf, Vec<u8>) {
        let mut found_directory = None;
        // Search through inodes and parse directory table at the specified location
        // searching for first file name that matches
        'outer: for inode in inodes {
            tracing::trace!("Searching following inode for filename: {inode:#02x?}");
            if let Inode::BasicDirectory(basic_dir) = inode {
                let block = &dir_blocks[basic_dir.block_index as usize];
                let bytes = &block[basic_dir.block_offset as usize..];
                let (_, dir) = Dir::from_bytes((bytes, 0)).unwrap();
                tracing::trace!("Searching following dir for filename: {dir:#02x?}");
                for entry in dir.dir_entries {
                    let entry_name = std::str::from_utf8(&entry.name).unwrap();
                    tracing::debug!(entry_name);
                    if name == entry_name {
                        found_directory = Some((inode, dir.inode_num, entry));
                        break 'outer;
                    }
                }
            }
        }
        tracing::debug!("found matching inode/dir: {found_directory:#02x?}");

        // We now have the:
        // (
        //     directory inode matching the filename,
        //     base_inode(directory base inode_num)
        //     entry from dir.entires that matches the filename
        //  )
        let (dir_inode, base_inode, entry) = found_directory.unwrap();
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

            let (_, dir) = Dir::from_bytes((bytes, 0)).unwrap();
            let base_inode = dir.inode_num;

            for entry in &dir.dir_entries {
                let entry_name = std::str::from_utf8(&entry.name).unwrap();
                if base_inode as i16 + entry.inode_offset == search_inode as i16 {
                    let entry_name = String::from_utf8(entry.name.clone()).unwrap();
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
        pathbuf.push(name);
        tracing::debug!("path: {}", pathbuf.display());

        // look through basic file inodes in search of the one true basic_inode and extract the
        // bytes from the data and fragment sections
        for inode in inodes {
            if let Inode::BasicFile(basic_file) = inode {
                if basic_file.header.inode_number == looking_inode as u32 {
                    return (pathbuf, self.data(basic_file, fragments));
                }
            }
        }
        todo!("file not found, did you give me a dir name?");
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
    ) -> Vec<T> {
        tracing::debug!(
            "Lookup Table: seek {:02x?}, metadata size: {:02x?}",
            seek,
            size
        );
        // find the pointer at the initial offset
        self.io.seek(seek).unwrap();
        let mut buf = [0u8; 4];
        self.io.read_exact(&mut buf).unwrap();
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
    ) -> Vec<(usize, T)> {
        tracing::debug!("Metadata: seek {:02x?}, size: {:02x?}", seek, size);
        self.io.seek(seek).unwrap();

        // The directory inodes store the total, uncompressed size of the entire listing, including headers.
        // Using this size, a SquashFS reader can determine if another header with further entries
        // should be following once it reaches the end of a run.

        // TODO: with capacity?
        let mut ret_bytes = vec![];
        let mut all_read = 0;

        while all_read <= size {
            // parse into metadata
            let mut buf = vec![0u8; (size - all_read) as usize];
            self.io.read_exact(&mut buf).unwrap();
            if let Ok((_, m)) = Metadata::from_bytes((&buf, 0)) {
                // decompress
                let mut bytes = if Metadata::is_compressed(m.len) {
                    self.decompress(m.data)
                } else {
                    m.data
                };

                all_read += bytes.len() as u64;
                ret_bytes.append(&mut bytes);
            } else {
                break;
            }
        }
        //println!("{:02x?}", ret_bytes);

        // TODO: with capacity?
        let mut ret_vec = vec![];
        let mut total_read = 0;
        // TODO: this can be calculate better w.r.t the length of these bytes and the failure.
        loop {
            match T::from_bytes((&ret_bytes, 0)) {
                Ok(((rest, _), t)) => {
                    // Push the new T to the return, with the position this was read from
                    ret_vec.push((total_read, t));
                    total_read += ret_bytes.len() - rest.len();
                    ret_bytes = rest.to_vec();
                },
                Err(_) => {
                    break;
                },
            }
        }

        ret_vec
    }

    /// Parse count of `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadata_with_count<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        seek: SeekFrom,
        count: u64,
        can_be_compressed: bool,
    ) -> Vec<T> {
        tracing::debug!(
            "Metadata with count: seek {:02x?}, count: {:02x?}",
            seek,
            count
        );
        self.io.seek(seek).unwrap();

        let mut all_bytes = vec![];
        // in order to grab a `count` of Metadatas, we can't use Deku for usage of std::io::Read
        for _ in 0..count {
            let mut buf = [0u8; 2];
            self.io.read_exact(&mut buf).unwrap();
            let metadata_len = u16::from_le_bytes(buf);

            let byte_len = Metadata::len(metadata_len);
            let mut buf = vec![0u8; byte_len as usize];
            self.io.read_exact(&mut buf).unwrap();

            let mut bytes = if can_be_compressed && Metadata::is_compressed(metadata_len) {
                self.decompress(buf)
            } else {
                buf
            };
            all_bytes.append(&mut bytes);
        }

        // TODO: with capacity?
        let mut ret_vec = vec![];
        // TODO: this can be calculate better w.r.t the length of these bytes and the failure.
        loop {
            match T::from_bytes((&all_bytes, 0)) {
                Ok(((rest, _), t)) => {
                    ret_vec.push(t);
                    all_bytes = rest.to_vec();
                },
                Err(_) => {
                    break;
                },
            }
        }

        ret_vec
    }

    /// Parse into Metadata uncompressed blocks
    #[instrument(skip_all)]
    fn metadata_blocks(
        &mut self,
        seek: SeekFrom,
        count: u64,
        can_be_compressed: bool,
    ) -> Vec<Vec<u8>> {
        tracing::debug!("Seeking to 0x{seek:02x?}");
        self.io.seek(seek).unwrap();

        let mut all_bytes = vec![];
        // in order to grab a `count` of Metadatas, we can't use Deku for usage of std::io::Read
        for _ in 0..count {
            let mut buf = [0u8; 2];
            self.io.read_exact(&mut buf).unwrap();
            let metadata_len = u16::from_le_bytes(buf);

            let byte_len = Metadata::len(metadata_len);
            let mut buf = vec![0u8; byte_len as usize];
            self.io.read_exact(&mut buf).unwrap();

            let bytes = if can_be_compressed && Metadata::is_compressed(metadata_len) {
                self.decompress(buf)
            } else {
                buf
            };
            all_bytes.push(bytes);
        }

        all_bytes
    }

    #[instrument(skip_all)]
    fn data(&mut self, basic_file: &BasicFile, fragments: &Option<Vec<Fragment>>) -> Vec<u8> {
        tracing::debug!("extracting: {basic_file:#02x?}");
        let start_of_data = basic_file.blocks_start as u64;

        // seek to start of data
        self.io.seek(SeekFrom::Start(start_of_data)).unwrap();

        // Add data
        let mut data_bytes = vec![];
        for block_size in basic_file.block_sizes.iter() {
            // TODO: use deku for this?
            let uncompressed = block_size & (1 << 24) != 0;
            let size = block_size & !(1 << 24);
            let mut data = vec![0u8; size as usize];
            self.io.read_exact(&mut data).unwrap();

            let mut bytes = if uncompressed {
                data
            } else {
                self.decompress(data)
            };
            data_bytes.append(&mut bytes);
        }

        // Add fragments
        // TODO: this should be constant
        if basic_file.frag_index != 0xffffffff {
            if let Some(fragments) = fragments {
                let frag = fragments[basic_file.frag_index as usize];
                self.io.seek(SeekFrom::Start(frag.start)).unwrap();

                let uncompressed = frag.size & (1 << 24) != 0;
                let size = frag.size & !(1 << 24);

                let mut buf = vec![0u8; size as usize];
                self.io.read_exact(&mut buf).unwrap();

                let mut bytes = if uncompressed {
                    buf
                } else {
                    self.decompress(buf)
                };
                data_bytes.append(&mut bytes);
            }
        }

        data_bytes = data_bytes[basic_file.block_offset as usize..]
            [..basic_file.file_size as usize]
            .to_vec();

        data_bytes
    }

    /// Using the current compressor from the superblock, decompress bytes
    #[instrument(skip_all)]
    fn decompress(&self, bytes: Vec<u8>) -> Vec<u8> {
        let mut out = vec![];
        match self.superblock.compressor {
            Compressor::Gzip => {
                let mut decoder = flate2::read::ZlibDecoder::new(std::io::Cursor::new(bytes));
                decoder.read_to_end(&mut out).unwrap();
            },
            Compressor::Xz => {
                let mut decoder = XzDecoder::new(std::io::Cursor::new(bytes));
                decoder.read_to_end(&mut out).unwrap();
            },
            _ => todo!(),
        }
        out
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

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(type = "u16")]
#[deku(endian = "little")]
pub enum Inode {
    #[deku(id = "1")]
    BasicDirectory(BasicDirectory),

    #[deku(id = "2")]
    BasicFile(BasicFile),
}

impl Inode {
    pub fn expect_dir(&self) -> &BasicDirectory {
        if let Self::BasicDirectory(basic_dir) = self {
            basic_dir
        } else {
            panic!("not a dir");
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct InodeHeader {
    permissions: u16,
    uid: u16,
    gid: u16,
    mtime: u32,
    inode_number: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicDirectory {
    header: InodeHeader,
    block_index: u32,
    link_count: u32,
    file_size: u16,
    block_offset: u16,
    parent_inode: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicFile {
    pub header: InodeHeader,
    blocks_start: u32,
    frag_index: u32,
    block_offset: u32,
    file_size: u32,
    #[deku(count = "Self::count(*frag_index, *file_size)")]
    block_sizes: Vec<u32>,
}

impl BasicFile {
    fn count(fragment: u32, file_size: u32) -> u32 {
        const NO_FRAGMENT: u32 = 0xffffffff;

        // !!! TODO: this _needs_ to be from the superblock !!!
        const BLOCK_SIZE: u64 = 0x20000_u64;
        // !!! TODO: this _needs_ to be from the superblock !!!
        const BLOCK_LOG: u64 = 0x11;

        if fragment == NO_FRAGMENT {
            ((file_size as u64 + BLOCK_SIZE - 1) >> BLOCK_LOG) as u32
        } else {
            file_size >> BLOCK_LOG
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Dir {
    pub count: u32,
    pub start: u32,
    pub inode_num: u32,
    #[deku(count = "*count + 1")]
    pub dir_entries: Vec<DirEntry>,
}

// TODO: derive our own Debug, with name()
#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct DirEntry {
    pub offset: u16,
    pub inode_offset: i16,
    pub t: u16,
    pub name_size: u16,

    // TODO: CString
    #[deku(count = "*name_size + 1")]
    pub name: Vec<u8>,
}

impl DirEntry {
    pub fn name(&self) -> String {
        std::str::from_utf8(&self.name).unwrap().to_string()
    }
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Frag {
    start: u64,
    size: u32,
    unused: u32,
}
