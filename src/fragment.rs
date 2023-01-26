//! Data Fragment support

use deku::prelude::*;

pub(crate) const FRAGMENT_SIZE: usize =
    std::mem::size_of::<u64>() + std::mem::size_of::<u32>() + std::mem::size_of::<u32>();

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Fragment {
    pub(crate) start: u64,
    pub(crate) size: u32,
    pub(crate) unused: u32,
}
