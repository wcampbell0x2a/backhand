use core::fmt;
use core::num::NonZeroUsize;
use no_std_io2::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::error::BackhandError;
use crate::v4::data::Added;
use crate::v4::filesystem::normalize_squashfs_path;
use crate::v4::inode::{BasicFile, ExtendedFile, InodeHeader};
use crate::{DataSize, FilesystemReaderFile, Id};

/// File information for Node
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct NodeHeader {
    /// Unix permissions mode
    pub permissions: u16,
    /// actual value
    pub uid: u32,
    /// actual value
    pub gid: u32,
    /// Modification time (seconds since epoch)
    pub mtime: u32,
}

impl NodeHeader {
    /// Create a new node header
    pub fn new(permissions: u16, uid: u32, gid: u32, mtime: u32) -> Self {
        Self { permissions, uid, gid, mtime }
    }
}

impl NodeHeader {
    /// Create from an inode header, resolving uid/gid from the ID table
    pub fn from_inode(inode_header: InodeHeader, id_table: &[Id]) -> Result<Self, BackhandError> {
        let uid = id_table.get(inode_header.uid as usize).ok_or(BackhandError::InvalidIdTable)?;
        let gid = id_table.get(inode_header.gid as usize).ok_or(BackhandError::InvalidIdTable)?;
        Ok(Self {
            permissions: inode_header.permissions,
            uid: uid.num,
            gid: gid.num,
            mtime: inode_header.mtime,
        })
    }
}

/// Filesystem Node
#[derive(Clone, Debug)]
pub struct Node<T> {
    /// Full path from root
    pub fullpath: PathBuf,
    /// File metadata
    pub header: NodeHeader,
    /// Node type and type-specific data
    pub inner: InnerNode<T>,
}

impl<T> PartialEq for Node<T> {
    fn eq(&self, other: &Self) -> bool {
        self.fullpath.eq(&other.fullpath)
    }
}
impl<T> Eq for Node<T> {}
impl<T> PartialOrd for Node<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Node<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.fullpath.cmp(&other.fullpath)
    }
}

impl<T> Node<T> {
    pub(crate) fn new(fullpath: PathBuf, header: NodeHeader, inner: InnerNode<T>) -> Self {
        Self { fullpath, header, inner }
    }

    /// Create a root directory node
    pub fn new_root(header: NodeHeader) -> Self {
        let fullpath = PathBuf::from("/");
        let inner = InnerNode::Dir(SquashfsDir::default());
        Self { fullpath, header, inner }
    }
}

/// Filesystem node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerNode<T> {
    /// Either [`SquashfsFileReader`] or [`SquashfsFileWriter`]
    File(T),
    /// Symbolic link
    Symlink(SquashfsSymlink),
    /// Directory
    Dir(SquashfsDir),
    /// Character device
    CharacterDevice(SquashfsCharacterDevice),
    /// Block device
    BlockDevice(SquashfsBlockDevice),
    /// Named pipe (FIFO)
    NamedPipe,
    /// Unix domain socket
    Socket,
}

/// Unread file for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SquashfsFileReader {
    /// Basic file inode
    Basic(BasicFile),
    /// Extended file inode
    Extended(ExtendedFile),
}

impl SquashfsFileReader {
    /// Uncompressed file length in bytes
    pub fn file_len(&self) -> usize {
        match self {
            SquashfsFileReader::Basic(basic) => basic.file_size as usize,
            SquashfsFileReader::Extended(extended) => extended.file_size as usize,
        }
    }

    /// Fragment table index (0xFFFFFFFF if no fragment)
    pub fn frag_index(&self) -> usize {
        match self {
            SquashfsFileReader::Basic(basic) => basic.frag_index as usize,
            SquashfsFileReader::Extended(extended) => extended.frag_index as usize,
        }
    }

    /// Slice of data block sizes
    pub fn block_sizes(&self) -> &[DataSize] {
        match self {
            SquashfsFileReader::Basic(basic) => &basic.block_sizes,
            SquashfsFileReader::Extended(extended) => &extended.block_sizes,
        }
    }

    /// Offset to the start of data blocks
    pub fn blocks_start(&self) -> u64 {
        match self {
            SquashfsFileReader::Basic(basic) => basic.blocks_start as u64,
            SquashfsFileReader::Extended(extended) => extended.blocks_start,
        }
    }

