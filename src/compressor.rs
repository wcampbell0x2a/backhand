use deku::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[deku(type = "u16")]
pub enum Compressor {
    None = 0,
    Gzip = 1,
    Lzo  = 2,
    Lzma = 3,
    Xz   = 4,
    Lz4  = 5,
    Zstd = 6,
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
