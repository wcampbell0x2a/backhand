//! Types of supported compression algorithms

use std::io::{Cursor, Read};

use deku::prelude::*;
#[cfg(feature = "gzip")]
use flate2::read::ZlibEncoder;
#[cfg(feature = "gzip")]
use flate2::Compression;
use tracing::{error, instrument};
#[cfg(feature = "xz")]
use xz2::read::{XzDecoder, XzEncoder};
#[cfg(feature = "xz")]
use xz2::stream::{Check, Filters, LzmaOptions, MtStreamBuilder};

use crate::error::BackhandError;
/// All compression options for [`FilesystemWriter`]
#[derive(Debug, Copy, Clone, Default)]
pub struct FilesystemCompressor {
    pub(crate) id: Compressor,
    pub(crate) options: Option<CompressionOptions>,
    pub(crate) extra: Option<CompressionExtra>,
}

impl FilesystemCompressor {
    pub fn new(id: Compressor, options: Option<CompressionOptions>) -> Result<Self, BackhandError> {
        match (id, options) {
            // lz4 always requires options
            (Compressor::Lz4, None) => {
                error!("Lz4 compression options missing");
                return Err(BackhandError::InvalidCompressionOption);
            }
            //others having no options is always valid
            (_, None) => {}
            //only the corresponding option are valid
            (Compressor::Gzip, Some(CompressionOptions::Gzip(_)))
            | (Compressor::Lzma, Some(CompressionOptions::Lzma))
            | (Compressor::Lzo, Some(CompressionOptions::Lzo(_)))
            | (Compressor::Xz, Some(CompressionOptions::Xz(_)))
            | (Compressor::Lz4, Some(CompressionOptions::Lz4(_)))
            | (Compressor::Zstd, Some(CompressionOptions::Zstd(_))) => {}
            //other combinations are invalid
            _ => {
                error!("invalid compression settings");
                return Err(BackhandError::InvalidCompressionOption);
            }
        }
        Ok(Self {
            id,
            options,
            extra: None,
        })
    }

    /// Set options that are originally derived from the image if from a [`FilesystemReader`].
    /// These options will be written to the image when
    /// <https://github.com/wcampbell0x2a/backhand/issues/53> is fixed.
    pub fn options(&mut self, options: CompressionOptions) -> Result<(), BackhandError> {
        self.options = Some(options);
        Ok(())
    }

    /// Extra options that are *only* using during compression and are *not* stored in the
    /// resulting image
    pub fn extra(&mut self, extra: CompressionExtra) -> Result<(), BackhandError> {
        if matches!(extra, CompressionExtra::Xz(_)) && matches!(self.id, Compressor::Xz) {
            self.extra = Some(extra);
            return Ok(());
        }

        error!("invalid extra compression settings");
        Err(BackhandError::InvalidCompressionOption)
    }
}

/// Custom Compression support
///
/// For most instances, one should just use the [`DefaultCompressor`]. This will correctly
/// implement the Squashfs found within `squashfs-tools` and the Linux kernel.
///
/// However, the "wonderful world of vendor formats" has other ideas and has implemented their own
/// ideas of compression with custom tables and such! Thus, if the need arises you can implemented
/// your own [`CompressionAction`] to override the compression and de-compression used in this
/// library by default.
pub trait CompressionAction {
    /// Decompress function used for all decompression actions
    ///
    /// # Arguments
    ///
    /// * `bytes` - Input compressed bytes
    /// * `out` - Output uncompressed bytes
    /// * `compressor` - Compressor id from [SuperBlock]. This can be ignored if your custom
    /// compressor doesn't follow the normal values of the Compressor Id.
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
}

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
    // the rest of these fields are from OpenWRT. These are optional, as the kernel will ignore
    // these fields when seen. We follow the same behaviour and don't attempt to parse if the bytes
    // for these aren't found
    // TODO: both are currently unused in this library
    // TODO: in openwrt, git-hash:f97ad870e11ebe5f3dcf833dda6c83b9165b37cb shows that before
    // offical squashfs-tools had xz support they had the dictionary_size field as the last field
    // in this struct. If we get test images, I guess we can support this in the future.
    // TODO: fix
    // #[deku(cond = "!deku::rest.is_empty()")]
    // pub bit_opts: Option<u16>,
    // #[deku(cond = "!deku::rest.is_empty()")]
    // pub fb: Option<u16>,
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

/// Compression options only for [`FilesystemWriter`]
#[derive(Debug, Copy, Clone)]
pub enum CompressionExtra {
    Xz(ExtraXz),
}

/// Xz compression option for [`FilesystemWriter`]
#[derive(Debug, Copy, Clone, Default)]
pub struct ExtraXz {
    pub(crate) level: Option<u32>,
}

impl ExtraXz {
    /// Set compress preset level. Must be in range `0..=9`
    pub fn level(&mut self, level: u32) -> Result<(), BackhandError> {
        if level > 9 {
            return Err(BackhandError::InvalidCompressionOption);
        }
        self.level = Some(level);

        Ok(())
    }
}

/// Default compressor that handles the compression features that are enabled
#[derive(Copy, Clone)]
pub struct DefaultCompressor;

impl CompressionAction for DefaultCompressor {
    /// Using the current compressor from the superblock, decompress bytes
    #[instrument(skip_all)]
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Compressor,
    ) -> Result<(), BackhandError> {
        match compressor {
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
            _ => return Err(BackhandError::UnsupportedCompression),
        }
        Ok(())
    }

    #[instrument(skip_all)]
    fn compress(
        &self,
        bytes: &[u8],
        fc: FilesystemCompressor,
        block_size: u32,
    ) -> Result<Vec<u8>, BackhandError> {
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
                let mut buf = vec![];
                encoder.compress_to_buffer(bytes, &mut buf)?;
                Ok(buf)
            }
            _ => Err(BackhandError::UnsupportedCompression),
        }
    }
}
