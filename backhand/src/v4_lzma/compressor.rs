pub use crate::traits::CompressionAction;

#[derive(Copy, Clone)]
pub struct V4LzmaAdaptiveCompressor;

impl CompressionAction for V4LzmaAdaptiveCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = crate::v4::compressor::Compressor;
    type FilesystemCompressor = crate::v4::filesystem::writer::FilesystemCompressor;
    type SuperBlock = crate::v4::squashfs::SuperBlock;

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
