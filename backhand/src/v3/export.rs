use deku::prelude::*;

/// NFS export support
#[derive(Debug, Copy, Clone, DekuRead, PartialEq, Eq)]
#[deku(endian = "type_endian", ctx = "type_endian: deku::ctx::Endian")]
pub struct Export {
    pub num: u64,
}
