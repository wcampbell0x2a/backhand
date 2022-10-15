use deku::prelude::*;

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(type = "u16")]
#[deku(endian = "little")]
pub enum Inode {
    #[deku(id = "1")]
    BasicDirectory(BasicDirectory),

    #[deku(id = "2")]
    BasicFile(BasicFile),
}

impl Inode {
    pub fn expect_dir(&self) -> &BasicDirectory {
        if let Self::BasicDirectory(basic_dir) = self {
            basic_dir
        } else {
            panic!("not a dir");
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct InodeHeader {
    pub(crate) permissions: u16,
    pub(crate) uid: u16,
    pub(crate) gid: u16,
    pub(crate) mtime: u32,
    pub(crate) inode_number: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicDirectory {
    pub(crate) header: InodeHeader,
    pub(crate) block_index: u32,
    pub(crate) link_count: u32,
    pub(crate) file_size: u16,
    pub(crate) block_offset: u16,
    pub(crate) parent_inode: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicFile {
    pub(crate) header: InodeHeader,
    pub(crate) blocks_start: u32,
    pub(crate) frag_index: u32,
    pub(crate) block_offset: u32,
    pub(crate) file_size: u32,
    #[deku(count = "Self::count(*frag_index, *file_size)")]
    pub(crate) block_sizes: Vec<u32>,
}

impl BasicFile {
    fn count(fragment: u32, file_size: u32) -> u32 {
        const NO_FRAGMENT: u32 = 0xffffffff;

        // !!! TODO: this _needs_ to be from the superblock !!!
        const BLOCK_SIZE: u64 = 0x20000_u64;
        // !!! TODO: this _needs_ to be from the superblock !!!
        const BLOCK_LOG: u64 = 0x11;

        if fragment == NO_FRAGMENT {
            ((file_size as u64 + BLOCK_SIZE - 1) >> BLOCK_LOG) as u32
        } else {
            file_size >> BLOCK_LOG
        }
    }
}
