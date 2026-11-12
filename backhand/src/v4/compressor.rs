//! Types of supported compression algorithms

use no_std_io2::io::{Read, Write};
use std::io::Cursor;

use deku::prelude::*;
#[cfg(feature = "gzip")]
use flate2::read::ZlibEncoder;
#[cfg(feature = "gzip")]
use flate2::{Compress, Compression};
#[cfg(feature = "gzip")]
use flate2::{Decompress, FlushDecompress, Status};

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
    // A range from 9 ..= 15
    pub window_size: u8,
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

    /// Give the BCJ filters that are set, in the order that the encoder uses them.
    #[cfg(feature = "any-xz")]
    fn bcj_filters(self) -> impl Iterator<Item = BcjFilter> {
        const FILTER_BITS: [(u32, BcjFilter); 6] = [
            (0x0001, BcjFilter::X86),
            (0x0002, BcjFilter::PowerPc),
            (0x0004, BcjFilter::Ia64),
            (0x0008, BcjFilter::Arm),
            (0x0010, BcjFilter::ArmThumb),
            (0x0020, BcjFilter::Sparc),
        ];
        FILTER_BITS
            .into_iter()
            .filter(move |(bit, _)| self.0 & bit == *bit)
            .map(|(_, filter)| filter)
    }
}

/// A branch/call/jump filter that the xz encoder applies before LZMA2.
#[cfg(feature = "any-xz")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BcjFilter {
    X86,
    PowerPc,
    Ia64,
    Arm,
    ArmThumb,
    Sparc,
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

/// The maximum size of one decompressed block.
#[cfg(any(feature = "any-xz", feature = "gzip"))]
const MAX_DECOMPRESSED_SIZE: usize = 4 * super::squashfs::MAX_BLOCK_SIZE as usize;

/// xz with the liblzma C library.
#[cfg(feature = "xz")]
mod xz {
    use liblzma_sys::{lzma_filter, lzma_options_lzma, lzma_vli};

    use super::{BcjFilter, MAX_DECOMPRESSED_SIZE, XzFilter};
    use crate::error::BackhandError;

    /// Decompress a complete xz stream in one call. The data is added to `out`.
    pub(super) fn decode_buffer(bytes: &[u8], out: &mut Vec<u8>) -> Result<(), BackhandError> {
        if out.capacity() == out.len() {
            out.reserve(bytes.len().max(8 * 1024));
        }

        loop {
            let start = out.len();
            let spare = out.capacity() - start;
            // A stream that does not fit in this size gives LZMA_BUF_ERROR.
            let out_size = spare.min(MAX_DECOMPRESSED_SIZE);
            let mut memlimit = u64::MAX;
            let mut in_pos = 0usize;
            let mut out_pos = 0usize;

            // SAFETY: `input`/`in_size` and `out`/`out_size` point to valid memory
            // areas that do not overlap. liblzma writes a maximum of `out_size`
            // bytes into the spare capacity after `start`. It gives the number of
            // bytes in `out_pos`. The code increases the length only across the
            // bytes that liblzma wrote.
            let ret = unsafe {
                liblzma_sys::lzma_stream_buffer_decode(
                    &mut memlimit,
                    0,
                    std::ptr::null(),
                    bytes.as_ptr(),
                    &mut in_pos,
                    bytes.len(),
                    out.as_mut_ptr().add(start),
                    &mut out_pos,
                    out_size,
                )
            };

            match ret {
                liblzma_sys::LZMA_OK => {
                    // SAFETY: liblzma wrote `out_pos` bytes after `start`.
                    unsafe { out.set_len(start + out_pos) };
                    return Ok(());
                }
                // The output did not fit. Increase the buffer and decode again from
                // the start.
                liblzma_sys::LZMA_BUF_ERROR => {
                    if out_size >= MAX_DECOMPRESSED_SIZE {
                        return Err(BackhandError::CorruptedOrInvalidSquashfs);
                    }
                    out.reserve(spare.max(8 * 1024) * 2);
                }
                _ => return Err(BackhandError::CorruptedOrInvalidSquashfs),
            }
        }
    }

