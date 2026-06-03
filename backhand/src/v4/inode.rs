//! Index Node for file or directory

use core::fmt;
use no_std_io2::io::Write;
use std::io::Cursor;

use deku::prelude::*;

use crate::kinds::Kind;
use crate::v4::data::DataSize;
use crate::v4::dir::DirectoryIndex;
use crate::v4::entry::Entry;
use crate::v4::metadata::MetadataWriter;
use crate::v4::squashfs::SuperBlock;

/// On-disk inode entry
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(ctx = "bytes_used: u64, block_size: u32, block_log: u16, type_endian: deku::ctx::Endian")]
#[deku(endian = "type_endian")]
pub struct Inode {
    /// Inode type identifier
    pub id: InodeId,
    /// Common inode header fields
    pub header: InodeHeader,
    /// Type-specific inode data
    #[deku(ctx = "*id, bytes_used, block_size, block_log")]
    pub inner: InodeInner,
}

impl Inode {
    /// Create a new inode
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
            ),
        )
        .unwrap();
        let start = m_writer.metadata_start;
        let offset = m_writer.uncompressed_bytes.len() as u16;
        m_writer.write_all(inode_bytes.get_ref()).unwrap();

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

/// Inode type identifier
#[derive(Debug, DekuRead, DekuWrite, DekuSize, Clone, Copy, PartialEq, Eq)]
#[deku(id_type = "u16")]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
#[repr(u16)]
#[rustfmt::skip]
pub enum InodeId {
    /// Basic directory (type 1)
    BasicDirectory       = 1,
    /// Basic regular file (type 2)
    BasicFile            = 2,
    /// Basic symbolic link (type 3)
    BasicSymlink         = 3,
    /// Basic block device (type 4)
    BasicBlockDevice     = 4,
    /// Basic character device (type 5)
    BasicCharacterDevice = 5,
    /// Basic named pipe / FIFO (type 6)
    BasicNamedPipe       = 6, // aka FIFO
    /// Basic unix domain socket (type 7)
    BasicSocket          = 7,
    /// Extended directory with index (type 8)
    ExtendedDirectory    = 8,
    /// Extended file with extra fields (type 9)
    ExtendedFile         = 9,
    // TODO:
    // Extended Symlink = 10
    // Extended Block Device = 11
    // Extended Character Device = 12
    // Extended Named Pipe (FIFO) = 13
    // Extended Socked = 14
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

/// Type-specific inode data
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, id: InodeId, bytes_used: u64, block_size: u32, block_log: u16"
)]
#[deku(endian = "endian")]
#[deku(id = "id")]
pub enum InodeInner {
    /// Basic directory data
    #[deku(id = "InodeId::BasicDirectory")]
    BasicDirectory(BasicDirectory),

    /// Basic file data
    #[deku(id = "InodeId::BasicFile")]
    BasicFile(#[deku(ctx = "block_size, block_log")] BasicFile),

    /// Basic symlink data
    #[deku(id = "InodeId::BasicSymlink")]
    BasicSymlink(BasicSymlink),

    /// Basic block device data
    #[deku(id = "InodeId::BasicBlockDevice")]
    BasicBlockDevice(BasicDeviceSpecialFile),

    /// Basic character device data
    #[deku(id = "InodeId::BasicCharacterDevice")]
    BasicCharacterDevice(BasicDeviceSpecialFile),

    /// Basic named pipe data
    #[deku(id = "InodeId::BasicNamedPipe")]
    BasicNamedPipe(IPCNode),

    /// Basic socket data
    #[deku(id = "InodeId::BasicSocket")]
    BasicSocket(IPCNode),

    /// Extended directory data
    #[deku(id = "InodeId::ExtendedDirectory")]
    ExtendedDirectory(ExtendedDirectory),

    /// Extended file data
    #[deku(id = "InodeId::ExtendedFile")]
    ExtendedFile(#[deku(ctx = "bytes_used, block_size, block_log")] ExtendedFile),
}

/// Common inode header shared by all inode types
#[derive(Debug, DekuRead, DekuWrite, DekuSize, Clone, Copy, PartialEq, Eq, Default)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct InodeHeader {
    /// Unix permissions
    pub permissions: u16,
    /// index into id table
    pub uid: u16,
    /// index into id table
    pub gid: u16,
    /// Modification time (seconds since epoch)
    pub mtime: u32,
    /// Unique inode number
    pub inode_number: u32,
}

/// Basic directory inode data
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicDirectory {
    /// Index into the directory table
    pub block_index: u32,
    /// Number of hard links
    pub link_count: u32,
    /// Size of directory listing in the directory table
    pub file_size: u16,
    /// Offset into the uncompressed directory block
    pub block_offset: u16,
    /// Inode number of parent directory
    pub parent_inode: u32,
}

/// Extended directory inode with directory index
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct ExtendedDirectory {
    /// Number of hard links
    pub link_count: u32,
    /// Size of directory listing
    pub file_size: u32,
    /// Index into the directory table
    pub block_index: u32,
    /// Inode number of parent directory
    pub parent_inode: u32,
    /// Number of directory index entries
    pub index_count: u16,
    /// Offset into the uncompressed directory block
    pub block_offset: u16,
    /// Extended attribute index
    pub xattr_index: u32,
    /// Directory index entries for faster lookup
    #[deku(count = "*index_count")]
    pub dir_index: Vec<DirectoryIndex>,
}

#[allow(non_upper_case_globals)]
const TiB2: u128 = 0x200_0000_0000;

/// Basic file inode data
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian, block_size: u32, block_log: u16")]
pub struct BasicFile {
    /// Offset from start of data area to first data block
    pub blocks_start: u32,
    /// Index into fragment table (0xFFFFFFFF if no fragment)
    pub frag_index: u32,
    /// Offset into the fragment block
    pub block_offset: u32,
    /// Uncompressed file size
    #[deku(assert = "((*file_size as u128) < TiB2)")]
    pub file_size: u32,
    /// Sizes of each data block
    #[deku(count = "block_count(block_size, block_log, *frag_index, *file_size as u64)")]
    pub block_sizes: Vec<DataSize>,
}

/// Extended file inode with 64-bit sizes and extra fields
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    endian = "endian",
    ctx = "endian: deku::ctx::Endian, bytes_used: u64, block_size: u32, block_log: u16"
)]
pub struct ExtendedFile {
    /// Offset from start of data area to first data block
    pub blocks_start: u64,
    /// Uncompressed file size
    #[deku(assert = "((*file_size as u128) < TiB2)")]
    pub file_size: u64,
    /// Sparse file byte count
    pub sparse: u64,
    /// Number of hard links
    pub link_count: u32,
    /// Index into fragment table (0xFFFFFFFF if no fragment)
    pub frag_index: u32,
    /// Offset into the fragment block
    pub block_offset: u32,
    /// Extended attribute index
    pub xattr_index: u32,
    /// Sizes of each data block
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

/// Basic symbolic link inode data
#[derive(DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicSymlink {
    /// Number of hard links
    pub link_count: u32,
    /// Length of target path in bytes
    #[deku(assert = "*target_size < 256")]
    pub target_size: u32,
    /// Target path bytes
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
    /// Get the symlink target as a string
    pub fn target(&self) -> String {
        core::str::from_utf8(&self.target_path).unwrap().to_string()
    }
}

/// Block or character device special file inode data
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct BasicDeviceSpecialFile {
    /// Number of hard links
    pub link_count: u32,
    /// Device major/minor number
    pub device_number: u32,
}

/// IPC node (named pipe or socket) inode data
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct IPCNode {
    /// Number of hard links
    pub link_count: u32,
}
