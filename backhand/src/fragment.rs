//! Data Fragment support

use deku::prelude::*;

use crate::data::DataSize;

pub(crate) const SIZE: usize =
    core::mem::size_of::<u64>() + core::mem::size_of::<u32>() + core::mem::size_of::<u32>();

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(endian = "type_endian", ctx = "type_endian: deku::ctx::Endian")]
pub struct Fragment {
    pub start: u64,
    pub size: DataSize,
    pub unused: u32,
}

impl Fragment {
    pub fn new(start: u64, size: DataSize, unused: u32) -> Self {
        Self { start, size, unused }
    }
}
