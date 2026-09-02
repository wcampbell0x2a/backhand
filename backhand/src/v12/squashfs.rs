//! Superblock for SquashFS v1 and v2

use deku::prelude::*;

use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::v4::reader::BufReadSeek;
use crate::v12::inode::Layout;


/// Minor version that marks the AVM/Freetz LZMA variant of v2
pub const MINOR_LZMA: u16 = 76;

/// Highest minor version a stock v2 image uses
const MINOR_MAX: u16 = 1;

/// Largest block size v1 allows. v1 records the size in 16 bits, so this is
/// also the largest value the field can hold.
const MAX_BLOCK_SIZE_V1: u32 = 32 * 1024;
/// Largest block size v2 allows
const MAX_BLOCK_SIZE_V2: u32 = 64 * 1024;
/// Smallest block size either version allows
const MIN_BLOCK_SIZE: u32 = 4 * 1024;

/// Superblock for v1 and v2
///
/// The two share every field up to `root_inode`. v1 stops there; v2 adds the
/// three below it. v3 keeps going with a second set of 64-bit table starts,
/// which is why it cannot share this struct: reading a v2 image with the v3
/// definition runs past the superblock and into the inode table.
///
/// Both versions take every table offset from these 32-bit fields. Only v3 uses
/// the wider ones.
#[derive(Debug, Copy, Clone, DekuRead, PartialEq, Eq)]
#[deku(
    endian = "ctx_type_endian",
    ctx = "ctx_magic: [u8; 4], ctx_version_major: u16, ctx_type_endian: deku::ctx::Endian"
)]
pub struct SuperBlock {
    #[deku(assert_eq = "ctx_magic")]
    pub magic: [u8; 4],
    pub inode_count: u32,
    pub bytes_used: u32,
    pub uid_start: u32,
    pub guid_start: u32,
    pub inode_table_start: u32,
    pub directory_table_start: u32,
    /// Asserted against the kind, because v1, v2 and v3 share a magic and a
    /// superblock prefix. Without this a v2 image parses as v3 and only fails
    /// later, which would make the CLI's kind search pick the first kind that
    /// merely looks plausible.
    #[deku(assert_eq = "ctx_version_major")]
    pub version_major: u16,
    /// 76 (`'L'`) marks the AVM/Freetz LZMA variant. Stock images use 0 or 1.
    pub version_minor: u16,
    /// The v1 block size. v2 keeps the field but records the real value in
    /// `block_size_v2`.
    pub block_size_1: u16,
    pub block_log: u16,
    pub flags: u8,
    pub no_uids: u8,
    pub no_guids: u8,
    pub mkfs_time: u32,
    /// The root inode, packed as a metadata block and an offset inside it
    pub root_inode: u64,
    #[deku(cond = "ctx_version_major >= 2", default = "0")]
    block_size_v2: u32,
    #[deku(cond = "ctx_version_major >= 2", default = "0")]
    fragment_count: u32,
    #[deku(cond = "ctx_version_major >= 2", default = "0")]
    fragment_table_start_v2: u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Flags {
    InodesStoredUncompressed = 0b0000_0001,
    DataBlockStoredUncompressed = 0b0000_0010,
    FragmentsStoredUncompressed = 0b0000_0100,
    NoFragments = 0b0000_1000,
    AlwaysFragments = 0b0001_0000,
    Duplicates = 0b0010_0000,
    Exportable = 0b0100_0000,
    CheckData = 0b1000_0000,
}

impl SuperBlock {
    /// The image's block size, from whichever field its version uses
    pub fn block_size(&self, layout: Layout) -> u32 {
        match layout {
            Layout::V1 => u32::from(self.block_size_1),
            Layout::V2 => self.block_size_v2,
        }
    }

    /// Number of fragments. v1 has none.
    pub fn fragments(&self, layout: Layout) -> u32 {
        match layout {
            Layout::V1 => 0,
            Layout::V2 => self.fragment_count,
        }
    }

    /// Start of the fragment lookup table, when the image has fragments
    pub fn fragment_table_start(&self, layout: Layout) -> Option<u32> {
        match layout {
            Layout::V1 => None,
            Layout::V2 if self.fragment_count == 0 => None,
            Layout::V2 => Some(self.fragment_table_start_v2),
        }
    }

    /// Where the directory table ends
    ///
    /// The tables run superblock, inodes, directories, fragments, uids, guids.
    /// The directory table therefore ends at the fragment table when there is
    /// one, and at the uid table otherwise.
    pub fn directory_table_end(&self, layout: Layout) -> u32 {
        self.fragment_table_start(layout).unwrap_or(self.uid_start)
    }

    /// Inodes are stored uncompressed
    pub fn inodes_uncompressed(&self) -> bool {
        self.flags & Flags::InodesStoredUncompressed as u8 != 0
    }

    /// A check byte follows each metadata block length
    pub fn check_data(&self) -> bool {
        self.flags & Flags::CheckData as u8 != 0
    }

