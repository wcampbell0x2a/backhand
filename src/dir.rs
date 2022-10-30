//! [`Dir`], [`DirEntry`], and [`DirectoryIndex`]

use deku::prelude::*;

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "little")]
pub struct Dir {
    pub(crate) count: u32,
    pub(crate) start: u32,
    pub(crate) inode_num: u32,
    #[deku(count = "*count + 1")]
    pub(crate) dir_entries: Vec<DirEntry>,
}

// TODO: derive our own Debug, with name()
#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct DirEntry {
    pub(crate) offset: u16,
    pub(crate) inode_offset: i16,
    pub(crate) t: u16,
    pub(crate) name_size: u16,

    // TODO: CString
    #[deku(count = "*name_size + 1")]
    pub(crate) name: Vec<u8>,
}

impl DirEntry {
    pub fn name(&self) -> String {
        std::str::from_utf8(&self.name).unwrap().to_string()
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct DirectoryIndex {
    pub(crate) index: u32,
    pub(crate) start: u32,
    pub(crate) name_size: u32,
    #[deku(count = "*name_size + 1")]
    pub(crate) name: Vec<u8>,
}
