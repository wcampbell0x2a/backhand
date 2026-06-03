/// Version-independent compression algorithm identifier
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[rustfmt::skip]
pub enum Compressor {
    /// No compression
    Uncompressed = 0,
    /// Gzip (zlib) compression
    Gzip = 1,
    /// LZMA compression
    Lzma = 2,
    /// LZO compression
    Lzo =  3,
    /// XZ compression (default)
    #[default]
    Xz =   4,
    /// LZ4 compression
    Lz4 =  5,
    /// Zstandard compression
    Zstd = 6,
}

impl From<crate::v4::compressor::Compressor> for Compressor {
    fn from(v4_compressor: crate::v4::compressor::Compressor) -> Self {
        match v4_compressor {
            crate::v4::compressor::Compressor::Uncompressed => Compressor::Uncompressed,
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
            Compressor::Uncompressed => crate::v4::compressor::Compressor::Uncompressed,
            Compressor::Gzip => crate::v4::compressor::Compressor::Gzip,
            Compressor::Lzma => crate::v4::compressor::Compressor::Lzma,
            Compressor::Lzo => crate::v4::compressor::Compressor::Lzo,
            Compressor::Xz => crate::v4::compressor::Compressor::Xz,
            Compressor::Lz4 => crate::v4::compressor::Compressor::Lz4,
            Compressor::Zstd => crate::v4::compressor::Compressor::Zstd,
        }
    }
}
