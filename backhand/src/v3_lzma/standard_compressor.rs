use no_std_io2::io::Read;

use crate::lzma::MAX_BLOCK_SIZE;

pub use crate::traits::CompressionAction;
pub use crate::traits::types::Compressor;

#[derive(Copy, Clone)]
pub struct LzmaStandardCompressor;

impl CompressionAction for LzmaStandardCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Option<Compressor>;
    type FilesystemCompressor = crate::v3::compressor::FilesystemCompressor;
    type SuperBlock = crate::v3::squashfs::SuperBlock;

    /// Decompress bytes using standard LZMA for v3 filesystems
    ///
    /// A v3 image holds either a full .lzma stream or a bare 5-byte header.
    /// This tries the first form, then builds the missing header for the
    /// second.
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
            return Err(crate::BackhandError::UnsupportedCompression(err_text!(
                "lzma data too short"
            )));
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
            .map_err(|e| crate::BackhandError::UnsupportedCompression(err_text!("{e}")))?;

        // A SquashFS block never decompresses past the largest block size.
        let mut output = vec![0u8; MAX_BLOCK_SIZE];
        stream
            .process(&lzma_alone, &mut output, liblzma::stream::Action::Run)
            .map_err(|e| crate::BackhandError::UnsupportedCompression(err_text!("{e}")))?;

        // `process` returns Ok once the output buffer is full, so a short read
        // looks like success. Compare the input it consumed against the input it
        // was given, otherwise a block larger than the buffer is silently cut.
        if (stream.total_in() as usize) < lzma_alone.len() {
            return Err(crate::BackhandError::UnsupportedCompression(err_text!(
                "lzma block decompresses past the largest block size"
            )));
        }

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
