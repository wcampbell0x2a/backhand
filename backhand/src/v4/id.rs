use deku::prelude::*;

/// 32 bit user and group IDs
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, DekuSize, PartialEq, Eq)]
#[deku(endian = "type_endian", ctx = "type_endian: deku::ctx::Endian")]
pub struct Id {
    /// The 32-bit user or group ID value
    pub num: u32,
}

impl Id {
    /// Size of an ID entry in bytes
    pub const SIZE: usize = Self::SIZE_BYTES.unwrap();

    /// Create a new ID entry
    pub fn new(num: u32) -> Id {
        Id { num }
    }

    /// Create a default root ID table (uid=0, gid=0)
    pub fn root() -> Vec<Id> {
        vec![Id { num: 0 }]
    }
}
