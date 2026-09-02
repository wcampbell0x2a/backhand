//! On-disk inode layout for SquashFS v1
//!
//! v1 differs from v2 in more than field widths. The 4-bit type field carries
//! two things at once, and the 4-bit uid field is too narrow on its own, so the
//! two are read together; see [`TypeField`].

use deku::prelude::*;

use crate::v12::data::BlockSizeV1;

/// Number of inode types v1 defines: directory, file, symlink, block device,
/// character device.
pub const TYPES: u8 = 5;

/// Type value that marks a named pipe or a socket
pub const IPC_TYPE: u8 = 0;

/// `guid` value that means "the same id as the owner". v1's guid field is four
/// bits wide, so the sentinel is 15 rather than v2's 255.
pub const GUID_SAME_AS_UID: u8 = 15;

/// The type a v1 inode header holds, split into its two meanings
///
/// v1 has five types but a four-bit field, so the spare range encodes which
/// bank of sixteen the 4-bit `uid` field indexes into. The reference tools read
/// it as `(t - 1) % 5 + 1` for the type and `(t - 1) / 5 * 16 + uid` for the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeField {
    /// A named pipe or socket. Which one is in the body, not the header.
    Ipc,
    /// An ordinary type, with the uid bank the raw value also encoded.
    Typed { kind: Kind, uid_bank: u32 },
}

/// A v1 inode type, after the raw value is folded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Directory,
    File,
    Symlink,
    BlockDevice,
    CharacterDevice,
}

impl TypeField {
    /// Split a raw four-bit type field into a type and a uid bank
    pub fn from_raw(raw: u8) -> Option<Self> {
        if raw == IPC_TYPE {
            return Some(Self::Ipc);
        }
        let kind = match (raw - 1) % TYPES + 1 {
            1 => Kind::Directory,
            2 => Kind::File,
            3 => Kind::Symlink,
            4 => Kind::BlockDevice,
            5 => Kind::CharacterDevice,
            _ => return None,
        };
        Some(Self::Typed { kind, uid_bank: u32::from((raw - 1) / TYPES) * 16 })
    }
}

/// `squashfs_base_inode_header_1`: `type:4, mode:12, uid:4, guid:4`
///
/// Three bytes. `raw_type` stays unfolded because the uid index needs it; use
/// [`TypeField::from_raw`] to read it.
#[derive(Debug, DekuRead, Clone, Copy, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct BaseHeader {
    #[deku(bits = "4")]
    pub raw_type: u8,
    #[deku(bits = "12")]
    pub mode: u16,
    #[deku(bits = "4")]
    pub uid: u8,
    #[deku(bits = "4")]
    pub guid: u8,
}

/// `squashfs_ipc_inode_header_1`: `type:4, offset:4`
///
/// `kind` picks socket or named pipe. `offset` is this inode's uid bank, which
/// replaces the one the header's type field would give.
#[derive(Debug, DekuRead, Clone, Copy, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct Ipc {
    #[deku(bits = "4")]
    pub kind: u8,
    #[deku(bits = "4")]
    pub offset: u8,
}

/// Type value inside an ipc inode that marks a socket
pub const IPC_SOCKET: u8 = 7;

/// `squashfs_dir_inode_header_1`: `file_size:19, offset:13, mtime, start_block:24`
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

/// `squashfs_reg_inode_header_1`
///
/// v1 has no fragments, so a file is only whole blocks, and each block size is
/// 16 bits.
#[derive(Debug, DekuRead, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order, block_size: u32, block_log: u16",
    endian = "endian",
    bit_order = "order"
)]
pub struct File {
    pub mtime: u32,
    pub start_block: u32,
    pub file_size: u32,
    #[deku(count = "block_count_no_fragment(block_size, block_log, u64::from(*file_size))")]
    pub block_sizes: Vec<BlockSizeV1>,
}

/// Blocks a file of this size occupies when none of it lives in a fragment
fn block_count_no_fragment(block_size: u32, block_log: u16, file_size: u64) -> u64 {
    (file_size + u64::from(block_size) - 1) >> block_log
}

/// `squashfs_symlink_inode_header_1`
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

/// `squashfs_dev_inode_header_1`
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
    fn a_raw_type_in_the_first_bank_gives_bank_zero() {
        for (raw, kind) in [
            (1, Kind::Directory),
            (2, Kind::File),
            (3, Kind::Symlink),
            (4, Kind::BlockDevice),
            (5, Kind::CharacterDevice),
        ] {
            assert_eq!(TypeField::from_raw(raw), Some(TypeField::Typed { kind, uid_bank: 0 }));
        }
    }

    #[test]
    fn a_raw_type_above_the_first_bank_selects_a_uid_bank() {
        // 6..=10 repeat the five types with the next bank of sixteen ids.
        assert_eq!(
            TypeField::from_raw(6),
            Some(TypeField::Typed { kind: Kind::Directory, uid_bank: 16 })
        );
        assert_eq!(
            TypeField::from_raw(10),
            Some(TypeField::Typed { kind: Kind::CharacterDevice, uid_bank: 16 })
        );
        assert_eq!(
            TypeField::from_raw(11),
            Some(TypeField::Typed { kind: Kind::Directory, uid_bank: 32 })
        );
        assert_eq!(
            TypeField::from_raw(15),
            Some(TypeField::Typed { kind: Kind::CharacterDevice, uid_bank: 32 })
        );
    }

    #[test]
    fn type_zero_is_the_ipc_inode() {
        assert_eq!(TypeField::from_raw(IPC_TYPE), Some(TypeField::Ipc));
    }

    #[test]
    fn little_endian_base_header_reads_the_four_bit_ids() {
        // type 1 (directory, bank 0), mode 0755, uid 2, guid 15.
        let packed: u32 = 1 | (0o755 << 4) | (2 << 16) | (15 << 20);
        let mut cursor = std::io::Cursor::new(packed.to_le_bytes()[..3].to_vec());
        let mut reader = Reader::new(&mut cursor);
        let base =
            BaseHeader::from_reader_with_ctx(&mut reader, (Endian::Little, Order::Lsb0)).unwrap();

        assert_eq!(base.raw_type, 1);
        assert_eq!(base.mode, 0o755);
        assert_eq!(base.uid, 2);
        assert_eq!(base.guid, GUID_SAME_AS_UID);
    }

    #[test]
    fn big_endian_base_header_reads_the_four_bit_ids() {
        // The same fields, packed from the most significant bit down.
        let packed: u32 = (1 << 20) | (0o755 << 8) | (2 << 4) | 15;
        let mut cursor = std::io::Cursor::new(packed.to_be_bytes()[1..].to_vec());
        let mut reader = Reader::new(&mut cursor);
        let base =
            BaseHeader::from_reader_with_ctx(&mut reader, (Endian::Big, Order::Msb0)).unwrap();

        assert_eq!(base.raw_type, 1);
        assert_eq!(base.mode, 0o755);
        assert_eq!(base.uid, 2);
        assert_eq!(base.guid, GUID_SAME_AS_UID);
    }

    #[test]
    fn a_file_covers_every_block_because_v1_has_no_fragments() {
        assert_eq!(block_count_no_fragment(32768, 15, 0), 0);
        assert_eq!(block_count_no_fragment(32768, 15, 1), 1);
        assert_eq!(block_count_no_fragment(32768, 15, 32768), 1);
        assert_eq!(block_count_no_fragment(32768, 15, 32769), 2);
    }
}
