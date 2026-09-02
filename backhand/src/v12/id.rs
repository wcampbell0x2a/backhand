//! User and group id tables for SquashFS v1 and v2

use deku::prelude::*;

/// One entry of the uid or guid table
///
/// The reference tools read these tables as 32-bit elements for every version.
#[derive(Debug, Copy, Clone, DekuRead, DekuSize, PartialEq, Eq)]
#[deku(endian = "type_endian", ctx = "type_endian: deku::ctx::Endian")]
pub struct Id {
    pub num: u32,
}

impl Id {
    pub const SIZE: usize = Self::SIZE_BYTES.unwrap();

    pub fn new(num: u32) -> Id {
        Id { num }
    }

    pub fn root() -> Vec<Id> {
        vec![Id { num: 0 }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_is_four_bytes() {
        assert_eq!(Id::SIZE, 4);
    }
}
