use deku::prelude::*;

/// NFS export support
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, DekuSize, PartialEq, Eq)]
#[deku(endian = "type_endian", ctx = "type_endian: deku::ctx::Endian")]
pub struct Export {
    /// Inode number for NFS export
    pub num: u64,
}
