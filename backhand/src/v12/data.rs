//! Block size fields for SquashFS v1 and v2

use deku::prelude::*;

/// Set in a v2 block size to mean the block is stored uncompressed
const DATA_STORED_UNCOMPRESSED: u32 = 1 << 24;

/// Set in a v1 block size to mean the block is stored uncompressed
const DATA_STORED_UNCOMPRESSED_V1: u16 = 1 << 15;

/// Size of one data block, as v2 records it
///
/// The same 32-bit form v3 uses.
#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuSize)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    bit_order = "order"
)]
pub struct DataSize(u32);

impl DataSize {
    #[inline]
    pub fn new(size: u32, uncompressed: bool) -> Self {
        let mut value = size;
        if uncompressed {
            value |= DATA_STORED_UNCOMPRESSED;
        }
        Self(value)
    }

    #[inline]
    pub fn uncompressed(&self) -> bool {
        self.0 & DATA_STORED_UNCOMPRESSED != 0
    }

    #[inline]
    pub fn size(&self) -> u32 {
        self.0 & !DATA_STORED_UNCOMPRESSED
    }
}

/// Size of one data block, as v1 records it
///
/// v1 uses 16 bits, so the largest block it can describe is the flag bit
/// itself. A stored size of zero therefore means a full 32 KiB block, not an
/// empty one; see `SQUASHFS_COMPRESSED_SIZE` in the reference tools. Without
/// that rule an incompressible full block reads as a hole.
#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuSize)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    bit_order = "order"
)]
pub struct BlockSizeV1(u16);

impl BlockSizeV1 {
    #[inline]
    pub fn new(raw: u16) -> Self {
        Self(raw)
    }

    #[inline]
    pub fn uncompressed(&self) -> bool {
        self.0 & DATA_STORED_UNCOMPRESSED_V1 != 0
    }

    #[inline]
    pub fn size(&self) -> u32 {
        let size = self.0 & !DATA_STORED_UNCOMPRESSED_V1;
        if size == 0 { u32::from(DATA_STORED_UNCOMPRESSED_V1) } else { u32::from(size) }
    }
}

impl From<BlockSizeV1> for DataSize {
    fn from(v1: BlockSizeV1) -> Self {
        DataSize::new(v1.size(), v1.uncompressed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_size_and_flag_are_separate() {
        let compressed = DataSize::new(1234, false);
        assert_eq!(compressed.size(), 1234);
        assert!(!compressed.uncompressed());

        let stored = DataSize::new(1234, true);
        assert_eq!(stored.size(), 1234);
        assert!(stored.uncompressed());
    }

    #[test]
    fn v1_zero_size_means_a_full_block() {
        // The case that silently reads as a hole if the rule is missed.
        assert_eq!(BlockSizeV1::new(0).size(), 32768);
        assert!(!BlockSizeV1::new(0).uncompressed());

        // Uncompressed and full: only the flag bit is set.
        let full_stored = BlockSizeV1::new(1 << 15);
        assert_eq!(full_stored.size(), 32768);
        assert!(full_stored.uncompressed());
    }

    #[test]
    fn v1_ordinary_sizes_pass_through() {
        assert_eq!(BlockSizeV1::new(4096).size(), 4096);
        assert_eq!(BlockSizeV1::new((1 << 15) | 4096).size(), 4096);
        assert!(BlockSizeV1::new((1 << 15) | 4096).uncompressed());
    }

    #[test]
    fn v1_widens_into_the_v2_form() {
        let widened: DataSize = BlockSizeV1::new(0).into();
        assert_eq!(widened.size(), 32768);
        assert!(!widened.uncompressed());
    }
}