    pub(super) fn encode(
        bytes: &[u8],
        level: u32,
        dict_size: u32,
        filter: Option<XzFilter>,
    ) -> Result<Vec<u8>, BackhandError> {
        // SAFETY: `lzma_options_lzma` has only integers, enums and pointers. Zero is a
        // valid value for all of them, and `lzma_lzma_preset` sets all the fields.
        let mut opts: lzma_options_lzma = unsafe { core::mem::zeroed() };
        // SAFETY: `opts` is a valid `lzma_options_lzma`.
        if unsafe { liblzma_sys::lzma_lzma_preset(&mut opts, level) } != 0 {
            return Err(BackhandError::CompressionInit(err_text!(
                "xz options: preset {} is not supported",
                level
            )));
        }
        opts.dict_size = dict_size;

        let mut filters: Vec<lzma_filter> = filter
            .into_iter()
            .flat_map(XzFilter::bcj_filters)
            .map(|bcj| lzma_filter { id: bcj_filter_id(bcj), options: core::ptr::null_mut() })
            .collect();
        filters.push(lzma_filter {
            id: liblzma_sys::LZMA_FILTER_LZMA2,
            options: (&raw mut opts).cast(),
        });
        filters.push(lzma_filter {
            id: liblzma_sys::LZMA_VLI_UNKNOWN,
            options: core::ptr::null_mut(),
        });

        // The one-call encoder does not start threads, thus it uses the same
        // instructions on each run. It writes the same bytes as the multi-threaded
        // encoder, because both put the block sizes in the block header.
        // SAFETY: `lzma_stream_buffer_bound` has no preconditions.
        let out_size = unsafe { liblzma_sys::lzma_stream_buffer_bound(bytes.len()) };
        let mut buf = Vec::with_capacity(out_size);
        let mut out_pos = 0usize;
        // SAFETY: `filters` ends with `LZMA_VLI_UNKNOWN`, and `opts` lives until the
        // call returns. `input`/`in_size` and `out`/`out_size` point to valid memory
        // areas that do not overlap. liblzma writes a maximum of `out_size` bytes and
        // gives the number of bytes in `out_pos`.
        let ret = unsafe {
            liblzma_sys::lzma_stream_buffer_encode(
                filters.as_mut_ptr(),
                liblzma_sys::LZMA_CHECK_CRC32,
                core::ptr::null(),
                bytes.as_ptr(),
                bytes.len(),
                buf.as_mut_ptr(),
                &mut out_pos,
                out_size,
            )
        };
        if ret != liblzma_sys::LZMA_OK {
            return Err(BackhandError::CompressionInit(err_text!("xz encoder: error {}", ret)));
        }
        // SAFETY: liblzma wrote `out_pos` bytes from the start of `buf`.
        unsafe { buf.set_len(out_pos) };
        Ok(buf)
    }

    fn bcj_filter_id(bcj: BcjFilter) -> lzma_vli {
        match bcj {
            BcjFilter::X86 => liblzma_sys::LZMA_FILTER_X86,
            BcjFilter::PowerPc => liblzma_sys::LZMA_FILTER_POWERPC,
            BcjFilter::Ia64 => liblzma_sys::LZMA_FILTER_IA64,
            BcjFilter::Arm => liblzma_sys::LZMA_FILTER_ARM,
            BcjFilter::ArmThumb => liblzma_sys::LZMA_FILTER_ARMTHUMB,
            BcjFilter::Sparc => liblzma_sys::LZMA_FILTER_SPARC,
        }
    }
}

/// xz with the pure-Rust lzma-rust2 library.
#[cfg(feature = "xz-rust")]
mod xz {
    use lzma_rust2::{CheckType, FilterConfig, FilterType, XzOptions, XzReader, XzWriter};

    use super::{BcjFilter, MAX_DECOMPRESSED_SIZE, Read, Write, XzFilter};
    use crate::error::BackhandError;

