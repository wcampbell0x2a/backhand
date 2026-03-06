//! Shared adaptive LZMA decompression

use std::sync::Mutex;

use no_std_io2::io::Read;
use tracing::trace;

use crate::error::BackhandError;

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

/// Decompress LZMA data using adaptive parameter discovery with caching,
/// falling back to standard LZMA if adaptive fails.
pub(crate) fn decompress_adaptive(bytes: &[u8], out: &mut Vec<u8>) -> Result<(), BackhandError> {
    if bytes.is_empty() {
        return Ok(());
    }

    // Check if we have cached parameters
    if let Ok(cache) = LZMA_CACHE.lock() {
        if let Some(params) = *cache {
            drop(cache); // Release lock before attempting decompression
            if let Ok(result) = try_lzma_with_params(bytes, params) {
                trace!("LZMA decompression successful with cached parameters");
                out.extend_from_slice(&result);
                return Ok(());
            }
            trace!("Cached parameters failed, falling back to brute force");
        }
    }

    // Brute force parameter discovery
    if let Ok(result) = brute_force_lzma_params(bytes) {
        trace!("LZMA decompression successful after brute force discovery");
        out.extend_from_slice(&result);
        return Ok(());
    }

    // Fall back to standard LZMA (some blocks like fragments in le_v3_0_lzma_swap use standard format)
    trace!("Adaptive LZMA failed, trying standard LZMA");
    if let Ok(mut reader) = lzma_rust2::LzmaReader::new_mem_limit(bytes, u32::MAX, None) {
        if reader.read_to_end(out).is_ok() {
            trace!("Standard LZMA decompression successful: {} bytes", out.len());
            return Ok(());
        }
        out.clear();
    }

    Err(BackhandError::UnsupportedCompression(
        "Failed to decompress LZMA adaptive data".to_string(),
    ))
}

/// Try LZMA decompression with specific parameters using lzma-adaptive-sys
fn try_lzma_with_params(bytes: &[u8], params: LzmaParams) -> Result<Vec<u8>, BackhandError> {
    if params.offset >= bytes.len() {
        return Err(BackhandError::UnsupportedCompression("Invalid offset".to_string()));
    }

    let data = &bytes[params.offset..];
    trace!(
        "Processing {} bytes from offset {}: {:02x?}",
        data.len(),
        params.offset,
        &data[..core::cmp::min(16, data.len())]
    );

    // Estimate output size - must be at least block_size (commonly 65536 or 131072)
    // since data blocks and fragments decompress up to block_size bytes
    let estimated_output_size = core::cmp::max(data.len() * 10, 1 << 17);

    let dict_size = if params.dict_size == 0xFFFFFFFF || params.dict_size == 0 {
        1u32 << 23 // 8MB default like sasquatch
    } else {
        params.dict_size
    };

    trace!(
        "Calling lzma_adaptive_sys::decompress_lzma with lc={}, lp={}, pb={}, dict_size=0x{:X}, offset={}",
        params.lc, params.lp, params.pb, dict_size, params.offset
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
            trace!("lzma_adaptive_sys SUCCESS: decompressed {} bytes", result.len());
            Ok(result)
        }
        Err(error_code) => {
            trace!("lzma_adaptive_sys failed with error code: {}", error_code);
            Err(BackhandError::UnsupportedCompression("LZMA decompression failed".to_string()))
        }
    }
}

/// Brute force LZMA parameter discovery
fn brute_force_lzma_params(bytes: &[u8]) -> Result<Vec<u8>, BackhandError> {
    trace!("Starting LZMA brute force parameter discovery");

    for offset in 0..=LZMA_MAX_OFFSET {
        if offset >= bytes.len() {
            continue;
        }

        for lc in 0..=LZMA_MAX_LC {
            for lp in 0..=LZMA_MAX_LP {
                for pb in 0..=LZMA_MAX_PB {
                    for &dict_size in &[0xFFFFFFFF, 0x800000, 0x100000, 0x400000] {
                        let params = LzmaParams { lc, lp, pb, dict_size, offset };

                        trace!(
                            "Trying LZMA params: lc={}, lp={}, pb={}, dict_size=0x{:X}, offset={}",
                            lc, lp, pb, dict_size, offset
                        );

                        if let Ok(result) = try_lzma_with_params(bytes, params) {
                            trace!(
                                "SUCCESS: Found working LZMA params: lc={}, lp={}, pb={}, dict_size=0x{:X}, offset={}, decompressed {} bytes",
                                lc,
                                lp,
                                pb,
                                dict_size,
                                offset,
                                result.len()
                            );

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

    Err(BackhandError::UnsupportedCompression("Failed to find working LZMA parameters".to_string()))
}
