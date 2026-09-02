//! Inodes for SquashFS v1 and v2
//!
//! The two versions lay inodes out differently, so each has its own module of
//! deku structs. Both are normalized into the one [`Inode`] here, and every
//! version quirk is resolved during that step. Nothing past this module needs
//! to know which version an image is.

pub mod v1;
pub mod v2;

use deku::prelude::*;
use no_std_io2::io::Cursor;

use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::v12::data::DataSize;
use crate::v12::squashfs::SuperBlock;

/// Which on-disk layout an image uses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    V1,
    V2,
}

impl Layout {
    pub fn from_major(major: u16) -> Option<Self> {
        match major {
            1 => Some(Self::V1),
            2 => Some(Self::V2),
            _ => None,
        }
    }
}

/// Which id an inode's group refers to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GidRef {
    /// An index into the guid table
    Index(u32),
    /// The sentinel that means the group is the same as the owner
    SameAsUid,
}

/// The fields every v1 and v2 inode has
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InodeHeader {
    pub permissions: u16,
    /// An index into the uid table, with the v1 bank already applied
    pub uid_index: u32,
    pub gid: GidRef,
    pub mtime: u32,
}

/// An inode, in the one form both versions normalize to
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inode {
    pub header: InodeHeader,
    pub inner: InodeInner,
}

/// The part of an inode that depends on its type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InodeInner {
    Directory(Directory),
    File(File),
    Symlink(Vec<u8>),
    BlockDevice(u16),
    CharacterDevice(u16),
    NamedPipe,
    Socket,
}

/// A directory's listing, as a location in the directory table
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Directory {
    /// Bytes of listing to read.
    ///
    /// v3 and v4 store this three larger than the real length; v1 and v2 do
    /// not, so nothing is subtracted here.
    pub file_size: u32,
    /// Offset inside the metadata block named by `start_block`
    pub offset: u16,
    /// Offset of the metadata block, relative to the directory table start
    pub start_block: u32,
}

/// A file's data, with v1's narrower block sizes already widened
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub start_block: u32,
    /// The fragment holding the tail, or [`NO_FRAGMENT`] when there is none.
    /// v1 has no fragments, so it is always [`NO_FRAGMENT`].
    pub fragment: u32,
    /// Offset of the tail inside its fragment. Zero for v1.
    pub offset: u32,
    pub file_size: u32,
    pub block_sizes: Vec<DataSize>,
}

/// A fragment index that means the file has no tail fragment
pub const NO_FRAGMENT: u32 = 0xffff_ffff;

/// Blocks a file of this size occupies, given whether its tail is in a fragment
pub(crate) fn block_count(block_size: u32, block_log: u16, fragment: u32, file_size: u64) -> u64 {
    if fragment == NO_FRAGMENT {
        (file_size + u64::from(block_size) - 1) >> block_log
    } else {
        file_size >> block_log
    }
}

impl Inode {
    /// Read one inode out of the decompressed inode table
    ///
    /// v1 and v2 reach an inode by position, not by number, so `pos` is a byte
    /// offset into `table`.
    pub fn from_table(
        table: &[u8],
        pos: usize,
        layout: Layout,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<Self, BackhandError> {
        let bytes = table.get(pos..).ok_or(BackhandError::CorruptedOrInvalidSquashfs)?;
        let block_size = superblock.block_size(layout);
        match layout {
            Layout::V1 => Self::read_v1(bytes, superblock, block_size, kind),
            Layout::V2 => Self::read_v2(bytes, superblock, block_size, kind),
        }
    }

    fn read_v2(
        bytes: &[u8],
        superblock: &SuperBlock,
        block_size: u32,
        kind: &Kind,
    ) -> Result<Self, BackhandError> {
        let endian = kind.inner.type_endian;
        let order = kind.inner.bit_order.unwrap_or(deku::ctx::Order::Lsb0);
        let mut cursor = Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);

        let base = v2::BaseHeader::from_reader_with_ctx(&mut reader, (endian, order))?;
        let gid = if base.guid == v2::GUID_SAME_AS_UID {
            GidRef::SameAsUid
        } else {
            GidRef::Index(u32::from(base.guid))
        };

        // Only the directory and file bodies carry a time. Everything else
        // takes the image's build time, as the reference tools do.
        let mut header = InodeHeader {
            permissions: base.mode,
            uid_index: u32::from(base.uid),
            gid,
            mtime: superblock.mkfs_time,
        };

        let inner = match base.id {
            v2::InodeId::Directory => {
                let dir = v2::Directory::from_reader_with_ctx(&mut reader, (endian, order))?;
                header.mtime = dir.mtime;
                InodeInner::Directory(Directory {
                    file_size: dir.file_size,
                    offset: dir.offset,
                    start_block: dir.start_block,
                })
            }
            v2::InodeId::ExtendedDirectory => {
                let dir =
                    v2::ExtendedDirectory::from_reader_with_ctx(&mut reader, (endian, order))?;
                header.mtime = dir.mtime;
                InodeInner::Directory(Directory {
                    file_size: dir.file_size,
                    offset: dir.offset,
                    start_block: dir.start_block,
                })
            }
            v2::InodeId::File => {
                let file = v2::File::from_reader_with_ctx(
                    &mut reader,
                    (endian, order, block_size, superblock.block_log),
                )?;
                header.mtime = file.mtime;
                InodeInner::File(File {
                    start_block: file.start_block,
                    fragment: file.fragment,
                    offset: file.offset,
                    file_size: file.file_size,
                    block_sizes: file.block_sizes,
                })
            }
            v2::InodeId::Symlink => {
                let link = v2::Symlink::from_reader_with_ctx(&mut reader, (endian, order))?;
                InodeInner::Symlink(link.target_path)
            }
            v2::InodeId::BlockDevice => {
                let dev = v2::Device::from_reader_with_ctx(&mut reader, (endian, order))?;
                InodeInner::BlockDevice(dev.rdev)
            }
            v2::InodeId::CharacterDevice => {
                let dev = v2::Device::from_reader_with_ctx(&mut reader, (endian, order))?;
                InodeInner::CharacterDevice(dev.rdev)
            }
            v2::InodeId::NamedPipe => InodeInner::NamedPipe,
            v2::InodeId::Socket => InodeInner::Socket,
        };

        Ok(Self { header, inner })
    }

