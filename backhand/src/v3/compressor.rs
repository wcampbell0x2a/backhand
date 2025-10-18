use std::io::Read;

use flate2::read::ZlibDecoder;
use tracing::trace;

pub use crate::traits::types::Compressor;
pub use crate::traits::CompressionAction;

/// Empty filesystem compressor
#[derive(Debug, Copy, Clone, Default)]
pub struct FilesystemCompressor;

/// Default compressor for SquashFS v3 (gzip only)
#[derive(Copy, Clone)]
pub struct DefaultCompressor;

impl CompressionAction for DefaultCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Option<Compressor>;
    type FilesystemCompressor = FilesystemCompressor;
    type SuperBlock = super::squashfs::SuperBlock;

    /// Decompress bytes using gzip (the only compression algorithm supported in v3)
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        _compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        trace!("v3 decompress");
        let mut decoder = ZlibDecoder::new(bytes);
        decoder.read_to_end(out)?;
        Ok(())
    }

    /// Using the current compressor from the superblock, compress bytes
    fn compress(
        &self,
        _bytes: &[u8],
        _fc: Self::FilesystemCompressor,
        _block_size: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        unimplemented!();
    }
}
