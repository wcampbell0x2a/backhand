use tracing::trace;

pub use crate::traits::types::Compressor;
pub use crate::traits::CompressionAction;

#[derive(Copy, Clone)]
pub struct LzmaStandardCompressor;

impl CompressionAction for LzmaStandardCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Compressor;
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

        // Use standard LZMA decompression via liblzma
        let mut stream = liblzma::stream::Stream::new_lzma_decoder(u64::MAX)
            .map_err(|e| crate::BackhandError::UnsupportedCompression(e.to_string()))?;
        let _status = stream
            .process_vec(bytes, out, liblzma::stream::Action::Finish)
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
