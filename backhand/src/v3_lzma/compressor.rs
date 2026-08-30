pub use crate::traits::CompressionAction;
pub use crate::traits::types::Compressor;

#[derive(Copy, Clone)]
pub struct LzmaAdaptiveCompressor;

impl CompressionAction for LzmaAdaptiveCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Option<Compressor>;
    type FilesystemCompressor = crate::v3::compressor::FilesystemCompressor;
    type SuperBlock = crate::v3::squashfs::SuperBlock;

    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        _compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        crate::lzma::decompress_adaptive(bytes, out)
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
