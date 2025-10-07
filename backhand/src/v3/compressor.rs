use std::io::Read;

#[cfg(feature = "any-flate2")]
use flate2::read::ZlibDecoder;
use tracing::trace;

use crate::error::BackhandError;
pub use crate::traits::types::Compressor;
pub use crate::traits::CompressionAction;

/// FilesystemCompressor for v3 (only used internally, writing not supported)
#[derive(Debug, Copy, Clone, Default)]
pub struct FilesystemCompressor;

impl FilesystemCompressor {
    pub fn new(id: Compressor) -> Result<Self, BackhandError> {
        // v3 only supports gzip, no options
        match id {
            Compressor::Gzip | Compressor::None => {}
            _ => {
                return Err(BackhandError::UnsupportedCompression(format!(
                    "SquashFS v3 only supports gzip compression, got {:?}",
                    id
                )));
            }
        }
        Ok(Self)
    }
}

/// Default compressor for SquashFS v3 (gzip only)
#[derive(Copy, Clone)]
pub struct DefaultCompressor;

impl CompressionAction for DefaultCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Compressor;
    type FilesystemCompressor = FilesystemCompressor;
    type SuperBlock = super::squashfs::SuperBlock;

    /// Decompress bytes using gzip (the only compression algorithm supported in v3)
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        trace!("v3 decompress");
        match compressor {
            Compressor::None => out.extend_from_slice(bytes),
            #[cfg(feature = "any-flate2")]
            Compressor::Gzip => {
                let mut decoder = ZlibDecoder::new(bytes);
                decoder.read_to_end(out)?;
            }
            #[cfg(not(feature = "any-flate2"))]
            Compressor::Gzip => {
                return Err(BackhandError::UnsupportedCompression(
                    "gzip support not compiled in".to_string(),
                ));
            }
            _ => {
                return Err(BackhandError::UnsupportedCompression(format!(
                    "SquashFS v3 only supports gzip compression, got {:?}",
                    compressor
                )));
            }
        }
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