    /// The image uses LZMA rather than gzip
    pub fn is_lzma(&self) -> bool {
        self.version_minor == MINOR_LZMA
    }
}

/// Split a packed inode reference into its metadata block and its offset
///
/// v1 and v2 address an inode by position, not by number, and pack both halves
/// into one value.
pub fn inode_block_and_offset(inode: u64) -> (u32, u16) {
    ((inode >> 16) as u32, (inode & 0xffff) as u16)
}

/// Read the superblock and check what the rest of the reader relies on
pub fn superblock<R: BufReadSeek + ?Sized>(
    reader: &mut R,
    kind: &Kind,
) -> Result<(SuperBlock, Layout), BackhandError> {
    let layout = Layout::from_major(kind.inner.version_major)
        .ok_or(BackhandError::CorruptedOrInvalidSquashfs)?;

    let mut container = Reader::new(reader);
    let superblock = SuperBlock::from_reader_with_ctx(
        &mut container,
        (kind.inner.magic, kind.inner.version_major, kind.inner.type_endian),
    )?;
    trace!("{:02x?}", superblock);

    // Neither version records a compressor id, so the minor version is the only
    // thing that says whether the blocks are gzip or LZMA. The kind and the
    // image must agree, or a gzip kind would try to inflate LZMA blocks and
    // report a corrupt image instead of the wrong kind.
    if superblock.is_lzma() != (kind.inner.version_minor == MINOR_LZMA) {
        error!(
            "kind minor({}) does not match image minor({})",
            kind.inner.version_minor, superblock.version_minor
        );
        return Err(BackhandError::CorruptedOrInvalidSquashfs);
    }

    if !superblock.is_lzma() && superblock.version_minor > MINOR_MAX {
        error!("minor({}) is not a known minor version", superblock.version_minor);
        return Err(BackhandError::CorruptedOrInvalidSquashfs);
    }

    let block_size = superblock.block_size(layout);
    let max = match layout {
        Layout::V1 => MAX_BLOCK_SIZE_V1,
        Layout::V2 => MAX_BLOCK_SIZE_V2,
    };
    let power_of_two = block_size != 0 && (block_size & (block_size - 1)) == 0;
    if !(MIN_BLOCK_SIZE..=max).contains(&block_size) || !power_of_two {
        error!("block_size({:#02x}) invalid", block_size);
        return Err(BackhandError::CorruptedOrInvalidSquashfs);
    }

    if block_size.ilog2() != u32::from(superblock.block_log) {
        error!("block size.log2() != block_log");
        return Err(BackhandError::CorruptedOrInvalidSquashfs);
    }

    Ok((superblock, layout))
}

#[cfg(test)]
mod tests {
    use deku::ctx::Endian;

    use super::*;

    /// The first 51 bytes of an image from mksquashfs 1.3
    fn v1_superblock_bytes() -> Vec<u8> {
        let mut b = vec![];
        b.extend_from_slice(b"hsqs");
        for v in [0xbu32, 0x49553, 0x4954f, 0x0, 0x4946d, 0x494df] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&1u16.to_le_bytes()); // major
        b.extend_from_slice(&0u16.to_le_bytes()); // minor
        b.extend_from_slice(&0x8000u16.to_le_bytes()); // block_size_1
        b.extend_from_slice(&15u16.to_le_bytes()); // block_log
        b.extend_from_slice(&[0, 1, 0]); // flags, no_uids, no_guids
        b.extend_from_slice(&0x6a97_954fu32.to_le_bytes());
        b.extend_from_slice(&0xa2u64.to_le_bytes());
        b
    }

    #[test]
    fn a_v1_superblock_ends_after_the_root_inode() {
        let bytes = v1_superblock_bytes();
        assert_eq!(bytes.len(), 51);

        let mut cursor = std::io::Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);
        let sb =
            SuperBlock::from_reader_with_ctx(&mut reader, (*b"hsqs", 1, Endian::Little)).unwrap();

        assert_eq!(sb.version_major, 1);
        assert_eq!(sb.block_size(Layout::V1), 32768);
        assert_eq!(sb.block_log, 15);
        assert_eq!(sb.root_inode, 0xa2);
        // v1 has no fragments, so the directory table runs to the uid table.
        assert_eq!(sb.fragments(Layout::V1), 0);
        assert_eq!(sb.fragment_table_start(Layout::V1), None);
        assert_eq!(sb.directory_table_end(Layout::V1), 0x4954f);
    }

    #[test]
    fn a_v1_superblock_read_with_a_v2_kind_is_rejected() {
        let mut cursor = std::io::Cursor::new(v1_superblock_bytes());
        let mut reader = Reader::new(&mut cursor);
        // The major assert is what keeps the CLI's kind search honest across
        // three versions that share a magic.
        assert!(
            SuperBlock::from_reader_with_ctx(&mut reader, (*b"hsqs", 2, Endian::Little)).is_err()
        );
    }

    #[test]
    fn the_lzma_minor_is_the_freetz_marker() {
        assert_eq!(MINOR_LZMA, u16::from(b'L'));
    }

    #[test]
    fn an_inode_reference_splits_into_a_block_and_an_offset() {
        assert_eq!(inode_block_and_offset(0xcb), (0, 203));
        assert_eq!(inode_block_and_offset(0xa2), (0, 162));
        assert_eq!(inode_block_and_offset(0x0002_9471), (2, 0x9471));
    }
}
