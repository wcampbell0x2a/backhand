//! Data fragment support for SquashFS v1 and v2

use deku::prelude::*;

use super::data::DataSize;

pub(crate) const SIZE: usize = Fragment::SIZE_BYTES.unwrap();

/// One fragment table entry
///
/// Eight bytes, against v3's sixteen: the start is 32 bits, and there is no
/// trailing `pending` field.
#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuSize)]
#[deku(
    endian = "type_endian",
    ctx = "type_endian: deku::ctx::Endian, order: deku::ctx::Order",
    bit_order = "order"
)]
pub struct Fragment {
    pub start: u32,
    pub size: DataSize,
}

impl Fragment {
    pub fn new(start: u32, size: DataSize) -> Self {
        Self { start, size }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_is_eight_bytes() {
        assert_eq!(SIZE, 8);
    }
}
