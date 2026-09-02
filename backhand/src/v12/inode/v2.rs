//! On-disk inode layout for SquashFS v2
//!
//! These structs model the bytes and nothing else. [`super::Inode`] holds the
//! form the rest of the reader uses.

use deku::prelude::*;

use crate::v12::data::DataSize;
use crate::v12::dir::DirectoryIndex;

/// `guid` value that means "the same id as the owner"
pub const GUID_SAME_AS_UID: u8 = 255;

/// Inode types, as v2 numbers them
#[derive(Debug, DekuRead, Clone, Copy, PartialEq, Eq)]
#[deku(id_type = "u8", bits = "4")]
#[deku(
    endian = "endian",
    bit_order = "order",
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order"
)]
#[rustfmt::skip]
#[repr(u8)]
pub enum InodeId {
    Directory         = 1,
    File              = 2,
    Symlink           = 3,
    BlockDevice       = 4,
    CharacterDevice   = 5,
    NamedPipe         = 6,
    Socket            = 7,
    ExtendedDirectory = 8,
}

/// `squashfs_base_inode_header_2`: `type:4, mode:12, uid:8, guid:8`
///
/// Four bytes. Unlike v3 there is no `mtime` and no `inode_number`; the time
/// lives in the per-type body, and inodes have no numbers at all.
#[derive(Debug, DekuRead, Clone, Copy, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct BaseHeader {
    pub id: InodeId,
    #[deku(bits = "12")]
    pub mode: u16,
    #[deku(bits = "8")]
    pub uid: u8,
    #[deku(bits = "8")]
    pub guid: u8,
}

/// `squashfs_dir_inode_header_2`: `file_size:19, offset:13, mtime, start_block:24`
#[derive(Debug, DekuRead, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct Directory {
    #[deku(bits = "19")]
    pub file_size: u32,
    #[deku(bits = "13")]
    pub offset: u16,
    pub mtime: u32,
    #[deku(bits = "24")]
    pub start_block: u32,
}

/// `squashfs_ldir_inode_header_2`: as [`Directory`], with a wider size and an index
#[derive(Debug, DekuRead, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct ExtendedDirectory {
    #[deku(bits = "27")]
    pub file_size: u32,
    #[deku(bits = "13")]
    pub offset: u16,
    pub mtime: u32,
    #[deku(bits = "24")]
    pub start_block: u32,
    #[deku(assert = "*i_count < 256")]
    pub i_count: u16,
    #[deku(count = "*i_count")]
    pub dir_index: Vec<DirectoryIndex>,
}

/// `squashfs_reg_inode_header_2`
///
/// The block list is 32 bits per entry. The struct in the reference header
/// declares it as `unsigned short`, but the code that reads it uses
/// `sizeof(unsigned int)`; the declaration is stale.
#[derive(Debug, DekuRead, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order, block_size: u32, block_log: u16",
    endian = "endian",
    bit_order = "order"
)]
pub struct File {
    pub mtime: u32,
    pub start_block: u32,
    pub fragment: u32,
    pub offset: u32,
    pub file_size: u32,
    #[deku(count = "super::block_count(block_size, block_log, *fragment, u64::from(*file_size))")]
    pub block_sizes: Vec<DataSize>,
}

/// `squashfs_symlink_inode_header_2`
#[derive(Debug, DekuRead, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct Symlink {
    pub target_size: u16,
    #[deku(count = "*target_size")]
    pub target_path: Vec<u8>,
}

/// `squashfs_dev_inode_header_2`
#[derive(Debug, DekuRead, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct Device {
    pub rdev: u16,
}

#[cfg(test)]
mod tests {
    use deku::ctx::{Endian, Order};

    use super::*;

    #[test]
    fn little_endian_base_header_matches_a_real_image() {
        // The root inode of an image from mksquashfs 2.2: a directory with
        // mode 0755, uid index 0, and the "no group" guid sentinel.
        let bytes = {
            // type 1 (directory), mode 0755, uid index 0, guid sentinel 255
            let packed: u32 = 1 | (0o755 << 4) | (255 << 24);
            packed.to_le_bytes().to_vec()
        };
        let mut cursor = std::io::Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);
        let base =
            BaseHeader::from_reader_with_ctx(&mut reader, (Endian::Little, Order::Lsb0)).unwrap();

        assert_eq!(base.id, InodeId::Directory);
        assert_eq!(base.mode, 0o755);
        assert_eq!(base.uid, 0);
        assert_eq!(base.guid, GUID_SAME_AS_UID);
    }

    #[test]
    fn little_endian_directory_body_matches_a_real_image() {
        // Same image: file_size 38, offset 79, start_block 0.
        let mut bytes = vec![];
        let head: u32 = 38 | (79 << 19);
        bytes.extend_from_slice(&head.to_le_bytes());
        bytes.extend_from_slice(&1_788_318_820u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()[..3]);

        let mut cursor = std::io::Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);
        let dir =
            Directory::from_reader_with_ctx(&mut reader, (Endian::Little, Order::Lsb0)).unwrap();

        assert_eq!(dir.file_size, 38);
        assert_eq!(dir.offset, 79);
        assert_eq!(dir.mtime, 1_788_318_820);
        assert_eq!(dir.start_block, 0);
    }

    #[test]
    fn big_endian_base_header_packs_from_the_top() {
        let packed: u32 = (1 << 28) | (0o755 << 16) | (3 << 8) | 4;
        let mut cursor = std::io::Cursor::new(packed.to_be_bytes().to_vec());
        let mut reader = Reader::new(&mut cursor);
        let base =
            BaseHeader::from_reader_with_ctx(&mut reader, (Endian::Big, Order::Msb0)).unwrap();

        assert_eq!(base.id, InodeId::Directory);
        assert_eq!(base.mode, 0o755);
        assert_eq!(base.uid, 3);
        assert_eq!(base.guid, 4);
    }
}
