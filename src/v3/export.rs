use deku::prelude::*;

/// NFS export support
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(
    ctx = "type_endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "type_endian",
    bit_order = "order"
)]
pub struct Export {
    pub num: u64,
}
