//! Data Fragment support

use deku::prelude::*;

use crate::v4::data::DataSize;

pub(crate) const SIZE: usize = Fragment::SIZE_BYTES.unwrap();

/// Fragment table entry pointing to a fragment block
#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite, DekuSize)]
#[deku(endian = "type_endian", ctx = "type_endian: deku::ctx::Endian")]
pub struct Fragment {
    /// Offset to the fragment block in the image
    pub start: u64,
    /// Size of the fragment block
    pub size: DataSize,
    /// Unused field (must be zero)
    pub unused: u32,
}

impl Fragment {
    /// Create a new fragment entry
    pub fn new(start: u64, size: DataSize, unused: u32) -> Self {
        Self { start, size, unused }
    }
}
