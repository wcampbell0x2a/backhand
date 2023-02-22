use core::fmt;
use std::cell::RefCell;
use std::io::Read;
use std::path::PathBuf;

use crate::inode::{BasicFile, InodeHeader};
use crate::reader::ReadSeek;
use crate::FilesystemReaderFile;

/// File information for Node
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct NodeHeader {
    pub permissions: u16,
    pub uid: u16,
    pub gid: u16,
    pub mtime: u32,
}

impl From<InodeHeader> for NodeHeader {
    fn from(inode_header: InodeHeader) -> Self {
        Self {
            permissions: inode_header.permissions,
            uid: inode_header.uid,
            gid: inode_header.gid,
            mtime: inode_header.mtime,
        }
    }
}

/// Filesystem Node
#[derive(Debug, PartialEq, Eq)]
pub struct Node<T> {
    pub path: PathBuf,
    pub inner: InnerNode<T>,
}

impl<T> Node<T> {
    pub fn new(path: PathBuf, inner: InnerNode<T>) -> Self {
        Self { path, inner }
    }
}

/// Filesystem node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerNode<T> {
    /// Either [`SquashfsFileReader`] or [`SquashfsFileWriter`]
    File(T),
    Symlink(SquashfsSymlink),
    Dir(SquashfsDir),
    CharacterDevice(SquashfsCharacterDevice),
    BlockDevice(SquashfsBlockDevice),
}

/// Unread file for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsFileReader {
    pub header: NodeHeader,
    pub basic: BasicFile,
}

/// Read file
pub struct SquashfsFileWriter<'a, R: ReadSeek> {
    pub header: NodeHeader,
    pub reader: SquashfsFileSource<'a, R>,
}

impl<'a, R: ReadSeek> fmt::Debug for SquashfsFileWriter<'a, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileWriter")
            .field("header", &self.header)
            .finish()
    }
}
pub enum SquashfsFileSource<'a, R: ReadSeek> {
    UserDefined(RefCell<Box<dyn Read + 'a>>),
    SquashfsFile(FilesystemReaderFile<'a, R>),
}

/// Symlink for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsSymlink {
    pub header: NodeHeader,
    pub link: PathBuf,
}

/// Directory for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsDir {
    pub header: NodeHeader,
}

/// Character Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsCharacterDevice {
    pub header: NodeHeader,
    pub device_number: u32,
}

/// Block Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsBlockDevice {
    pub header: NodeHeader,
    pub device_number: u32,
}
