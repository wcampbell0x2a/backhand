//! Storage of directories with references to inodes
//!
//! For each directory inode, the directory table stores a linear list of all entries,
//! with references back to the inodes that describe those entries.

use core::fmt;
use std::ffi::OsStr;
use std::os::unix::prelude::OsStrExt;
use std::path::{Component, Path};

use deku::prelude::*;

use crate::v3::inode::InodeId;
use crate::BackhandError;

/// `squashfs_dir_header`
#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(ctx = "type_endian: deku::ctx::Endian, order: deku::ctx::Order")]
#[deku(bit_order = "order")]
#[deku(endian = "type_endian")]
pub struct Dir {
    /// Number of entries following the header.
    ///
    /// A header must be followed by AT MOST 256 entries. If there are more entries, a new header MUST be emitted.
    #[deku(assert = "*count <= 256", bytes = "1")]
    pub(crate) count: u32,
    /// The location of the metadata block in the inode table where the inodes are stored.
    /// This is relative to the inode table start from the super block.
    pub(crate) start: u32,
    /// An arbitrary inode number.
    /// The entries that follow store their inode number as a difference to this.
    pub(crate) inode_num: i32,
    //#[deku(count = "*count + 1")]
    #[deku(count = "*count + 1")]
    pub(crate) dir_entries: Vec<DirEntry>,
}

impl Dir {
    pub fn new(lowest_inode: u32) -> Self {
        Self {
            count: u32::default(),
            start: u32::default(),
            inode_num: lowest_inode as i32,
            dir_entries: vec![],
        }
    }

    pub fn push(&mut self, entry: DirEntry) {
        self.dir_entries.push(entry);
        self.count = (self.dir_entries.len() - 1) as u32;
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, Copy, PartialEq, Eq)]
#[deku(type = "u8", bits = "3")]
#[deku(endian = "endian", bit_order = "order", ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order")]
#[rustfmt::skip]
pub enum DirInodeId {
    BasicDirectory       = 1,
    BasicFile            = 2,
    BasicSymlink         = 3,
    BasicBlockDevice     = 4,
    BasicCharacterDevice = 5,
    ExtendedDirectory    = 8,
    ExtendedFile         = 9,
    // TODO:
    // Extended Symlink = 10
    // Extended Block Device = 11
    // Extended Character Device = 12
    // Extended Named Pipe (FIFO) = 13
    // Extended Socked = 14
}

// TODO: derive our own Debug, with name()
#[derive(DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    endian = "endian",
    bit_order = "order",
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order"
)]
pub struct DirEntry {
    /// An offset into the uncompressed inode metadata block.
    #[deku(bits = "13")]
    pub(crate) offset: u16,
    /// The inode type. For extended inodes, the basic type is stored here instead.
    pub(crate) t: DirInodeId,
    /// One less than the size of the entry name.
    #[deku(bytes = "1")]
    pub(crate) name_size: u16,
    /// The difference of this inode’s number to the reference stored in the header.
    pub(crate) inode_offset: i16,
    // TODO: CString
    /// The file name of the entry without a trailing null byte. Has name size + 1 bytes.
    #[deku(count = "*name_size + 1")]
    pub(crate) name: Vec<u8>,
}

impl fmt::Debug for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirEntry")
            .field("offset", &self.offset)
            .field("t", &self.t)
            .field("name_size", &self.name_size)
            .field("inode_offset", &self.inode_offset)
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

#[derive(DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct DirectoryIndex {
    /// This stores a byte offset from the first directory header to the current header,
    /// as if the uncompressed directory metadata blocks were laid out in memory consecutively.
    pub(crate) index: u32,
    /// Start offset of a directory table metadata block, relative to the directory table start.
    pub(crate) start: u32,
    #[deku(assert = "*name_size < 100")]
    pub(crate) name_size: u32,
    #[deku(count = "*name_size + 1")]
    pub(crate) name: Vec<u8>,
}

impl fmt::Debug for DirectoryIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirectoryIndex")
            .field("index", &self.index)
            .field("start", &self.start)
            .field("name_size", &self.name_size)
            .field("name", &self.name())
            .finish()
    }
}

impl DirectoryIndex {
    pub fn name(&self) -> String {
        std::str::from_utf8(&self.name).unwrap().to_string()
    }
}