    /// Offset into the fragment block
    pub fn block_offset(&self) -> u32 {
        match self {
            SquashfsFileReader::Basic(basic) => basic.block_offset,
            SquashfsFileReader::Extended(extended) => extended.block_offset,
        }
    }
}

/// Read file from other SquashfsFile or an user file
pub enum SquashfsFileWriter<'a, 'b, 'c> {
    /// User-provided reader
    UserDefined(Arc<Mutex<dyn Read + 'c>>),
    /// File from an existing SquashFS image
    SquashfsFile(FilesystemReaderFile<'a, 'b>),
    /// Already written (file size, location)
    Consumed(usize, Added),
}

impl fmt::Debug for SquashfsFileWriter<'_, '_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileWriter").finish()
    }
}

/// Symlink for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsSymlink {
    /// Symlink target path
    pub link: PathBuf,
}

/// Directory for filesystem
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct SquashfsDir {}

/// Character Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SquashfsCharacterDevice {
    /// Device major/minor number
    pub device_number: u32,
}

/// Block Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SquashfsBlockDevice {
    /// Device major/minor number
    pub device_number: u32,
}

/// Sorted collection of filesystem nodes
#[derive(Debug, Clone)]
pub struct Nodes<T> {
    /// All nodes sorted by path
    pub nodes: Vec<Node<T>>,
}

impl<T> Nodes<T> {
    /// Create with only a root node
    pub fn new_root(header: NodeHeader) -> Self {
        Self { nodes: vec![Node::new_root(header)] }
    }

    /// Get the root node
    pub fn root(&self) -> &Node<T> {
        &self.nodes[0]
    }

    /// Get the root node mutably
    pub fn root_mut(&mut self) -> &mut Node<T> {
        &mut self.nodes[0]
    }

    /// Find a node by path, returning a mutable reference
    pub fn node_mut<S: AsRef<Path>>(&mut self, path: S) -> Option<&mut Node<T>> {
        //the search path root prefix is optional, so remove it if present to
        //not affect the search
        let find_path = normalize_squashfs_path(path.as_ref()).ok()?;
        self.nodes
            .binary_search_by(|node| node.fullpath.cmp(&find_path))
            .ok()
            .map(|found| &mut self.nodes[found])
    }

    /// Insert a node, checking parent exists and is a directory
    pub fn insert(&mut self, node: Node<T>) -> Result<(), BackhandError> {
        let path = &node.fullpath;
        let parent = node.fullpath.parent().ok_or(BackhandError::InvalidFilePath)?;

        //check if the parent exists and is a dir
        let parent = self.node_mut(parent).ok_or(BackhandError::InvalidFilePath)?;
        match &parent.inner {
            InnerNode::Dir(_) => {}
            _ => return Err(BackhandError::InvalidFilePath),
        }

        match self.nodes.binary_search_by(|node| node.fullpath.as_path().cmp(path)) {
            //file with this fullpath already exists
            Ok(_index) => Err(BackhandError::DuplicatedFileName),
            //file don't exists, insert it at this location
            Err(index) => {
                self.nodes.insert(index, node);
                Ok(())
            }
        }
    }

    fn inner_children_of(&self, node_index: usize) -> Option<&[Node<T>]> {
        let parent = &self.nodes[node_index];
        let children_start = node_index + 1;
        let unbounded_children = self.nodes.get(children_start..)?;
        let children_len = unbounded_children
            .iter()
            .enumerate()
            .find(|(_, node)| !node.fullpath.starts_with(&parent.fullpath))
            .map(|(index, _)| index)
            .unwrap_or(unbounded_children.len());
        Some(&unbounded_children[..children_len])
    }

    /// Get a node by 1-based index
    pub fn node(&self, node_index: NonZeroUsize) -> Option<&Node<T>> {
        self.nodes.get(node_index.get() - 1)
    }

    /// Iterate over all children (recursive) of the node at the given index
    pub fn children_of(
        &self,
        node_index: NonZeroUsize,
    ) -> impl Iterator<Item = (NonZeroUsize, &Node<T>)> {
        self.inner_children_of(node_index.get() - 1).unwrap_or(&[]).iter().enumerate().map(
            move |(index, node)| (NonZeroUsize::new(node_index.get() + index + 1).unwrap(), node),
        )
    }
}
