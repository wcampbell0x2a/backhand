use no_std_io2::io::Read;

use tracing::trace;

pub use crate::traits::CompressionAction;
pub use crate::traits::types::Compressor;

#[derive(Copy, Clone)]
pub struct LzmaStandardCompressor;

impl CompressionAction for LzmaStandardCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Option<Compressor>;
    type FilesystemCompressor = crate::v3::compressor::FilesystemCompressor;
    type SuperBlock = crate::v3::squashfs::SuperBlock;

    /// Decompress bytes using standard LZMA for v3 filesystems.
    ///
    /// Tries lzma-rust2 first (handles standard .lzma streams with embedded
    /// headers). Falls back to constructing a .lzma alone header from the
    /// 5-byte SquashFS LZMA prefix (props + dict_size) for blocks that only
    /// contain the raw 5-byte header without the uncompressed size field.
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

        // Try lzma-rust2 first (works for standard .lzma streams)
        if let Ok(mut reader) = lzma_rust2::LzmaReader::new_mem_limit(bytes, u32::MAX, None) {
            if reader.read_to_end(out).is_ok() {
                return Ok(());
            }
            out.clear();
        }

        // Fall back: construct .lzma alone header from 5-byte SquashFS prefix.
        // SquashFS LZMA blocks use props(1) + dict_size(4) without the 8-byte
        // uncompressed size field. Insert -1 (unknown) to form a valid header.
        if bytes.len() < 5 {
            return Err(crate::BackhandError::UnsupportedCompression(
                "lzma data too short".to_string(),
            ));
        }

        trace!(
            "lzma-rust2 failed, trying liblzma with constructed header: props=0x{:02x}, dict_size=0x{:x}",
            bytes[0],
            u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]),
        );

        let mut lzma_alone = Vec::with_capacity(bytes.len() + 8);
        lzma_alone.extend_from_slice(&bytes[..5]);
        lzma_alone.extend_from_slice(&u64::MAX.to_le_bytes());
        lzma_alone.extend_from_slice(&bytes[5..]);

        let mut stream = liblzma::stream::Stream::new_lzma_decoder(u64::MAX)
            .map_err(|e| crate::BackhandError::UnsupportedCompression(e.to_string()))?;

        let mut output = vec![0u8; 1 << 20];
        stream
            .process(&lzma_alone, &mut output, liblzma::stream::Action::Run)
            .map_err(|e| crate::BackhandError::UnsupportedCompression(e.to_string()))?;

        let produced = stream.total_out() as usize;
        trace!("liblzma decompressed {} bytes", produced);
        output.truncate(produced);
        out.extend_from_slice(&output);
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
