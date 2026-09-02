//! Compressors for SquashFS v1 and v2
//!
//! Neither version records a compressor id. v1 and stock v2 are always gzip;
//! only the AVM/Freetz minor version 76 differs, and it uses LZMA.

use no_std_io2::io::Read;

use flate2::read::ZlibDecoder;

pub use crate::traits::CompressionAction;
pub use crate::traits::types::Compressor;

/// Empty filesystem compressor. v1 and v2 are read-only here.
#[derive(Debug, Copy, Clone, Default)]
pub struct FilesystemCompressor;

/// gzip, the only compressor stock v1 and v2 use
#[derive(Copy, Clone)]
pub struct DefaultCompressor;

impl CompressionAction for DefaultCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Option<Compressor>;
    type FilesystemCompressor = FilesystemCompressor;
    type SuperBlock = super::squashfs::SuperBlock;

    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        _compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        trace!("v1/v2 decompress");
        let mut decoder = ZlibDecoder::new(bytes);
        decoder.read_to_end(out)?;
        Ok(())
    }

    fn compress(
        &self,
        _bytes: &[u8],
        _fc: Self::FilesystemCompressor,
        _block_size: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        unimplemented!();
    }
}

/// LZMA, for the AVM/Freetz v2 variant
///
/// The blocks are plain LZMA streams. The shared engine finds their parameters,
/// so this only forwards to it.
#[cfg(feature = "v2_lzma")]
#[derive(Copy, Clone)]
pub struct V2LzmaCompressor;

#[cfg(feature = "v2_lzma")]
impl CompressionAction for V2LzmaCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Option<Compressor>;
    type FilesystemCompressor = FilesystemCompressor;
    type SuperBlock = super::squashfs::SuperBlock;

    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        _compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        crate::lzma::decompress_adaptive(
            bytes,
            out,
            &crate::lzma::LzmaCache::new(),
            crate::lzma::DEFAULT_BLOCK_SIZE,
        )
    }

    fn compress(
        &self,
        _bytes: &[u8],
        _fc: Self::FilesystemCompressor,
        _block_size: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        unimplemented!();
    }
}
