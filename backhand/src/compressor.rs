//! Types of supported compression algorithms

use std::io::{Cursor, Read, Write};

use deku::prelude::*;
#[cfg(feature = "any-flate2")]
use flate2::read::ZlibEncoder;
#[cfg(feature = "any-flate2")]
use flate2::Compression;
use tracing::trace;
#[cfg(feature = "xz")]
use xz2::read::{XzDecoder, XzEncoder};
#[cfg(feature = "xz")]
use xz2::stream::{Check, Filters, LzmaOptions, MtStreamBuilder};

use crate::error::BackhandError;
use crate::filesystem::writer::{CompressionExtra, FilesystemCompressor};
use crate::kind::Kind;
use crate::metadata::MetadataWriter;
use crate::squashfs::Flags;
use crate::SuperBlock;

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite, Default)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[deku(id_type = "u16")]
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct XzFilter(u32);

impl XzFilter {
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

/// Custom Compression support
///
/// For most instances, one should just use the [`DefaultCompressor`]. This will correctly
/// implement the Squashfs found within `squashfs-tools` and the Linux kernel.
///
/// However, the "wonderful world of vendor formats" has other ideas and has implemented their own
/// ideas of compression with custom tables and such! Thus, if the need arises you can implement
/// your own [`CompressionAction`] to override the compression and de-compression used in this
/// library by default.
pub trait CompressionAction {
    /// Decompress function used for all decompression actions
    ///
    /// # Arguments
    ///
    /// * `bytes` - Input compressed bytes
    /// * `out` - Output uncompressed bytes. You will need to call `out.resize(out.capacity(), 0)`
    ///           if your compressor relies on having a max sized buffer to write into.
    /// * `compressor` - Compressor id from [SuperBlock]. This can be ignored if your custom
    ///                  compressor doesn't follow the normal values of the Compressor Id.
    ///
    /// [SuperBlock]: [`crate::SuperBlock`]
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Compressor,
    ) -> Result<(), BackhandError>;

    /// Compression function used for all compression actions
    ///
    /// # Arguments
    /// * `bytes` - Input uncompressed bytes
    /// * `fc` - Information from both the derived image and options added during compression
    /// * `block_size` - Block size from [SuperBlock]
    ///
    /// [SuperBlock]: [`crate::SuperBlock`]
    fn compress(
        &self,
        bytes: &[u8],
        fc: FilesystemCompressor,
        block_size: u32,
    ) -> Result<Vec<u8>, BackhandError>;

    /// Compression Options for non-default compression specific options
    ///
    /// This function is called when calling [FilesystemWriter::write](crate::FilesystemWriter::write), and the returned bytes are the
    ///  section right after the SuperBlock.
    ///
    /// # Arguments
    /// * `superblock` - Mutatable squashfs superblock info that will be written to disk after
    ///                  this function is called. The fields `inode_count`, `block_size`,
    ///                  `block_log` and `mod_time` *will* be set to `FilesystemWriter` options and can be trusted
    ///                  in this function.
    /// * `kind` - Kind information
    /// * `fs_compressor` - Compression Options
    fn compression_options(
        &self,
        superblock: &mut SuperBlock,
        kind: &Kind,
        fs_compressor: FilesystemCompressor,
    ) -> Result<Vec<u8>, BackhandError>;
}

/// Default compressor that handles the compression features that are enabled
#[derive(Copy, Clone)]
pub struct DefaultCompressor;

impl CompressionAction for DefaultCompressor {
    /// Using the current compressor from the superblock, decompress bytes
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Compressor,
    ) -> Result<(), BackhandError> {
        match compressor {
            Compressor::None => out.extend_from_slice(bytes),
            #[cfg(feature = "any-flate2")]
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
            _ => return Err(BackhandError::UnsupportedCompression(compressor)),
        }
        Ok(())
    }

    /// Using the current compressor from the superblock, compress bytes
    fn compress(
        &self,
        bytes: &[u8],
        fc: FilesystemCompressor,
        block_size: u32,
    ) -> Result<Vec<u8>, BackhandError> {
        match (fc.id, fc.options, fc.extra) {
            (Compressor::None, None, _) => Ok(bytes.to_vec()),
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
            #[cfg(feature = "any-flate2")]
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
            _ => Err(BackhandError::UnsupportedCompression(fc.id)),
        }
    }

    /// Using the current compressor options, create compression options
    fn compression_options(
        &self,
        superblock: &mut SuperBlock,
        kind: &Kind,
        fs_compressor: FilesystemCompressor,
    ) -> Result<Vec<u8>, BackhandError> {
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
                fs_compressor,
                superblock.block_size,
                Kind { inner: kind.inner.clone() },
            );
            metadata.write_all(compression_opt_buf_out.get_ref())?;
            metadata.finalize(&mut w)?;
        }

        Ok(w.into_inner())
    }
}
