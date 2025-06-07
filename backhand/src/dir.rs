//! Storage of directories with references to inodes
//!
//! For each directory inode, the directory table stores a linear list of all entries,
//! with references back to the inodes that describe those entries.

use std::ffi::OsStr;
use std::path::{Component, Path};

use deku::prelude::*;

use crate::BackhandError;
use crate::inode::InodeId;
use crate::unix_string::OsStrExt;

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(ctx = "type_endian: deku::ctx::Endian")]
#[deku(endian = "type_endian")]
pub struct Dir {
    /// Number of entries following the header.
    ///
    /// A header must be followed by AT MOST 256 entries. If there are more entries, a new header MUST be emitted.
    #[deku(assert = "*count <= 256")]
    pub(crate) count: u32,
    /// The location of the metadata block in the inode table where the inodes are stored.
    /// This is relative to the inode table start from the super block.
    pub(crate) start: u32,
    /// An arbitrary inode number.
    /// The entries that follow store their inode number as a difference to this.
    pub(crate) inode_num: u32,
    #[deku(count = "*count + 1")]
    pub(crate) dir_entries: Vec<DirEntry>,
}

impl Dir {
    pub fn new(lowest_inode: u32) -> Self {
        Self {
            count: u32::default(),
            start: u32::default(),
            inode_num: lowest_inode,
            dir_entries: vec![],
        }
    }

    pub fn push(&mut self, entry: DirEntry) {
        self.dir_entries.push(entry);
        self.count = (self.dir_entries.len() - 1) as u32;
    }
}

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct DirEntry {
    /// An offset into the uncompressed inode metadata block.
    pub(crate) offset: u16,
    /// The difference of this inode’s number to the reference stored in the header.
    pub(crate) inode_offset: i16,
    /// The inode type. For extended inodes, the basic type is stored here instead.
    pub(crate) t: InodeId,
    /// One less than the size of the entry name.
    pub(crate) name_size: u16,
    // TODO: CString
    /// The file name of the entry without a trailing null byte. Has name size + 1 bytes.
    #[deku(count = "*name_size + 1")]
    pub(crate) name: Vec<u8>,
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

#[derive(Debug, DekuRead, DekuWrite, Clone, PartialEq, Eq)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct DirectoryIndex {
    /// This stores a byte offset from the first directory header to the current header,
    /// as if the uncompressed directory metadata blocks were laid out in memory consecutively.
    pub(crate) index: u32,
    /// Start offset of a directory table metadata block, relative to the directory table start.
    pub(crate) start: u32,
    #[deku(assert = "*name_size < 256")]
    pub(crate) name_size: u32,
    #[deku(count = "*name_size + 1")]
    pub(crate) name: Vec<u8>,
}

impl DirectoryIndex {
    pub fn name(&self) -> String {
        std::str::from_utf8(&self.name).unwrap().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_invalid_dir_entry() {
        // just root
        let dir = DirEntry {
            offset: 0x300,
            inode_offset: 0x0,
            t: InodeId::BasicDirectory,
            name_size: 0x1,
            name: b"/".to_vec(),
        };
        assert_eq!(Path::new("/"), dir.name().unwrap());

        // InvalidFilePath
        let dir = DirEntry {
            offset: 0x300,
            inode_offset: 0x0,
            t: InodeId::BasicDirectory,
            name_size: 0x1,
            name: b"/nice/".to_vec(),
        };
        assert!(dir.name().is_err());
    }
}
