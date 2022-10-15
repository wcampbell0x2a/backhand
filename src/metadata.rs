use deku::prelude::*;

const METADATA_COMPRESSED: u16 = 1 << 15;

#[derive(Debug, DekuRead, DekuWrite)]
pub struct Metadata {
    // TODO; use deku to parse METADATA_COMPRESSED?
    pub(crate) len: u16,
    #[deku(count = "Self::len(*len)")]
    pub(crate) data: Vec<u8>,
}

impl Metadata {
    /// Check is_compressed bit within raw `len`
    pub fn is_compressed(len: u16) -> bool {
        len & METADATA_COMPRESSED == 0
    }

    /// Get actual length of `data` following `len` from unedited `len`
    pub fn len(len: u16) -> u16 {
        len & !(METADATA_COMPRESSED)
    }
}
