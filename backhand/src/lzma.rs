//! Shared adaptive LZMA decompression
//!
//! Some vendor firmware images use LZMA streams that do not record their own
//! parameters. To read them, this module finds the parameters by trial: it
//! decompresses one block with each candidate until one succeeds. The result is
//! kept in an [`LzmaCache`], which lives on the image's `Kind`, so the search
//! runs once per image and not once per block.

use std::sync::Mutex;

use no_std_io2::io::Read;

use crate::error::BackhandError;

/// LZMA parameters that the stream itself does not record
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LzmaParams {
    lc: u32,
    lp: u32,
    pb: u32,
    dict_size: u32,
    offset: usize,
}

/// What the blocks of one image need in order to decompress
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LzmaFormat {
    /// No block decompressed yet
    Unknown,
    /// The blocks are standard LZMA streams, which carry their own parameters
    Standard,
    /// The blocks need these parameters, found by search
    Adaptive(LzmaParams),
    /// No candidate decompressed the first block, so later blocks do not retry
    Undecodable,
}

/// What one image taught us about its LZMA blocks
///
/// A [`Kind`](crate::kind::Kind) holds one of these, so each opened image
/// searches at most once. Without it, an image whose parameters are not the
/// first candidate pays the full search on every block.
pub(crate) struct LzmaCache(Mutex<LzmaFormat>);

impl LzmaCache {
    pub(crate) const fn new() -> Self {
        Self(Mutex::new(LzmaFormat::Unknown))
    }

    /// A poisoned lock reads as `Unknown`, which costs a repeated search but
    /// stays correct.
    fn get(&self) -> LzmaFormat {
        self.0.lock().map(|format| *format).unwrap_or(LzmaFormat::Unknown)
    }

    fn set(&self, format: LzmaFormat) {
        if let Ok(mut cached) = self.0.lock() {
            *cached = format;
        }
    }
}

/// A clone describes a different image, so it starts with no knowledge.
///
/// `Kind::with_magic` and the other builders clone the inner kind. Carrying the
/// parameters over would apply one image's findings to another.
impl Clone for LzmaCache {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for LzmaCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("LzmaCache").field(&self.get()).finish()
    }
}

impl Default for LzmaCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Largest block size SquashFS allows, so the most a block can decompress to
pub(crate) const MAX_BLOCK_SIZE: usize = 1 << 20;

/// Block size to assume when the caller does not know the image's real one
///
/// Every search attempt allocates a buffer of this size, and the allocation is
/// zeroed, so a large value here is costly. 128 KiB is the usual SquashFS block
/// size. The reader passes the real block size and never uses this.
pub(crate) const DEFAULT_BLOCK_SIZE: usize = 1 << 17;

/// Largest `lc + lp` that the LZMA format allows
const LZMA_MAX_LC_PLUS_LP: u32 = 4;
const LZMA_MAX_LC: u32 = 4;
const LZMA_MAX_LP: u32 = 4;
const LZMA_MAX_PB: u32 = 4;
const LZMA_MAX_OFFSET: usize = 10;

/// The parameters that `lzma` uses unless told otherwise. Most images match
/// these, so try them before the search.
const LZMA_DEFAULT_PARAMS: LzmaParams =
    LzmaParams { lc: 3, lp: 0, pb: 2, dict_size: DICT_SIZE_DEFAULT, offset: 0 };

/// Dictionary size that sasquatch uses. The value only sizes the decoder
/// window, so one that is too large still decompresses correctly.
const DICT_SIZE_DEFAULT: u32 = 0x800000;

/// Other dictionary sizes seen in the wild. `0xFFFFFFFF` is absent on purpose:
/// [`try_lzma_with_params`] maps it to [`DICT_SIZE_DEFAULT`], so trying it again
/// would repeat work already done.
const DICT_SIZE_FALLBACKS: [u32; 2] = [0x100000, 0x400000];

