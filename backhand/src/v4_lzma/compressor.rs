pub use crate::traits::CompressionAction;

#[derive(Copy, Clone)]
pub struct V4LzmaAdaptiveCompressor;

impl CompressionAction for V4LzmaAdaptiveCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = crate::v4::compressor::Compressor;
    type FilesystemCompressor = crate::v4::filesystem::writer::FilesystemCompressor;
    type SuperBlock = crate::v4::squashfs::SuperBlock;

    /// Decompress one block, finding the LZMA parameters by search
    ///
    /// The reader does not use this path. It calls through
    /// [`Kind`](crate::kind::Kind), which holds the parameter cache and the
    /// block size. This impl exists for callers that use the trait directly, so
    /// it searches again on every block and assumes the usual block size.
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        _compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        let cache = crate::lzma::LzmaCache::new();
        crate::lzma::decompress_adaptive(bytes, out, &cache, crate::lzma::DEFAULT_BLOCK_SIZE)
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
