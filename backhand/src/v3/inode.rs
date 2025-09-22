//! Index Node for file or directory

use core::fmt;
use std::io::{Cursor, Write};

use deku::prelude::*;

use crate::kind::Kind;
use crate::v3::data::DataSize;
use crate::v3::dir::DirectoryIndex;
use crate::v3::entry::Entry;
use crate::v3::metadata::MetadataWriter;
use crate::v3::squashfs::SuperBlock;

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "bytes_used: u64, block_size: u32, block_log: u16, type_endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "type_endian",
    bit_order = "order"
)]
pub struct Inode {
    pub id: InodeId,
    pub header: InodeHeader,
    #[deku(ctx = "*id, bytes_used, block_size, block_log")]
    pub inner: InodeInner,
}

impl Inode {
    pub fn new(id: InodeId, header: InodeHeader, inner: InodeInner) -> Self {
        Inode { id, header, inner }
    }

    /// Write to `m_writer`, creating Entry
    pub(crate) fn to_bytes<'a>(
        &self,
        name: &'a [u8],
        m_writer: &mut MetadataWriter,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Entry<'a> {
        let mut inode_bytes = Cursor::new(vec![]);
        let mut writer = Writer::new(&mut inode_bytes);
        self.to_writer(
            &mut writer,
            (
                0xffff_ffff_ffff_ffff, // bytes_used is unused for ctx. set to max
                superblock.block_size,
                superblock.block_log,
                kind.inner.type_endian,
                kind.inner.bit_order.unwrap(),
            ),
        )
        .unwrap();
        let start = m_writer.metadata_start;
        let offset = m_writer.uncompressed_bytes.len() as u16;
        m_writer.write(inode_bytes.get_ref()).unwrap();

        Entry {
            start,
            offset,
            inode: self.header.inode_number,
            t: self.id,
            name_size: name.len() as u16 - 1,
            name,
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, Copy, PartialEq, Eq)]
#[deku(id_type = "u8", bits = "4")]
#[deku(endian = "endian", bit_order = "order", ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order")]
#[rustfmt::skip]
#[repr(u8)]
pub enum InodeId {
    BasicDirectory       = 1,
    BasicFile            = 2,
    BasicSymlink         = 3,
    BasicBlockDevice     = 4,
    BasicCharacterDevice = 5,
    BasicNamedPipe       = 6, // aka FIFO
    BasicSocket          = 7,
    ExtendedDirectory    = 8,
    ExtendedFile         = 9,
    ExtendedSymlink      = 10,
    ExtendedBlockDevice  = 11,
    ExtendedCharacterDevice = 12,
    ExtendedNamedPipe    = 13,
    ExtendedSocket       = 14
}

impl InodeId {
    pub(crate) fn into_base_type(self) -> Self {
        match self {
            Self::ExtendedDirectory => InodeId::BasicDirectory,
            Self::ExtendedFile => InodeId::BasicFile,
            _ => self,
        }
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order, id: InodeId, bytes_used: u64, block_size: u32, block_log: u16",
    endian = "endian",
    bit_order = "order",
    id = "id"
)]
pub enum InodeInner {
    #[deku(id = "InodeId::BasicDirectory")]
    BasicDirectory(BasicDirectory),

