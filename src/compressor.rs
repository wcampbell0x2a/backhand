//! Types of supported compression algorithms

use std::io::{Cursor, Read};

use deku::prelude::*;
#[cfg(feature = "gzip")]
use flate2::read::ZlibEncoder;
#[cfg(feature = "gzip")]
use flate2::Compression;
use tracing::instrument;
#[cfg(feature = "xz")]
use xz2::read::{XzDecoder, XzEncoder};
#[cfg(feature = "xz")]
use xz2::stream::{Check, Filters, LzmaOptions, MtStreamBuilder};

use crate::error::SquashfsError;
use crate::filesystem::writer::{CompressionExtra, FilesystemCompressor};

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite, Default)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[deku(type = "u16")]
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

#[derive(Debug, DekuRead, DekuWrite, PartialEq, Eq, Clone, Copy)]
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

#[derive(Debug, DekuRead, DekuWrite, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Gzip {
    pub compression_level: u32,
    pub window_size: u16,
    // TODO: enum
    pub strategies: u16,
}

#[derive(Debug, DekuRead, DekuWrite, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Lzo {
    // TODO: enum
    pub algorithm: u32,
    pub compression_level: u32,
}

#[derive(Debug, DekuRead, DekuWrite, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Xz {
    pub dictionary_size: u32,
    pub filters: XzFilter,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[deku(type = "u32")]
#[rustfmt::skip]
pub enum XzFilter {
    X86      = 0x01,
    PowerPC  = 0x02,
    IA64     = 0x04,
    Arm      = 0x08,
    ArmThumb = 0x10,
    Sparc    = 0x20,
}

#[derive(Debug, DekuRead, DekuWrite, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Lz4 {
    pub version: u32,
    //TODO: enum
    pub flags: u32,
}

#[derive(Debug, DekuRead, DekuWrite, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Zstd {
    pub compression_level: u32,
}

/// Using the current compressor from the superblock, decompress bytes
#[instrument(skip_all)]
pub(crate) fn decompress(
    bytes: &[u8],
    out: &mut Vec<u8>,
    compressor: Compressor,
) -> Result<(), SquashfsError> {
    match compressor {
        #[cfg(feature = "gzip")]
        Compressor::Gzip => {
            let mut decoder = flate2::read::ZlibDecoder::new(bytes);
            decoder.read_to_end(out)?;
        },
        #[cfg(feature = "xz")]
        Compressor::Xz => {
            let mut decoder = XzDecoder::new(bytes);
            decoder.read_to_end(out)?;
        },
        #[cfg(feature = "lzo")]
        Compressor::Lzo => {
            out.resize(out.capacity(), 0);
            let (out_size, error) = rust_lzo::LZOContext::decompress_to_slice(bytes, out);
            let out_size = out_size.len();
            out.truncate(out_size);
            if error != rust_lzo::LZOError::OK {
                return Err(SquashfsError::CorruptedOrInvalidSquashfs);
            }
        },
        #[cfg(feature = "zstd")]
        Compressor::Zstd => {
            let mut decoder = zstd::bulk::Decompressor::new().unwrap();
            decoder.decompress_to_buffer(bytes, out)?;
        },
        _ => return Err(SquashfsError::UnsupportedCompression(compressor)),
    }
    Ok(())
}

#[instrument(skip_all)]
pub(crate) fn compress(
    bytes: &[u8],
    fc: FilesystemCompressor,
    block_size: u32,
) -> Result<Vec<u8>, SquashfsError> {
    match (fc.id, fc.options, fc.extra) {
        #[cfg(feature = "xz")]
        (Compressor::Xz, option @ (Some(CompressionOptions::Xz(_)) | None), extra) => {
            let dict_size = match option {
                None => block_size,
                Some(CompressionOptions::Xz(option)) => option.dictionary_size,
                Some(_) => unreachable!(),
            };
            let default_level = 6; // LZMA_DEFAULT
            let level = match extra {
                None => default_level,
                Some(CompressionExtra::Xz(xz)) => {
                    if let Some(level) = xz.level {
                        level
                    } else {
                        default_level
                    }
                },
            };
            let check = Check::Crc32;
            let mut opts = LzmaOptions::new_preset(level).unwrap();
            opts.dict_size(dict_size);

            let mut filters = Filters::new();
            if let Some(CompressionOptions::Xz(xz)) = option {
                match xz.filters {
                    XzFilter::X86 => filters.x86(),
                    XzFilter::PowerPC => filters.powerpc(),
                    XzFilter::IA64 => filters.ia64(),
                    XzFilter::Arm => filters.arm(),
                    XzFilter::ArmThumb => filters.arm_thumb(),
                    XzFilter::Sparc => filters.sparc(),
                };
            }
            filters.lzma2(&opts);

            let stream = MtStreamBuilder::new()
                .threads(2)
                .filters(filters)
                .check(check)
                .encoder()
                .unwrap();

            let mut encoder = XzEncoder::new_stream(Cursor::new(bytes), stream);
            let mut buf = vec![];
            encoder.read_to_end(&mut buf)?;
            Ok(buf)
        },
        #[cfg(feature = "gzip")]
        (Compressor::Gzip, Some(CompressionOptions::Gzip(gzip)), _) => {
            // TODO(#8): Use window_size and strategies
            let mut encoder =
                ZlibEncoder::new(Cursor::new(bytes), Compression::new(gzip.compression_level));
            let mut buf = vec![];
            encoder.read_to_end(&mut buf)?;
            Ok(buf)
        },
        #[cfg(feature = "gzip")]
        (Compressor::Gzip, None, _) => {
            let mut encoder = ZlibEncoder::new(Cursor::new(bytes), Compression::new(9));
            let mut buf = vec![];
            encoder.read_to_end(&mut buf)?;
            Ok(buf)
        },
        #[cfg(feature = "lzo")]
        (Compressor::Lzo, _, _) => {
            let mut lzo = rust_lzo::LZOContext::new();
            let mut buf = vec![0; rust_lzo::worst_compress(bytes.len())];
            let error = lzo.compress(bytes, &mut buf);
            if error != rust_lzo::LZOError::OK {
                return Err(SquashfsError::CorruptedOrInvalidSquashfs);
            }
            Ok(buf)
        },
        #[cfg(feature = "zstd")]
        (Compressor::Zstd, option @ (Some(CompressionOptions::Zstd(_)) | None), _) => {
            let compression_level = match option {
                None => 3,
                Some(CompressionOptions::Zstd(option)) => option.compression_level,
                Some(_) => unreachable!(),
            };
            let mut encoder = zstd::bulk::Compressor::new(compression_level as i32)?;
            let mut buf = vec![];
            encoder.compress_to_buffer(bytes, &mut buf)?;
            Ok(buf)
        },
        _ => Err(SquashfsError::UnsupportedCompression(fc.id)),
    }
}
