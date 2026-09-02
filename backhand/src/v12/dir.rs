//! Directory table for SquashFS v1 and v2
//!
//! v1 and v2 share this format exactly; the reference tools use the same
//! `squashfs_dir_header_2` and `squashfs_dir_entry_2` for both. Against v3, the
//! header has no inode number and the entry has no inode offset, because these
//! versions address an inode by its position rather than by a number.

use core::fmt;
use std::ffi::OsStr;
use std::path::{Component, Path};

use deku::prelude::*;

use crate::BackhandError;
use crate::v4::unix_string::OsStrExt;

/// `squashfs_dir_header_2`: `count:8, start_block:24`
#[derive(Debug, DekuRead, Clone, PartialEq, Eq)]
#[deku(ctx = "type_endian: deku::ctx::Endian, order: deku::ctx::Order")]
#[deku(bit_order = "order")]
#[deku(endian = "type_endian")]
pub struct Dir {
    /// One less than the number of entries that follow.
    #[deku(bits = "8")]
    pub(crate) count: u32,
    /// Offset of the inode metadata block, relative to the inode table start.
    #[deku(bits = "24")]
    pub(crate) start: u32,
    #[deku(count = "*count + 1")]
    pub(crate) dir_entries: Vec<DirEntry>,
}

/// `squashfs_dir_entry_2`: `offset:13, type:3, size:8`, then the name
#[derive(DekuRead, Clone, PartialEq, Eq)]
#[deku(
    endian = "endian",
    bit_order = "order",
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order"
)]
pub struct DirEntry {
    /// Offset of the inode inside its metadata block.
    #[deku(bits = "13")]
    pub(crate) offset: u16,
    /// The inode type, narrowed to three bits.
    ///
    /// Three bits cannot hold the extended-directory type, so this is only a
    /// hint. The reference tools use it for tracing alone. Read the inode and
    /// use its own type instead.
    #[deku(bits = "3")]
    pub(crate) t: u8,
    /// One less than the length of the entry name.
    #[deku(bits = "8")]
    pub(crate) name_size: u16,
    #[deku(count = "*name_size + 1")]
    pub(crate) name: Vec<u8>,
}

impl fmt::Debug for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirEntry")
            .field("offset", &self.offset)
            .field("t", &self.t)
            .field("name_size", &self.name_size)
            .field("name", &self.name())
            .finish()
    }
}

impl DirEntry {
    pub fn name(&self) -> Result<&Path, BackhandError> {
        // allow root and nothing else
        if self.name == Component::RootDir.as_os_str().as_bytes() {
            return Ok(Path::new(Component::RootDir.as_os_str()));
        }
        let path = Path::new(OsStr::from_bytes(&self.name));
        // if not a simple filename, return an error
        let filename = path.file_name().map(OsStrExt::as_bytes);
        if filename != Some(&self.name) {
            return Err(BackhandError::InvalidFilePath);
        }
        Ok(path)
    }
}

/// `squashfs_dir_index_2`: `index:27, start_block:29, size:8`, then the name
///
/// Only the extended directory inode carries these, and only as a lookup
/// shortcut. The reader walks the whole listing instead, so nothing depends on
/// this parse being right; it exists so the inode consumes the correct number
/// of bytes.
#[derive(DekuRead, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct DirectoryIndex {
    #[deku(bits = "27")]
    pub(crate) index: u32,
    #[deku(bits = "29")]
    pub(crate) start: u32,
    #[deku(bits = "8")]
    pub(crate) name_size: u8,
    #[deku(count = "*name_size + 1")]
    pub(crate) name: Vec<u8>,
}

impl fmt::Debug for DirectoryIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirectoryIndex")
            .field("index", &self.index)
            .field("start", &self.start)
            .field("name_size", &self.name_size)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use deku::ctx::{Endian, Order};
    use deku::prelude::*;

    use super::*;

    /// Build the bytes of one little-endian directory header with a single
    /// entry, packed the way a compiler lays the C bitfields out on a
    /// little-endian target.
    fn le_header_with_one_entry(name: &[u8]) -> Vec<u8> {
        let mut out = vec![];
        // count:8 = 0 (one entry), start_block:24 = 0x40
        out.extend_from_slice(&[0x00, 0x40, 0x00, 0x00]);
        // offset:13 = 0x1a, type:3 = 2, size:8 = name.len() - 1
        let packed: u32 = 0x1a | (2 << 13) | ((name.len() as u32 - 1) << 16);
        out.extend_from_slice(&packed.to_le_bytes()[..3]);
        out.extend_from_slice(name);
        out
    }

    #[test]
    fn little_endian_header_and_entry_round_trip() {
        let bytes = le_header_with_one_entry(b"hello.txt");
        let mut cursor = std::io::Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);
        let dir = Dir::from_reader_with_ctx(&mut reader, (Endian::Little, Order::Lsb0)).unwrap();

        assert_eq!(dir.count, 0);
        assert_eq!(dir.start, 0x40);
        assert_eq!(dir.dir_entries.len(), 1);

        let entry = &dir.dir_entries[0];
        assert_eq!(entry.offset, 0x1a);
        assert_eq!(entry.t, 2);
        assert_eq!(entry.name_size, 8);
        assert_eq!(entry.name, b"hello.txt");
        assert_eq!(entry.name().unwrap(), Path::new("hello.txt"));
    }

    #[test]
    fn an_entry_name_with_a_path_separator_is_rejected() {
        let bytes = le_header_with_one_entry(b"a/b");
        let mut cursor = std::io::Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);
        let dir = Dir::from_reader_with_ctx(&mut reader, (Endian::Little, Order::Lsb0)).unwrap();

        assert!(dir.dir_entries[0].name().is_err());
    }

    #[test]
    fn big_endian_header_and_entry_round_trip() {
        let name = b"be.bin";
        let mut bytes = vec![];
        // count:8 then start_block:24, most significant bit first
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x40]);
        // offset:13, type:3, size:8 packed from the top of a 24-bit field
        let packed: u32 = (0x1a << 11) | (2 << 8) | (name.len() as u32 - 1);
        bytes.extend_from_slice(&packed.to_be_bytes()[1..]);
        bytes.extend_from_slice(name);

        let mut cursor = std::io::Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);
        let dir = Dir::from_reader_with_ctx(&mut reader, (Endian::Big, Order::Msb0)).unwrap();

        assert_eq!(dir.start, 0x40);
        let entry = &dir.dir_entries[0];
        assert_eq!(entry.offset, 0x1a);
        assert_eq!(entry.t, 2);
        assert_eq!(entry.name, name);
    }
}
