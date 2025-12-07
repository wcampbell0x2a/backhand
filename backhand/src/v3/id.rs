use deku::prelude::*;

/// 32 bit user and group IDs
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
