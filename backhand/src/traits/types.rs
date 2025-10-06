use deku::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite, Default)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[deku(id_type = "u16")]
#[repr(u16)]
#[rustfmt::skip]
pub enum Compressor {
    None = 0,
    Gzip = 1,
    Lzma = 2,
    Lzo =  3,
    #[default]
    Xz =   4,
    Lz4 =  5,
    Zstd = 6,
}

impl From<crate::v4::compressor::Compressor> for Compressor {
    fn from(v4_compressor: crate::v4::compressor::Compressor) -> Self {
        match v4_compressor {
            crate::v4::compressor::Compressor::None => Compressor::None,
            crate::v4::compressor::Compressor::Gzip => Compressor::Gzip,
            crate::v4::compressor::Compressor::Lzma => Compressor::Lzma,
            crate::v4::compressor::Compressor::Lzo => Compressor::Lzo,
            crate::v4::compressor::Compressor::Xz => Compressor::Xz,
            crate::v4::compressor::Compressor::Lz4 => Compressor::Lz4,
            crate::v4::compressor::Compressor::Zstd => Compressor::Zstd,
        }
    }
}

impl From<Compressor> for crate::v4::compressor::Compressor {
    fn from(compressor: Compressor) -> Self {
        match compressor {
            Compressor::None => crate::v4::compressor::Compressor::None,
            Compressor::Gzip => crate::v4::compressor::Compressor::Gzip,
            Compressor::Lzma => crate::v4::compressor::Compressor::Lzma,
            Compressor::Lzo => crate::v4::compressor::Compressor::Lzo,
            Compressor::Xz => crate::v4::compressor::Compressor::Xz,
            Compressor::Lz4 => crate::v4::compressor::Compressor::Lz4,
            Compressor::Zstd => crate::v4::compressor::Compressor::Zstd,
        }
    }
}
