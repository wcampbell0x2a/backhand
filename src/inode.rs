//! [`Inode`], and other inode types

use deku::prelude::*;

use crate::dir::DirectoryIndex;

// TODO: change the Inode struct to first read the id, then header (in a struct), then the enum for
// the type specific information

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(ctx = "block_size: u32, block_log: u16")]
#[deku(type = "u16")]
#[deku(endian = "little")]
pub enum Inode {
    #[deku(id = "1")]
    BasicDirectory(BasicDirectory),

    #[deku(id = "2")]
    BasicFile(#[deku(ctx = "block_size, block_log")] BasicFile),

    #[deku(id = "3")]
    BasicSymlink(BasicSymlink),

    #[deku(id = "4")]
    BasicBlockDevice(BasicDeviceSpecialFile),

    #[deku(id = "5")]
    BasicCharacterDevice(BasicDeviceSpecialFile),

    #[deku(id = "8")]
    ExtendedDirectory(ExtendedDirectory),

    #[deku(id = "9")]
    ExtendedFile(#[deku(ctx = "block_size, block_log")] ExtendedFile),
}

impl Inode {
    /// Returns a reference to the expect dir of this [`Inode`].
    ///
    /// # Panics
    ///
    /// Panics if `self` is not `Inode::BasicDirectory`
    pub fn expect_dir(&self) -> BasicDirectory {
        match self {
            Self::BasicDirectory(basic_dir) => basic_dir.clone(),
            Self::ExtendedDirectory(ex_dir) => BasicDirectory::from(ex_dir),
            _ => panic!("not a dir"),
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, Copy, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct InodeHeader {
    pub(crate) permissions: u16,
    pub(crate) uid: u16,
    pub(crate) gid: u16,
    pub(crate) mtime: u32,
    pub(crate) inode_number: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicDirectory {
    pub(crate) header: InodeHeader,
    pub(crate) block_index: u32,
    pub(crate) link_count: u32,
    pub(crate) file_size: u16,
    pub(crate) block_offset: u16,
    pub(crate) parent_inode: u32,
}

impl From<&ExtendedDirectory> for BasicDirectory {
    fn from(ex_dir: &ExtendedDirectory) -> Self {
        Self {
            header: ex_dir.header,
            block_index: ex_dir.block_index,
            link_count: ex_dir.link_count,
            file_size: ex_dir.file_size as u16,
            block_offset: ex_dir.block_offset,
            parent_inode: ex_dir.parent_inode,
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct ExtendedDirectory {
    pub(crate) header: InodeHeader,
    pub(crate) link_count: u32,
    pub(crate) file_size: u32,
    pub(crate) block_index: u32,
    pub(crate) parent_inode: u32,
    pub(crate) index_count: u16,
    pub(crate) block_offset: u16,
    pub(crate) xattr_index: u32,
    // TODO: this has a type
    #[deku(count = "*index_count")]
    pub(crate) dir_index: Vec<DirectoryIndex>,
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, block_size: u32, block_log: u16"
)]
pub struct BasicFile {
    pub(crate) header: InodeHeader,
    pub(crate) blocks_start: u32,
    pub(crate) frag_index: u32,
    pub(crate) block_offset: u32,
    pub(crate) file_size: u32,
    #[deku(count = "block_count(block_size, block_log, *frag_index, *file_size as u64)")]
    pub(crate) block_sizes: Vec<u32>,
}

impl From<&ExtendedFile> for BasicFile {
    fn from(ex_file: &ExtendedFile) -> Self {
        Self {
            header: ex_file.header,
            blocks_start: ex_file.blocks_start as u32,
            frag_index: ex_file.frag_index,
            block_offset: ex_file.block_offset,
            file_size: ex_file.file_size as u32,
            block_sizes: ex_file.block_sizes.clone(),
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, block_size: u32, block_log: u16"
)]
pub struct ExtendedFile {
    pub(crate) header: InodeHeader,
    pub(crate) blocks_start: u64,
    pub(crate) file_size: u64,
    pub(crate) sparse: u64,
    pub(crate) link_count: u32,
    pub(crate) frag_index: u32,
    pub(crate) block_offset: u32,
    pub(crate) xattr_index: u32,
    #[deku(count = "block_count(block_size, block_log, *frag_index, *file_size)")]
    pub(crate) block_sizes: Vec<u32>,
}

fn block_count(block_size: u32, block_log: u16, fragment: u32, file_size: u64) -> u64 {
    const NO_FRAGMENT: u32 = 0xffffffff;

    if fragment == NO_FRAGMENT {
        (file_size + u64::from(block_size) - 1) >> block_log
    } else {
        file_size >> block_log
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicSymlink {
    pub(crate) header: InodeHeader,
    pub(crate) link_count: u32,
    pub(crate) target_size: u32,
    #[deku(count = "target_size")]
    pub(crate) target_path: Vec<u8>,
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicDeviceSpecialFile {
    pub(crate) header: InodeHeader,
    pub(crate) link_count: u32,
    pub(crate) device_number: u32,
}
