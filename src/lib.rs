use deku::bitvec::*;
use deku::ctx::Limit;
use deku::prelude::*;

fn read_offset<'a, C, T, O>(
    rest: &'a BitSlice<Msb0, u8>,
    input: &'a BitSlice<Msb0, u8>,
    ctx: C,
    offset: O,
) -> Result<(&'a BitSlice<Msb0, u8>, T), DekuError>
where
    C: Copy,
    T: DekuRead<'a, C>,
    O: Into<usize>,
{
    let offset = offset.into();
    let subslice = input.get(offset..).ok_or_else(|| {
        let need = NeedSize::new(rest.len() - offset);
        DekuError::Incomplete(need)
    })?;

    T::read(subslice, ctx)
}

fn read_offset_count<'a, C, T, U, O, N>(
    rest: &'a BitSlice<Msb0, u8>,
    input: &'a BitSlice<Msb0, u8>,
    ctx: C,
    offset: O,
    count: N,
) -> Result<(&'a BitSlice<Msb0, u8>, T), DekuError>
where
    C: Copy,
    T: DekuRead<'a, (Limit<U, fn(&U) -> bool>, C)>,
    U: Copy + DekuRead<'a, C>,
    O: Into<usize>,
    N: Into<usize>,
{
    read_offset(rest, input, (Limit::new_count(count.into()), ctx), offset)
}

#[derive(Copy, Clone, Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[deku(type = "u16")]
enum Compressor {
    None = 0,
    Gzip = 1,
    Lzo = 2,
    Lzma = 3,
    Xz = 4,
    Lz4 = 5,
    Zstd = 6,
}

enum Flags {
    InodesStoredUncompressed = 0b0000_0000_0000_0001,
    DataBlockStoredUncompressed = 0b0000_0000_0000_0010,
    Unused = 0b0000_0000_0000_0100,
    FragmentsStoredUncompressed = 0b0000_0000_0000_1000,
    FragmentsAreNotUsed = 0b0000_0000_0001_0000,
    FragmentsAreAlwaysGenerated = 0b0000_0000_0010_0000,
    DataHasBeenDeduplicated = 0b0000_0000_0100_0000,
    NFSExportTableExists = 0b0000_0000_1000_0000,
    XattrsAreStoredUncompressed = 0b0000_0001_0000_0000,
    NoXattrsInArchive = 0b0000_0010_0000_0000,
    CompressorOptionsArePresent = 0b0000_0100_0000_0000,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, compressor: Compressor"
)]
#[deku(id = "compressor")]
enum CompressionOptions {
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
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
struct Gzip {
    compression_level: u32,
    window_size: u16,
    // TODO: enum
    strategies: u16,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
struct Lzo {
    // TODO: enum
    algorithm: u32,
    compression_level: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
struct Xz {
    dictionary_size: u32,
    // TODO: enum
    filters: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
struct Lz4 {
    version: u32,
    //TODO: enum
    flags: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
struct Zstd {
    compression_level: u32,
}

#[derive(Debug, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct FileSystem {
    #[deku(assert_eq = "0x73717368")]
    magic: u32,
    inode_count: u32,
    mod_time: u32,
    block_size: u32,
    flag_count: u32,
    compressor: Compressor,
    block_log: u16,
    flags: u16,
    id_count: u16,
    version_major: u16,
    version_minor: u16,
    root_inode: u64,
    bytes_used: u64,
    pub id_table: u64,
    pub xattr_table: u64,
    pub inode_table: u64,
    pub dir_table: u64,
    pub frag_table: u64,
    pub export_table: u64,
    #[deku(
        reader = "read_offset_count(deku::rest, deku::input_bits, deku::ctx::Endian::Little, *inode_table as usize * 8, (*dir_table - *inode_table) as usize)"
    )]
    pub inode_metadata: Vec<u8>,
    #[deku(
        reader = "read_offset_count(deku::rest, deku::input_bits, deku::ctx::Endian::Little, *dir_table as usize * 8, (*frag_table - *dir_table) as usize)"
    )]
    pub dir_metadata: Vec<u8>,
    #[deku(
        reader = "read_offset_count(deku::rest, deku::input_bits, deku::ctx::Endian::Little, *frag_table as usize * 8, (*export_table - *frag_table) as usize)"
    )]
    pub frag_metadata: Vec<u8>,
}

impl FileSystem {
    pub fn parse_metadata(bytes: &[u8]) -> (u16, &[u8]) {
        println!("{:02x?}", &bytes);
        let len = u16::from_le_bytes([bytes[0], bytes[1]]);
        let bytes = &bytes[2..];
        (len, bytes)
    }
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
    blocks_start: u32,
    frag_index: u32,
    block_offset: u32,
    file_size: u32,
    #[deku(count = "file_size / 0x2000")]
    block_sizes: Vec<u32>,
}
