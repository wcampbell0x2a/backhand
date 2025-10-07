use std::sync::Mutex;

pub use crate::traits::types::Compressor;
pub use crate::traits::CompressionAction;
use tracing::trace;

// Cache for discovered LZMA parameters
#[derive(Debug, Clone, Copy)]
struct LzmaParams {
    lc: u32,
    lp: u32,
    pb: u32,
    dict_size: u32,
    offset: usize,
}

static LZMA_CACHE: Mutex<Option<LzmaParams>> = Mutex::new(None);

const LZMA_MAX_LC: u32 = 4;
const LZMA_MAX_LP: u32 = 4;
const LZMA_MAX_PB: u32 = 4;
const LZMA_MAX_OFFSET: usize = 10;

#[derive(Copy, Clone)]
pub struct LzmaAdaptiveCompressor;

impl CompressionAction for LzmaAdaptiveCompressor {
    type Error = crate::error::BackhandError;
    type Compressor = Compressor;
    type FilesystemCompressor = crate::v3::compressor::FilesystemCompressor;
    type SuperBlock = crate::v3::squashfs::SuperBlock;

    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Self::Compressor,
    ) -> Result<(), Self::Error> {
        trace!("v3_lzma decompress");
        match compressor {
            _ => {
                if bytes.is_empty() {
                    return Ok(());
                }

                // Check if we have cached parameters
                if let Ok(cache) = LZMA_CACHE.lock() {
                    if let Some(params) = *cache {
                        drop(cache); // Release lock before attempting decompression
                        if let Ok(result) = self.try_lzma_with_params(bytes, params) {
                            tracing::trace!("LZMA decompression successful with cached parameters");
                            out.extend_from_slice(&result);
                            return Ok(());
                        }
                        tracing::trace!("Cached parameters failed, falling back to brute force");
                    }
                }

                // Brute force parameter discovery
                if let Ok(result) = self.brute_force_lzma_params(bytes) {
                    tracing::trace!("LZMA decompression successful after brute force discovery");
                    out.extend_from_slice(&result);
                    return Ok(());
                }

                Err(crate::BackhandError::UnsupportedCompression(
                    "Failed to decompress LZMA adaptive data".to_string(),
                ))
            }
        }
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

impl LzmaAdaptiveCompressor {
    /// Try LZMA decompression with specific parameters using raw liblzma-sys
    fn try_lzma_with_params(
        &self,
        bytes: &[u8],
        params: LzmaParams,
    ) -> Result<Vec<u8>, crate::BackhandError> {
        if params.offset >= bytes.len() {
            return Err(crate::BackhandError::UnsupportedCompression("Invalid offset".to_string()));
        }

        let data = &bytes[params.offset..];
        tracing::trace!(
            "Processing {} bytes from offset {}: {:02x?}",
            data.len(),
            params.offset,
            &data[..std::cmp::min(16, data.len())]
        );

        tracing::trace!("Attempting lzma-adaptive-sys decompression");

        // Estimate output size
        let estimated_output_size = std::cmp::max(data.len() * 10, 8192);

        // Create LZMA properties (lc, lp, pb, dict_size)
        let dict_size = if params.dict_size == 0xFFFFFFFF || params.dict_size == 0 {
            1u32 << 23 // 8MB default like sasquatch
        } else {
            params.dict_size
        };

        // Call lzma-adaptive-sys's safe wrapper
        tracing::trace!(
                "Calling lzma_adaptive_sys::decompress_lzma with lc={}, lp={}, pb={}, dict_size=0x{:X}, offset={}",
                params.lc,
                params.lp,
                params.pb,
                dict_size,
                params.offset
            );

        match lzma_adaptive_sys::decompress_lzma(
            bytes,
            params.lc,
            params.lp,
            params.pb,
            dict_size,
            params.offset,
            estimated_output_size,
        ) {
            Ok(result) => {
                tracing::trace!("lzma_adaptive_sys SUCCESS: decompressed {} bytes", result.len());
                return Ok(result);
            }
            Err(error_code) => {
                tracing::trace!("lzma_adaptive_sys failed with error code: {}", error_code);
            }
        }

        Err(crate::BackhandError::UnsupportedCompression("LZMA decompression failed".to_string()))
    }

    /// Brute force LZMA parameter discovery
    fn brute_force_lzma_params(&self, bytes: &[u8]) -> Result<Vec<u8>, crate::BackhandError> {
        tracing::trace!("Starting LZMA brute force parameter discovery");

        for offset in 0..=LZMA_MAX_OFFSET {
            if offset >= bytes.len() {
                continue;
            }

            for lc in 0..=LZMA_MAX_LC {
                for lp in 0..=LZMA_MAX_LP {
                    for pb in 0..=LZMA_MAX_PB {
                        // Try with various dictionary sizes, starting with the one sasquatch uses
                        for &dict_size in &[0xFFFFFFFF, 0x800000, 0x100000, 0x400000] {
                            let params = LzmaParams { lc, lp, pb, dict_size, offset };

                            tracing::trace!("Trying LZMA params: lc={}, lp={}, pb={}, dict_size=0x{:X}, offset={}",
                                          lc, lp, pb, dict_size, offset);

                            if let Ok(result) = self.try_lzma_with_params(bytes, params) {
                                tracing::trace!("SUCCESS: Found working LZMA params: lc={}, lp={}, pb={}, dict_size=0x{:X}, offset={}, decompressed {} bytes",
                                              lc, lp, pb, dict_size, offset, result.len());

                                // Cache the successful parameters
                                if let Ok(mut cache) = LZMA_CACHE.lock() {
                                    *cache = Some(params);
                                }

                                return Ok(result);
                            }
                        }
                    }
                }
            }
        }

        Err(crate::BackhandError::UnsupportedCompression(
            "Failed to find working LZMA parameters".to_string(),
        ))
    }
}