/// Decompress one LZMA block, finding the parameters if they are not yet known
///
/// `max_out` is the largest size the block can decompress to: `block_size` for
/// file data, `METADATA_MAXSIZE` for metadata. Each attempt allocates a buffer
/// of this size, so an accurate value matters for both memory and speed.
pub(crate) fn decompress_adaptive(
    bytes: &[u8],
    out: &mut Vec<u8>,
    cache: &LzmaCache,
    max_out: usize,
) -> Result<(), BackhandError> {
    if bytes.is_empty() {
        return Ok(());
    }

    match cache.get() {
        LzmaFormat::Adaptive(params) => {
            if let Ok(result) = try_lzma_with_params(bytes, params, max_out) {
                out.extend_from_slice(&result);
                return Ok(());
            }
            trace!("cached LZMA parameters failed, searching again");
        }
        LzmaFormat::Standard => {
            if try_standard_lzma(bytes, out) {
                return Ok(());
            }
            trace!("cached standard LZMA failed, searching again");
        }
        // A block that no candidate decompressed means the image does not match
        // this kind. Searching again for every later block only wastes time.
        LzmaFormat::Undecodable => {
            return Err(BackhandError::UnsupportedCompression(err_text!(
                "no LZMA parameters decompress this image"
            )));
        }
        LzmaFormat::Unknown => {}
    }

    // Standard LZMA is one attempt, so try it before the search. Some images,
    // such as the fragments in le_v3_0_lzma_swap, use it for every block.
    if try_standard_lzma(bytes, out) {
        trace!("standard LZMA decompressed the block");
        cache.set(LzmaFormat::Standard);
        return Ok(());
    }

    if let Some((result, params)) = search_lzma_params(bytes, max_out) {
        trace!("found LZMA parameters {:?}", params);
        cache.set(LzmaFormat::Adaptive(params));
        out.extend_from_slice(&result);
        return Ok(());
    }

    cache.set(LzmaFormat::Undecodable);
    Err(BackhandError::UnsupportedCompression(err_text!(
        "no LZMA parameters decompress this image"
    )))
}

/// Decompress a standard LZMA stream, which records its own parameters
///
/// Returns `true` and fills `out` on success. On failure `out` is left empty,
/// because a failed read can still have written to it.
fn try_standard_lzma(bytes: &[u8], out: &mut Vec<u8>) -> bool {
    if let Ok(mut reader) = lzma_rust2::LzmaReader::new_mem_limit(bytes, u32::MAX, None) {
        if reader.read_to_end(out).is_ok() {
            return true;
        }
        out.clear();
    }
    false
}

/// Decompress with one set of parameters
fn try_lzma_with_params(
    bytes: &[u8],
    params: LzmaParams,
    max_out: usize,
) -> Result<Vec<u8>, BackhandError> {
    if params.offset >= bytes.len() {
        return Err(BackhandError::UnsupportedCompression(err_text!("invalid offset")));
    }

    let dict_size = if params.dict_size == 0xFFFFFFFF || params.dict_size == 0 {
        DICT_SIZE_DEFAULT
    } else {
        params.dict_size
    };

    lzma_adaptive_sys::decompress_lzma(
        bytes,
        params.lc,
        params.lp,
        params.pb,
        dict_size,
        params.offset,
        max_out,
    )
    .map_err(|code| {
        BackhandError::UnsupportedCompression(err_text!("LZMA decompression failed: {code}"))
    })
}