    fn read_v1(
        bytes: &[u8],
        superblock: &SuperBlock,
        block_size: u32,
        kind: &Kind,
    ) -> Result<Self, BackhandError> {
        let endian = kind.inner.type_endian;
        let order = kind.inner.bit_order.unwrap_or(deku::ctx::Order::Lsb0);
        let mut cursor = Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);

        let base = v1::BaseHeader::from_reader_with_ctx(&mut reader, (endian, order))?;
        let type_field = v1::TypeField::from_raw(base.raw_type)
            .ok_or(BackhandError::CorruptedOrInvalidSquashfs)?;

        let gid = if base.guid == v1::GUID_SAME_AS_UID {
            GidRef::SameAsUid
        } else {
            GidRef::Index(u32::from(base.guid))
        };

        let mut header = InodeHeader {
            permissions: base.mode,
            uid_index: u32::from(base.uid),
            gid,
            mtime: superblock.mkfs_time,
        };

        let inner = match type_field {
            v1::TypeField::Ipc => {
                let ipc = v1::Ipc::from_reader_with_ctx(&mut reader, (endian, order))?;
                // The ipc inode carries its own uid bank, in place of the one
                // the type field encodes for every other type.
                header.uid_index = u32::from(ipc.offset) * 16 + u32::from(base.uid);
                if ipc.kind == v1::IPC_SOCKET { InodeInner::Socket } else { InodeInner::NamedPipe }
            }
            v1::TypeField::Typed { kind: v1_kind, uid_bank } => {
                header.uid_index = uid_bank + u32::from(base.uid);
                match v1_kind {
                    v1::Kind::Directory => {
                        let dir =
                            v1::Directory::from_reader_with_ctx(&mut reader, (endian, order))?;
                        header.mtime = dir.mtime;
                        InodeInner::Directory(Directory {
                            file_size: dir.file_size,
                            offset: dir.offset,
                            start_block: dir.start_block,
                        })
                    }
                    v1::Kind::File => {
                        let file = v1::File::from_reader_with_ctx(
                            &mut reader,
                            (endian, order, block_size, superblock.block_log),
                        )?;
                        header.mtime = file.mtime;
                        InodeInner::File(File {
                            start_block: file.start_block,
                            fragment: NO_FRAGMENT,
                            offset: 0,
                            file_size: file.file_size,
                            block_sizes: file.block_sizes.into_iter().map(DataSize::from).collect(),
                        })
                    }
                    v1::Kind::Symlink => {
                        let link = v1::Symlink::from_reader_with_ctx(&mut reader, (endian, order))?;
                        InodeInner::Symlink(link.target_path)
                    }
                    v1::Kind::BlockDevice => {
                        let dev = v1::Device::from_reader_with_ctx(&mut reader, (endian, order))?;
                        InodeInner::BlockDevice(dev.rdev)
                    }
                    v1::Kind::CharacterDevice => {
                        let dev = v1::Device::from_reader_with_ctx(&mut reader, (endian, order))?;
                        InodeInner::CharacterDevice(dev.rdev)
                    }
                }
            }
        };

        Ok(Self { header, inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_comes_from_the_major_version() {
        assert_eq!(Layout::from_major(1), Some(Layout::V1));
        assert_eq!(Layout::from_major(2), Some(Layout::V2));
        assert_eq!(Layout::from_major(3), None);
    }

    #[test]
    fn a_file_with_a_fragment_stops_at_the_last_whole_block() {
        assert_eq!(block_count(32768, 15, 0, 40000), 1);
        assert_eq!(block_count(32768, 15, NO_FRAGMENT, 40000), 2);
    }
}