    /// Decompress a complete xz stream in one call. The data is added to `out`.
    pub(super) fn decode_buffer(bytes: &[u8], out: &mut Vec<u8>) -> Result<(), BackhandError> {
        let start = out.len();
        // Read one byte more than the limit to find streams that are too large.
        let limit = MAX_DECOMPRESSED_SIZE as u64 + 1;
        let decoded = XzReader::new(bytes, false)
            .take(limit)
            .read_to_end(out)
            .map_err(|_| BackhandError::CorruptedOrInvalidSquashfs)?;
        if decoded > MAX_DECOMPRESSED_SIZE {
            out.truncate(start);
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        Ok(())
    }

    pub(super) fn encode(
        bytes: &[u8],
        level: u32,
        dict_size: u32,
        filter: Option<XzFilter>,
    ) -> Result<Vec<u8>, BackhandError> {
        let mut options = XzOptions::with_preset(level);
        options.lzma_options.dict_size = dict_size;
        options.set_check_sum_type(CheckType::Crc32);
        options.filters.extend(filter.into_iter().flat_map(XzFilter::bcj_filters).map(|bcj| {
            let filter_type = match bcj {
                BcjFilter::X86 => FilterType::BcjX86,
                BcjFilter::PowerPc => FilterType::BcjPpc,
                BcjFilter::Ia64 => FilterType::BcjIa64,
                BcjFilter::Arm => FilterType::BcjArm,
                BcjFilter::ArmThumb => FilterType::BcjArmThumb,
                BcjFilter::Sparc => FilterType::BcjSparc,
            };
            FilterConfig { filter_type, property: 0 }
        }));

        let mut encoder = XzWriter::new(Vec::with_capacity(bytes.len()), options)
            .map_err(|e| BackhandError::CompressionInit(err_text!("xz encoder: {}", e)))?;
        encoder.write_all(bytes)?;
        Ok(encoder.finish()?)
    }
}

/// Decompress a complete zlib stream in one call. The data is added to `out`.
#[cfg(feature = "gzip")]
fn gzip_decode_buffer(bytes: &[u8], out: &mut Vec<u8>) -> Result<(), BackhandError> {
    let start = out.len();
    let mut spare = out.capacity() - start;
    loop {
        if spare < 8 * 1024 {
            out.reserve(8 * 1024);
            spare = out.capacity() - start;
        }

        out.truncate(start);
        match Decompress::new(true).decompress_vec(bytes, out, FlushDecompress::Finish) {
            Ok(Status::StreamEnd) => return Ok(()),
            // The output did not fit. Increase the buffer and decode again.
            Ok(_) => {
                if spare >= MAX_DECOMPRESSED_SIZE {
                    return Err(BackhandError::CorruptedOrInvalidSquashfs);
                }
                out.reserve(spare * 2);
                spare = out.capacity() - start;
            }
            Err(_) => return Err(BackhandError::CorruptedOrInvalidSquashfs),
        }
    }
}

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
            Compressor::Gzip => gzip_decode_buffer(bytes, out)?,
            #[cfg(feature = "any-xz")]
            Compressor::Xz => xz::decode_buffer(bytes, out)?,
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
                let mut decoder = zstd::bulk::Decompressor::new().map_err(|e| {
                    BackhandError::CompressionInit(err_text!("zstd decompressor: {}", e))
                })?;
                decoder.decompress_to_buffer(bytes, out)?;
            }
            #[cfg(feature = "lz4")]
            Compressor::Lz4 => {
                out.resize(out.capacity(), 0u8);
                let out_size =
                    lz4_flex::decompress_into(bytes, out.as_mut_slice()).map_err(|e| {
                        BackhandError::CompressionInit(err_text!("lz4 decompression: {}", e))
                    })?;
                out.truncate(out_size);
            }
            _ => return Err(BackhandError::UnsupportedCompression(err_text!("{:?}", compressor))),
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
            #[cfg(feature = "any-xz")]
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
                let filter = match option {
                    Some(CompressionOptions::Xz(xz)) => Some(xz.filters),
                    _ => None,
                };
                xz::encode(bytes, level, dict_size, filter)
            }
            #[cfg(feature = "gzip")]
            (Compressor::Gzip, option @ (Some(CompressionOptions::Gzip(_)) | None), _) => {
                let (compression_level, window_size) = match option {
                    None => (Compression::best(), 15), // 9 (best compression level)
                    Some(CompressionOptions::Gzip(option)) => {
                        (Compression::new(option.compression_level), option.window_size)
                    }
                    Some(_) => unreachable!(),
                };
                let options = Compress::new_with_window_bits(compression_level, true, window_size);

                // TODO(#8): Use strategies

                let mut encoder = ZlibEncoder::new_with_compress(Cursor::new(bytes), options);
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
            _ => Err(BackhandError::UnsupportedCompression(err_text!("{:?}", fc.id))),
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

#[cfg(all(test, feature = "any-xz"))]
mod xz_tests {
    use super::*;
    use crate::v4::filesystem::writer::ExtraXz;

    const BLOCK_SIZE: u32 = 0x20000;

    /// Data that compresses well, but is not only one repeated byte.
    fn sample_data(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8 ^ (i / 4096) as u8).collect()
    }

    fn xz_compressor(filters: Option<u32>) -> FilesystemCompressor {
        let options = filters.map(|filters| {
            CompressionOptions::Xz(Xz {
                dictionary_size: BLOCK_SIZE,
                filters: XzFilter::new(filters),
                bit_opts: None,
                fb: None,
            })
        });
        FilesystemCompressor::new(Compressor::Xz, options).unwrap()
    }

    fn round_trip(data: &[u8], fc: FilesystemCompressor) -> Vec<u8> {
        let compressed = DefaultCompressor.compress(data, fc, BLOCK_SIZE).unwrap();
        let mut out = vec![];
        DefaultCompressor.decompress(&compressed, &mut out, Compressor::Xz).unwrap();
        out
    }

    #[test]
    fn round_trip_without_options() {
        let data = sample_data(BLOCK_SIZE as usize);
        assert_eq!(round_trip(&data, xz_compressor(None)), data);
    }

    #[test]
    fn round_trip_empty() {
        assert!(round_trip(&[], xz_compressor(None)).is_empty());
    }

    #[test]
    fn round_trip_each_bcj_filter() {
        let data = sample_data(BLOCK_SIZE as usize);
        for bit in 0..6 {
            let fc = xz_compressor(Some(1 << bit));
            assert_eq!(round_trip(&data, fc), data, "filter bit {bit}");
        }
    }

    #[test]
    fn round_trip_each_level() {
        let data = sample_data(BLOCK_SIZE as usize);
        for level in 0..=9 {
            let mut extra = ExtraXz::default();
            extra.level(level).unwrap();
            let mut fc = xz_compressor(None);
            fc.extra(CompressionExtra::Xz(extra)).unwrap();
            assert_eq!(round_trip(&data, fc), data, "level {level}");
        }
    }

    #[test]
    fn more_than_three_bcj_filters_is_an_error() {
        let fc = xz_compressor(Some(0b1111));
        let result = DefaultCompressor.compress(&sample_data(64), fc, BLOCK_SIZE);
        assert!(matches!(result, Err(BackhandError::CompressionInit(_))));
    }

    #[test]
    #[cfg(feature = "xz")]
    fn unsupported_preset_is_an_error() {
        let result = xz::encode(&sample_data(64), 10, BLOCK_SIZE, None);
        assert!(matches!(result, Err(BackhandError::CompressionInit(_))));
    }

    #[test]
    fn decompress_appends_to_out() {
        let data = sample_data(1000);
        let compressed =
            DefaultCompressor.compress(&data, xz_compressor(None), BLOCK_SIZE).unwrap();
        let mut out = b"prefix".to_vec();
        DefaultCompressor.decompress(&compressed, &mut out, Compressor::Xz).unwrap();
        assert_eq!(&out[..6], b"prefix");
        assert_eq!(&out[6..], data);
    }

    #[test]
    fn decompress_corrupt_data_is_an_error() {
        let data = sample_data(BLOCK_SIZE as usize);
        let compressed =
            DefaultCompressor.compress(&data, xz_compressor(None), BLOCK_SIZE).unwrap();
        let mut out = vec![];

        let truncated = &compressed[..compressed.len() / 2];
        assert!(DefaultCompressor.decompress(truncated, &mut out, Compressor::Xz).is_err());

        let mut flipped = compressed.clone();
        let middle = flipped.len() / 2;
        flipped[middle] ^= 0xff;
        out.clear();
        assert!(DefaultCompressor.decompress(&flipped, &mut out, Compressor::Xz).is_err());

        out.clear();
        assert!(DefaultCompressor.decompress(b"not xz", &mut out, Compressor::Xz).is_err());
    }

    #[test]
    fn decompress_larger_than_limit_is_an_error() {
        let data = vec![0u8; MAX_DECOMPRESSED_SIZE + 1];
        let compressed =
            DefaultCompressor.compress(&data, xz_compressor(None), BLOCK_SIZE).unwrap();
        let mut out = vec![];
        let result = DefaultCompressor.decompress(&compressed, &mut out, Compressor::Xz);
        assert!(matches!(result, Err(BackhandError::CorruptedOrInvalidSquashfs)));
        assert!(out.is_empty());
    }

    #[test]
    fn decompress_at_limit_is_ok() {
        let data = vec![0u8; MAX_DECOMPRESSED_SIZE];
        let compressed =
            DefaultCompressor.compress(&data, xz_compressor(None), BLOCK_SIZE).unwrap();
        let mut out = vec![];
        DefaultCompressor.decompress(&compressed, &mut out, Compressor::Xz).unwrap();
        assert_eq!(out.len(), MAX_DECOMPRESSED_SIZE);
    }

    #[test]
    fn bcj_filters_follow_bit_order() {
        let filters: Vec<_> = XzFilter::new(0x0021).bcj_filters().collect();
        assert_eq!(filters, [BcjFilter::X86, BcjFilter::Sparc]);
        assert_eq!(XzFilter::new(0).bcj_filters().count(), 0);
        assert_eq!(XzFilter::new(0xffc0).bcj_filters().count(), 0);
    }

    proptest::proptest! {
        #[test]
        fn round_trip_any_data(data in proptest::collection::vec(proptest::num::u8::ANY, 0..4096), filters in 0u32..0x40) {
            let fc = xz_compressor(Some(filters));
            match DefaultCompressor.compress(&data, fc, BLOCK_SIZE) {
                Ok(compressed) => {
                    proptest::prop_assert!(filters.count_ones() <= 3);
                    let mut out = vec![];
                    DefaultCompressor.decompress(&compressed, &mut out, Compressor::Xz).unwrap();
                    proptest::prop_assert_eq!(out, data);
                }
                Err(_) => proptest::prop_assert!(filters.count_ones() > 3),
            }
        }

        #[test]
        fn decompress_any_bytes_does_not_panic(data in proptest::collection::vec(proptest::num::u8::ANY, 0..512)) {
            let mut out = vec![];
            let _ = DefaultCompressor.decompress(&data, &mut out, Compressor::Xz);
        }
    }
}
