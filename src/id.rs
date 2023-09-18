use deku::prelude::*;

/// 32 bit user and group IDs
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(
    ctx = "type_endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "type_endian",
    bit_order = "order"
)]
pub struct Id {
    pub num: u32,
}

impl Id {
    pub const SIZE: usize = (u32::BITS / 8) as usize;

    pub fn new(num: u32) -> Id {
        Id { num }
    }

    pub fn root() -> Vec<Id> {
        vec![Id { num: 0 }]
    }
}