    #[deku(id = "InodeId::BasicFile")]
    BasicFile(#[deku(ctx = "block_size, block_log")] BasicFile),

    #[deku(id = "InodeId::BasicSymlink")]
    BasicSymlink(BasicSymlink),

    #[deku(id = "InodeId::BasicBlockDevice")]
    BasicBlockDevice(BasicDeviceSpecialFile),

    #[deku(id = "InodeId::BasicCharacterDevice")]
    BasicCharacterDevice(BasicDeviceSpecialFile),

    #[deku(id = "InodeId::BasicNamedPipe")]
    BasicNamedPipe(IPCNode),

    #[deku(id = "InodeId::BasicSocket")]
    BasicSocket(IPCNode),

    #[deku(id = "InodeId::ExtendedDirectory")]
    ExtendedDirectory(ExtendedDirectory),

    #[deku(id = "InodeId::ExtendedFile")]
    ExtendedFile(#[deku(ctx = "bytes_used as u32, block_size, block_log")] ExtendedFile),

    #[deku(id = "InodeId::ExtendedSymlink")]
    ExtendedSymlink(ExtendedSymlink),

    #[deku(id = "InodeId::ExtendedBlockDevice")]
    ExtendedBlockDevice(ExtendedDeviceSpecialFile),

    #[deku(id = "InodeId::ExtendedCharacterDevice")]
    ExtendedCharacterDevice(ExtendedDeviceSpecialFile),

    #[deku(id = "InodeId::ExtendedNamedPipe")]
    ExtendedNamedPipe(ExtendedIPCNode),

    #[deku(id = "InodeId::ExtendedSocket")]
    ExtendedSocket(ExtendedIPCNode),
}

#[derive(Debug, DekuRead, DekuWrite, Clone, Copy, PartialEq, Eq, Default)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct InodeHeader {
    #[deku(bits = "12")]
    pub permissions: u16,
    /// index into id table
    #[deku(bits = "8")]
    pub uid: u16,
    /// index into id table
    #[deku(bits = "8")]
    pub gid: u16,
    pub mtime: u32,
    pub inode_number: u32,
}

// `squashfs_dir_inode_header`
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct BasicDirectory {
    pub nlink: u32,
    #[deku(bits = "19")]
    pub file_size: u32,
    #[deku(bits = "13")]
    pub offset: u32,
    pub start_block: u32,
    pub parent_inode: u32,
}

// `squashfs_ldir_inode_header`
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct ExtendedDirectory {
    pub link_count: u32,
    #[deku(bits = "27")]
    pub file_size: u32,
    #[deku(bits = "13")]
    pub block_offset: u64,
    pub start_block: u32,
    #[deku(assert = "*i_count < 256")]
    pub i_count: u16,
    pub parent_inode: u32,
    #[deku(count = "*i_count")]
    pub dir_index: Vec<DirectoryIndex>,
}

// #[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
// #[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
// pub struct ExtendedDirectory {
//     pub link_count: u32,
//     pub file_size: u32,
//     pub block_index: u32,
//     pub parent_inode: u32,
//     #[deku(assert = "*index_count < 256")]
//     pub index_count: u16,
//     pub block_offset: u16,
//     pub xattr_index: u32,
//     #[deku(count = "*index_count")]
//     pub dir_index: Vec<DirectoryIndex>,
// }

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order, block_size: u32, block_log: u16",
    endian = "endian",
    bit_order = "order"
)]
pub struct BasicFile {
    pub blocks_start: u64, // TODO: this looks like a u64????
    // this is more, "fragment_offset"
    pub frag: u32,
    pub block_offset: u32,
    #[deku(bytes = "4")]
    pub file_size: u64,
    #[deku(count = "block_count(block_size, block_log, *frag, *file_size as u64)")]
    pub block_sizes: Vec<DataSize>,
}

// impl From<&ExtendedFile> for BasicFile {
//     fn from(ex_file: &ExtendedFile) -> Self {
//         Self {
//             blocks_start: ex_file.blocks_start as u32,
//             frag_index: ex_file.frag_index,
//             block_offset: ex_file.block_offset,
//             file_size: ex_file.file_size as u32,
//             block_sizes: ex_file.block_sizes.clone(),
//         }
//     }
// }

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order, bytes_used: u32, block_size: u32, block_log: u16",
    endian = "endian",
    bit_order = "order"
)]
pub struct ExtendedFile {
    pub blocks_start: u64,
    pub file_size: u64,
    pub sparse: u64,
    pub link_count: u32,
    pub frag_index: u32,
    pub block_offset: u32,
    pub xattr_index: u32,
    #[deku(count = "block_count(block_size, block_log, *frag_index, *file_size)")]
    pub block_sizes: Vec<DataSize>,
}

fn block_count(block_size: u32, block_log: u16, fragment: u32, file_size: u64) -> u64 {
    const NO_FRAGMENT: u32 = 0xffffffff;

    if fragment == NO_FRAGMENT {
        (file_size + u64::from(block_size) - 1) >> block_log
    } else {
        file_size >> block_log
    }
}

#[derive(DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct BasicSymlink {
    pub link_count: u32,
    #[deku(assert = "*target_size < 256")]
    pub target_size: u32,
    #[deku(count = "target_size")]
    pub target_path: Vec<u8>,
}

impl fmt::Debug for BasicSymlink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BasicSymlink")
            .field("link_count", &self.link_count)
            .field("target_size", &self.target_size)
            .field("target_path", &self.target())
            .finish()
    }
}
impl BasicSymlink {
    pub fn target(&self) -> String {
        std::str::from_utf8(&self.target_path).unwrap().to_string()
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct BasicDeviceSpecialFile {
    pub link_count: u32,
    #[deku(bytes = "2")] // v3
    pub device_number: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    bit_order = "order"
)]
pub struct IPCNode {
    pub link_count: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct ExtendedSymlink {
    pub link_count: u32,
    pub target_size: u32,
    #[deku(count = "*target_size")]
    pub target_path: Vec<u8>,
    pub xattr_index: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct ExtendedDeviceSpecialFile {
    pub link_count: u32,
    pub device_number: u32,
    pub xattr_index: u32,
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct ExtendedIPCNode {
    pub link_count: u32,
    pub xattr_index: u32,
}
