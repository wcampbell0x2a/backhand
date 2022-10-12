use std::io::{Read, Seek, SeekFrom};

use deku::prelude::*;
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
    pub fn is_compressed(len: u16) -> bool {
        len & METADATA_COMPRESSED == 0
    }

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

    /// Parse Directory Table
    pub fn dirs(&mut self) -> Vec<Dir> {
        let offset = self.superblock.dir_table;
        let seek = SeekFrom::Start(offset);
        let size = self.superblock.frag_table - offset;

        self.metadatas::<Dir>(seek, size)
    }

    /// Parse Inode Table
    pub fn inodes(&mut self) -> Vec<Inode> {
        self.metadatas::<Inode>(
            SeekFrom::Start(self.superblock.inode_table),
            self.superblock.dir_table - self.superblock.inode_table,
        )
    }

    /// Parse Fragment Table
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

    /// Extract file from squashfs filesystem
    ///
    /// Extract file from a already parsed Directory Table and Inode table from a squashfs
    /// filesystem. This doesn't respect filepaths, and instead just gives you the bytes
    /// representing that file.
    // TODO: for now, this requires you to have ALL the Inodes already deserialized, but the
    // Dir/BasicFile gives information as to where the inode is located, so use that in the future
    // instead of passing in `inodes`
    pub fn extract_file(
        &mut self,
        name: &str,
        dirs: &[Dir],
        inodes: &[Inode],
        fragments: &Option<Vec<Fragment>>,
    ) -> Vec<u8> {
        // search through dirs for file name that matches
        let mut found_directory = None;
        for dir in dirs {
            for entry in &dir.dir_entries {
                if name == std::str::from_utf8(&entry.name).unwrap() {
                    found_directory = Some((dir.inode_num, entry));
                }
            }
        }

        // TODO: exit nicely in the future
        let (base_inode, entry) = found_directory.unwrap();
        let looking_inode = base_inode as i16 + entry.inode_offset;

        // look through basic file inodes in search of the one true basic_inode
        for inode in inodes {
            if let Inode::BasicFile(basic_file) = inode {
                if basic_file.header.inode_number == looking_inode as u32 {
                    return self.data(basic_file, fragments);
                }
            }
        }
        todo!();
    }
}

/// private
impl Squashfs {
    /// Parse Lookup Table
    fn lookup_table<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        seek: SeekFrom,
        size: u64,
    ) -> Vec<T> {
        println!(
            "Lookup Table: seek {:02x?}, metadata size: {:02x?}",
            seek, size
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
    fn metadatas<T: for<'a> DekuContainerRead<'a>>(&mut self, seek: SeekFrom, size: u64) -> Vec<T> {
        println!("Metadata: seek {:02x?}, size: {:02x?}", seek, size);
        self.io.seek(seek).unwrap();

        // TODO: with capacity?
        let mut ret_bytes = vec![];
        let mut all_read = 0;

        pub const METADATA_SIZE: usize = 8 * 1024;
        while all_read <= size {
            // parse into metadata
            println!("{:02x?}", size);
            let mut buf = vec![0u8; size as usize];
            self.io.read_exact(&mut buf).unwrap();
            all_read += METADATA_SIZE as u64;
            if let Ok((_, m)) = Metadata::from_bytes((&buf, 0)) {
                //println!("Metadata: {m:?}");

                // decompress
                let mut bytes = if Metadata::is_compressed(m.len) {
                    self.decompress(m.data)
                } else {
                    m.data
                };
                ret_bytes.append(&mut bytes);
            } else {
                break;
            }
        }
        //println!("{:02x?}", ret_bytes);

        // TODO: with capacity?
        let mut ret_vec = vec![];
        // TODO: this can be calculate better w.r.t the length of these bytes and the failure.
        loop {
            match T::from_bytes((&ret_bytes, 0)) {
                Ok(((rest, _), t)) => {
                    ret_vec.push(t);
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
    fn metadata_with_count<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        seek: SeekFrom,
        count: u64,
        can_be_compressed: bool,
    ) -> Vec<T> {
        println!(
            "Metadata with count: seek {:02x?}, count: {:02x?}",
            seek, count
        );
        self.io.seek(seek).unwrap();

        let mut all_bytes = vec![];
        // in order to grab a `count` of Metadatas, we can't use Deku for usage of std::io::Read
        for _ in 0..count {
            let mut buf = [0u8; 2];
            self.io.read_exact(&mut buf).unwrap();

            let len = Metadata::len(u16::from_le_bytes(buf));
            let mut buf = vec![0u8; len as usize];
            self.io.read_exact(&mut buf).unwrap();

            let mut bytes = if can_be_compressed && Metadata::is_compressed(len) {
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

    fn data(&mut self, basic_file: &BasicFile, fragments: &Option<Vec<Fragment>>) -> Vec<u8> {
        let start_of_data = basic_file.blocks_start as u64;
        //println!("Data: seek_start: {start_of_data:02x?}");

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

        data_bytes
    }

    /// Using the current compressor from the superblock, decompress bytes
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

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(type = "u16")]
#[deku(endian = "little")]
pub enum Inode {
    #[deku(id = "1")]
    BasicDirectory(BasicDirectory),

    #[deku(id = "2")]
    BasicFile(BasicFile),
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct InodeHeader {
    permissions: u16,
    uid: u16,
    gid: u16,
    mtime: u32,
    inode_number: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicDirectory {
    header: InodeHeader,
    block_index: u32,
    link_count: u32,
    file_size: u16,
    block_offset: u16,
    parent_inode: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
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
