//! Types of supported compression algorithms

use no_std_io2::io::{Read, Write};
use std::io::Cursor;

use deku::prelude::*;
#[cfg(feature = "gzip")]
use flate2::Compression;
#[cfg(feature = "gzip")]
use flate2::read::ZlibEncoder;
#[cfg(feature = "xz")]
use liblzma::read::{XzDecoder, XzEncoder};
#[cfg(feature = "xz")]
use liblzma::stream::{Check, Filters, LzmaOptions, MtStreamBuilder};
use tracing::trace;

use crate::error::BackhandError;
use crate::traits::CompressionAction;
use crate::v4::filesystem::writer::{CompressionExtra, FilesystemCompressor};
use crate::v4::metadata::MetadataWriter;
use crate::v4::squashfs::Flags;

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite, DekuSize, Default)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[deku(id_type = "u16")]
#[repr(u16)]
#[rustfmt::skip]
pub enum Compressor {
    Uncompressed = 0,
    Gzip = 1,
    Lzma = 2,
    Lzo =  3,
    #[default]
    Xz =   4,
    Lz4 =  5,
    Zstd = 6,
}

#[derive(Debug, DekuRead, DekuWrite, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian, compressor: Compressor")]
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

#[derive(Debug, DekuRead, DekuWrite, DekuSize, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Gzip {
    pub compression_level: u32,
    pub window_size: u16,
    // TODO: enum
    pub strategies: u16,
}

#[derive(Debug, DekuRead, DekuWrite, DekuSize, PartialEq, Eq, Clone, Copy)]
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

    // the rest of these fields are from OpenWRT. These are optional, as the kernel will ignore
    // these fields when seen. We follow the same behaviour and don't attempt to parse if the bytes
    // for these aren't found
    // TODO: both are currently unused in this library
    // TODO: in openwrt, git-hash:f97ad870e11ebe5f3dcf833dda6c83b9165b37cb shows that before
    // official squashfs-tools had xz support they had the dictionary_size field as the last field
    // in this struct. If we get test images, I guess we can support this in the future.
    #[deku(cond = "!deku::reader.end()")]
    pub bit_opts: Option<u16>,
    #[deku(cond = "!deku::reader.end()")]
    pub fb: Option<u16>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite, DekuSize)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct XzFilter(u32);

impl XzFilter {
    pub fn new(filter: u32) -> Self {
        Self(filter)
    }

    fn x86(&self) -> bool {
        self.0 & 0x0001 == 0x0001
    }

    fn powerpc(&self) -> bool {
        self.0 & 0x0002 == 0x0002
    }

    fn ia64(&self) -> bool {
        self.0 & 0x0004 == 0x0004
    }

    fn arm(&self) -> bool {
        self.0 & 0x0008 == 0x0008
    }

    fn armthumb(&self) -> bool {
        self.0 & 0x0010 == 0x0010
    }

    fn sparc(&self) -> bool {
        self.0 & 0x0020 == 0x0020
    }
}

#[derive(Debug, DekuRead, DekuWrite, DekuSize, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Lz4 {
    pub version: u32,
    //TODO: enum
    pub flags: u32,
}

#[derive(Debug, DekuRead, DekuWrite, DekuSize, PartialEq, Eq, Clone, Copy)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct Zstd {
    pub compression_level: u32,
}

/// Default compressor that handles the compression features that are enabled
#[derive(Copy, Clone)]
pub struct DefaultCompressor;

