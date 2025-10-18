use std::io::Read;

use tracing::trace;

pub use crate::traits::types::Compressor;
pub use crate::traits::CompressionAction;

#[derive(Copy, Clone)]
pub struct LzmaStandardCompressor;

impl CompressionAction for LzmaStandardCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Option<Compressor>;
    type FilesystemCompressor = crate::v3::compressor::FilesystemCompressor;
    type SuperBlock = crate::v3::squashfs::SuperBlock;

    /// Decompress bytes using standard LZMA for v3 filesystems
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        _compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        trace!("v3_lzma standard decompress");

        if bytes.is_empty() {
            return Ok(());
        }

        // Use lzma-rust2 for standard LZMA decompression
        let mut reader = lzma_rust2::LzmaReader::new_mem_limit(bytes, u32::MAX, None)
            .map_err(|e| crate::BackhandError::UnsupportedCompression(e.to_string()))?;
        reader
            .read_to_end(out)
            .map_err(|e| crate::BackhandError::UnsupportedCompression(e.to_string()))?;
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
