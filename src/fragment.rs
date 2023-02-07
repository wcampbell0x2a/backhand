//! Data Fragment support

use deku::prelude::*;

use crate::data::DataSize;

pub(crate) const SIZE: usize =
    std::mem::size_of::<u64>() + std::mem::size_of::<u32>() + std::mem::size_of::<u32>();

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Fragment {
    pub start: u64,
    pub size: DataSize,
    pub unused: u32,
}