impl CompressionAction for DefaultCompressor {
    type Compressor = Compressor;
    type FilesystemCompressor = FilesystemCompressor;
    type SuperBlock = super::squashfs::SuperBlock;
    type Error = crate::BackhandError;
    /// Using the current compressor from the superblock, decompress bytes
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        match compressor {
            Compressor::Uncompressed => out.extend_from_slice(bytes),
            #[cfg(feature = "gzip")]
            Compressor::Gzip => {
                let mut decoder = flate2::read::ZlibDecoder::new(bytes);
                decoder.read_to_end(out)?;
            }
            #[cfg(feature = "xz")]
            Compressor::Xz => {
                let mut decoder = XzDecoder::new(bytes);
                decoder.read_to_end(out)?;
            }
            #[cfg(feature = "lzo")]
            Compressor::Lzo => {
                out.resize(out.capacity(), 0);
                let (out_size, error) = rust_lzo::LZOContext::decompress_to_slice(bytes, out);
                let out_size = out_size.len();
                out.truncate(out_size);
                if error != rust_lzo::LZOError::OK {
                    return Err(BackhandError::CorruptedOrInvalidSquashfs);
                }
            }
            #[cfg(feature = "zstd")]
            Compressor::Zstd => {
                let mut decoder = zstd::bulk::Decompressor::new().unwrap();
                decoder.decompress_to_buffer(bytes, out)?;
            }
            #[cfg(feature = "lz4")]
            Compressor::Lz4 => {
                out.resize(out.capacity(), 0u8);
                let out_size = lz4_flex::decompress_into(bytes, out.as_mut_slice()).unwrap();
                out.truncate(out_size);
            }
            _ => return Err(BackhandError::UnsupportedCompression(format!("{:?}", compressor))),
        }
        Ok(())
    }

    /// Using the current compressor from the superblock, compress bytes
    fn compress(
        &self,
        bytes: &[u8],
        fc: Self::FilesystemCompressor,
        block_size: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        match (fc.id, fc.options, fc.extra) {
            (Compressor::Uncompressed, None, _) => Ok(bytes.to_vec()),
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
                    }
                };
                let check = Check::Crc32;
                let mut opts = LzmaOptions::new_preset(level).unwrap();
                opts.dict_size(dict_size);

                let mut filters = Filters::new();
                if let Some(CompressionOptions::Xz(xz)) = option {
                    if xz.filters.x86() {
                        filters.x86();
                    }
                    if xz.filters.powerpc() {
                        filters.powerpc();
                    }
                    if xz.filters.ia64() {
                        filters.ia64();
                    }
                    if xz.filters.arm() {
                        filters.arm();
                    }
                    if xz.filters.armthumb() {
                        filters.arm_thumb();
                    }
                    if xz.filters.sparc() {
                        filters.sparc();
                    }
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
            }
            #[cfg(feature = "gzip")]
            (Compressor::Gzip, option @ (Some(CompressionOptions::Gzip(_)) | None), _) => {
                let compression_level = match option {
                    None => Compression::best(), // 9
                    Some(CompressionOptions::Gzip(option)) => {
                        Compression::new(option.compression_level)
                    }
                    Some(_) => unreachable!(),
                };

                // TODO(#8): Use window_size and strategies (current window size defaults to 15)

                let mut encoder = ZlibEncoder::new(Cursor::new(bytes), compression_level);
                let mut buf = vec![];
                encoder.read_to_end(&mut buf)?;
                Ok(buf)
            }
            #[cfg(feature = "lzo")]
            (Compressor::Lzo, _, _) => {
                let mut lzo = rust_lzo::LZOContext::new();
                let mut buf = vec![0; rust_lzo::worst_compress(bytes.len())];
                let error = lzo.compress(bytes, &mut buf);
                if error != rust_lzo::LZOError::OK {
                    return Err(BackhandError::CorruptedOrInvalidSquashfs);
                }
                Ok(buf)
            }
            #[cfg(feature = "zstd")]
            (Compressor::Zstd, option @ (Some(CompressionOptions::Zstd(_)) | None), _) => {
                let compression_level = match option {
                    None => 3,
                    Some(CompressionOptions::Zstd(option)) => option.compression_level,
                    Some(_) => unreachable!(),
                };
                let mut encoder = zstd::bulk::Compressor::new(compression_level as i32)?;
                let buffer_len = zstd_safe::compress_bound(bytes.len());
                let mut buf = Vec::with_capacity(buffer_len);
                encoder.compress_to_buffer(bytes, &mut buf)?;
                Ok(buf)
            }
            #[cfg(feature = "lz4")]
            (Compressor::Lz4, _option, _) => Ok(lz4_flex::compress(bytes)),
            _ => Err(BackhandError::UnsupportedCompression(format!("{:?}", fc.id))),
        }
    }

    /// Using the current compressor options, create compression options
    fn compression_options(
        &self,
        superblock: &mut Self::SuperBlock,
        kind: &crate::kinds::Kind,
        fs_compressor: Self::FilesystemCompressor,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let mut w = Cursor::new(vec![]);

        // Write compression options, if any
        if let Some(options) = &fs_compressor.options {
            trace!("writing compression options");
            superblock.flags |= Flags::CompressorOptionsArePresent as u16;
            let mut compression_opt_buf_out = Cursor::new(vec![]);
            let mut writer = Writer::new(&mut compression_opt_buf_out);
            match options {
                CompressionOptions::Gzip(gzip) => {
                    gzip.to_writer(&mut writer, kind.inner.type_endian)?
                }
                CompressionOptions::Lz4(lz4) => {
                    lz4.to_writer(&mut writer, kind.inner.type_endian)?
                }
                CompressionOptions::Zstd(zstd) => {
                    zstd.to_writer(&mut writer, kind.inner.type_endian)?
                }
                CompressionOptions::Xz(xz) => xz.to_writer(&mut writer, kind.inner.type_endian)?,
                CompressionOptions::Lzo(lzo) => {
                    lzo.to_writer(&mut writer, kind.inner.type_endian)?
                }
                CompressionOptions::Lzma => {}
            }
            let mut metadata = MetadataWriter::new(
                self,
                fs_compressor,
                superblock.block_size,
                kind.inner.data_endian,
            );
            metadata.write_all(compression_opt_buf_out.get_ref())?;
            metadata.finalize(&mut w)?;
        }

        Ok(Some(w.into_inner()))
    }
}