/// Find parameters that decompress this block
///
/// The search is ordered by how likely each candidate is, and skips candidates
/// that the LZMA format rules out. Every attempt allocates `max_out` bytes
/// inside `lzma-adaptive-sys`, so rejecting a candidate without calling into it
/// is what keeps the cost down.
fn search_lzma_params(bytes: &[u8], max_out: usize) -> Option<(Vec<u8>, LzmaParams)> {
    trace!("searching for LZMA parameters");

    // Most images use the default parameters, so spend one attempt on them
    // before walking the grid.
    if let Ok(result) = try_lzma_with_params(bytes, LZMA_DEFAULT_PARAMS, max_out) {
        return Some((result, LZMA_DEFAULT_PARAMS));
    }

    for offset in 0..=LZMA_MAX_OFFSET {
        // The range coder writes a zero as the first byte after the header, so
        // a nonzero byte rules out this offset without a decompression attempt.
        match bytes.get(offset) {
            Some(0) => {}
            _ => continue,
        }

        for lc in 0..=LZMA_MAX_LC {
            for lp in 0..=LZMA_MAX_LP {
                // The format requires lc + lp <= 4.
                if lc + lp > LZMA_MAX_LC_PLUS_LP {
                    continue;
                }

                for pb in 0..=LZMA_MAX_PB {
                    for dict_size in core::iter::once(DICT_SIZE_DEFAULT).chain(DICT_SIZE_FALLBACKS)
                    {
                        let params = LzmaParams { lc, lp, pb, dict_size, offset };
                        if params == LZMA_DEFAULT_PARAMS {
                            continue;
                        }

                        if let Ok(result) = try_lzma_with_params(bytes, params, max_out) {
                            return Some((result, params));
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metadata blocks are the smallest thing the reader decompresses, so use
    /// that bound in tests.
    const MAX_OUT: usize = 0x2000;

    #[test]
    fn empty_input_writes_nothing() {
        let cache = LzmaCache::new();
        let mut out = Vec::new();
        decompress_adaptive(&[], &mut out, &cache, MAX_OUT).unwrap();
        assert!(out.is_empty());
        // An empty block must not teach the cache anything.
        assert_eq!(cache.get(), LzmaFormat::Unknown);
    }

    #[test]
    fn garbage_input_reports_unsupported() {
        let cache = LzmaCache::new();
        let mut out = Vec::new();
        let error = decompress_adaptive(&[0xff; 64], &mut out, &cache, MAX_OUT).unwrap_err();
        assert!(matches!(error, BackhandError::UnsupportedCompression(_)));
        assert!(out.is_empty());
    }

    #[test]
    fn undecodable_block_is_not_searched_twice() {
        let cache = LzmaCache::new();
        let mut out = Vec::new();

        let _ = decompress_adaptive(&[0xff; 64], &mut out, &cache, MAX_OUT);
        assert_eq!(cache.get(), LzmaFormat::Undecodable);

        // The second call must fail from the cache, without another search.
        let error = decompress_adaptive(&[0xff; 64], &mut out, &cache, MAX_OUT).unwrap_err();
        assert!(matches!(error, BackhandError::UnsupportedCompression(_)));
    }

    #[test]
    fn caches_are_independent() {
        let first = LzmaCache::new();
        let second = LzmaCache::new();

        first.set(LzmaFormat::Standard);

        assert_eq!(first.get(), LzmaFormat::Standard);
        assert_eq!(second.get(), LzmaFormat::Unknown);
    }

    #[test]
    fn clone_starts_empty() {
        let cache = LzmaCache::new();
        cache.set(LzmaFormat::Adaptive(LZMA_DEFAULT_PARAMS));

        // A cloned Kind describes a different image and must search again.
        assert_eq!(cache.clone().get(), LzmaFormat::Unknown);
        assert_eq!(cache.get(), LzmaFormat::Adaptive(LZMA_DEFAULT_PARAMS));
    }

    #[test]
    fn search_skips_offsets_that_cannot_start_a_stream() {
        // No byte is zero, so every offset is rejected before any attempt.
        assert!(search_lzma_params(&[0xab; 32], MAX_OUT).is_none());
    }

    #[test]
    fn search_ends_when_every_offset_looks_valid() {
        // All zeroes passes the offset guard, so this walks the whole grid and
        // shows the search ends rather than running on.
        assert!(search_lzma_params(&[0x00; 128], MAX_OUT).is_none());
    }

    #[test]
    fn truncated_input_does_not_panic() {
        // A block shorter than the offsets the search tries must not index out
        // of bounds.
        for len in 0..LZMA_MAX_OFFSET + 2 {
            let cache = LzmaCache::new();
            let mut out = Vec::new();
            let _ = decompress_adaptive(&vec![0x00; len], &mut out, &cache, MAX_OUT);
        }
    }
}
