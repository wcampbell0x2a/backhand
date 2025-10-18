use core::fmt;
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::super::data::Added;
use super::super::data::DataSize;
use super::super::id::Id;
use super::super::inode::{BasicFile, ExtendedFile, InodeHeader};
use super::normalize_squashfs_path;
use super::reader::FilesystemReaderFile;
use crate::error::BackhandError;

/// File information for Node
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct NodeHeader {
    pub permissions: u16,
    /// actual value
    pub uid: u32,
    /// actual value
    pub gid: u32,
    pub mtime: u32,
}

impl NodeHeader {
    pub fn new(permissions: u16, uid: u32, gid: u32, mtime: u32) -> Self {
        Self { permissions, uid, gid, mtime }
    }
}

impl NodeHeader {
    pub fn from_inode(inode_header: InodeHeader, id_table: &[Id]) -> Result<Self, BackhandError> {
        let uid = if (inode_header.uid as usize) < id_table.len() {
            id_table[inode_header.uid as usize].num
        } else {
            // Fallback to root uid for out-of-bounds values
            0
        };
        // Handle case where gid might be out of bounds (v3 quirk)
        let gid = if (inode_header.gid as usize) < id_table.len() {
            id_table[inode_header.gid as usize].num
        } else {
            // Fallback to root gid for out-of-bounds values
            0
        };
        Ok(Self { permissions: inode_header.permissions, uid, gid, mtime: inode_header.mtime })
    }
}

/// Filesystem Node
#[derive(Clone, Debug)]
pub struct Node<T> {
    pub fullpath: PathBuf,
    pub header: NodeHeader,
    pub inner: InnerNode<T>,
}

impl<T> PartialEq for Node<T> {
    fn eq(&self, other: &Self) -> bool {
        self.fullpath.eq(&other.fullpath)
    }
}
impl<T> Eq for Node<T> {}
impl<T> PartialOrd for Node<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Node<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.fullpath.cmp(&other.fullpath)
    }
}

impl<T> Node<T> {
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
    Symlink(SquashfsSymlink),
    Dir(SquashfsDir),
    CharacterDevice(SquashfsCharacterDevice),
    BlockDevice(SquashfsBlockDevice),
    NamedPipe,
    Socket,
}

/// Unread file for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SquashfsFileReader {
    Basic(BasicFile),
    Extended(ExtendedFile),
}

impl SquashfsFileReader {
    pub fn file_len(&self) -> usize {
        match self {
            SquashfsFileReader::Basic(basic) => basic.file_size as usize,
            SquashfsFileReader::Extended(extended) => extended.file_size as usize,
        }
    }

    pub fn frag_index(&self) -> usize {
        match self {
            SquashfsFileReader::Basic(basic) => basic.frag as usize,
            SquashfsFileReader::Extended(extended) => extended.frag_index as usize,
        }
    }

    pub fn block_sizes(&self) -> &[DataSize] {
        match self {
            SquashfsFileReader::Basic(basic) => &basic.block_sizes,
            SquashfsFileReader::Extended(extended) => &extended.block_sizes,
        }
    }

    pub fn blocks_start(&self) -> u64 {
        match self {
            SquashfsFileReader::Basic(basic) => basic.blocks_start,
            SquashfsFileReader::Extended(extended) => extended.blocks_start,
        }
    }

    pub fn block_offset(&self) -> u32 {
        match self {
            SquashfsFileReader::Basic(basic) => basic.block_offset,
            SquashfsFileReader::Extended(extended) => extended.block_offset,
        }
    }
}

/// Read file from other SquashfsFile or an user file
pub enum SquashfsFileWriter<'a, 'b, 'c> {
    UserDefined(Arc<Mutex<dyn Read + 'c>>),
    SquashfsFile(FilesystemReaderFile<'a, 'b>),
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
    pub link: PathBuf,
}

/// Directory for filesystem
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct SquashfsDir {}

/// Character Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SquashfsCharacterDevice {
    pub device_number: u32,
}

/// Block Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SquashfsBlockDevice {
    pub device_number: u32,
}

#[derive(Debug, Clone)]
pub struct Nodes<T> {
    pub nodes: Vec<Node<T>>,
}

impl<T> Nodes<T> {
    pub fn new_root(header: NodeHeader) -> Self {
        Self { nodes: vec![Node::new_root(header)] }
    }

    pub fn root(&self) -> &Node<T> {
        &self.nodes[0]
    }

    pub fn root_mut(&mut self) -> &mut Node<T> {
        &mut self.nodes[0]
    }

    pub fn node_mut<S: AsRef<Path>>(&mut self, path: S) -> Option<&mut Node<T>> {
        //the search path root prefix is optional, so remove it if present to
        //not affect the search
        let find_path = normalize_squashfs_path(path.as_ref()).ok()?;
        self.nodes
            .binary_search_by(|node| node.fullpath.cmp(&find_path))
            .ok()
            .map(|found| &mut self.nodes[found])
    }

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

    pub fn node(&self, node_index: NonZeroUsize) -> Option<&Node<T>> {
        self.nodes.get(node_index.get() - 1)
    }

    pub fn children_of(
        &self,
        node_index: NonZeroUsize,
    ) -> impl Iterator<Item = (NonZeroUsize, &Node<T>)> {
        self.inner_children_of(node_index.get() - 1).unwrap_or(&[]).iter().enumerate().map(
            move |(index, node)| (NonZeroUsize::new(node_index.get() + index + 1).unwrap(), node),
        )
    }
}
