//! Filesystem tree nodes for SquashFS v1 and v2

use std::path::PathBuf;

use crate::error::BackhandError;
use crate::v4::id::Id;
use crate::v12::data::DataSize;
use crate::v12::inode::{File, GidRef, InodeHeader};

/// Owner, group, permissions and time of one node, with the ids resolved
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct NodeHeader {
    pub permissions: u16,
    pub uid: u32,
    pub gid: u32,
    pub mtime: u32,
}

impl NodeHeader {
    pub fn new(permissions: u16, uid: u32, gid: u32, mtime: u32) -> Self {
        Self { permissions, uid, gid, mtime }
    }

    /// Resolve an inode's id indexes against the image's tables
    ///
    /// An index past the end of a table falls back to root, as the v3 reader
    /// does: vendor images are often truncated, and refusing to open one over a
    /// single bad id would lose the rest of the tree.
    pub fn from_inode(
        header: InodeHeader,
        uid_table: &[Id],
        guid_table: &[Id],
    ) -> Result<Self, BackhandError> {
        let uid = uid_table.get(header.uid_index as usize).map_or(0, |id| id.num);
        let gid = match header.gid {
            // The sentinel means the group is whatever the owner is.
            GidRef::SameAsUid => uid,
            GidRef::Index(index) => guid_table.get(index as usize).map_or(0, |id| id.num),
        };

        Ok(Self { permissions: header.permissions, uid, gid, mtime: header.mtime })
    }
}

/// One node of the filesystem tree
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
    pub fn new_root(header: NodeHeader) -> Self {
        Self { fullpath: PathBuf::from("/"), header, inner: InnerNode::Dir(SquashfsDir::default()) }
    }
}

/// What kind of node this is, and what it holds
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerNode<T> {
    File(T),
    Symlink(SquashfsSymlink),
    Dir(SquashfsDir),
    CharacterDevice(SquashfsCharacterDevice),
    BlockDevice(SquashfsBlockDevice),
    NamedPipe,
    Socket,
}

/// A file that has not been read yet
///
/// v1 and v2 have no extended file inode, so unlike v3 there is only one form.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsFileReader(pub File);

impl SquashfsFileReader {
    pub fn file_len(&self) -> usize {
        self.0.file_size as usize
    }

    pub fn frag_index(&self) -> usize {
        self.0.fragment as usize
    }

    pub fn block_sizes(&self) -> &[DataSize] {
        &self.0.block_sizes
    }

    pub fn blocks_start(&self) -> u64 {
        u64::from(self.0.start_block)
    }

    pub fn block_offset(&self) -> u32 {
        self.0.offset
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SquashfsSymlink {
    pub link: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SquashfsDir {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquashfsCharacterDevice {
    pub device_number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquashfsBlockDevice {
    pub device_number: u32,
}

/// Every node of one image, kept sorted by path
#[derive(Debug, Clone)]
pub struct Nodes<T> {
    pub nodes: Vec<Node<T>>,
}

impl<T> Nodes<T> {
    pub fn new_root(header: NodeHeader) -> Self {
        Self { nodes: vec![Node::new_root(header)] }
    }

    pub fn push(&mut self, node: Node<T>) {
        self.nodes.push(node);
    }

    pub fn sort(&mut self) {
        self.nodes.sort_unstable_by(|a, b| a.fullpath.cmp(&b.fullpath));
    }

    pub fn root(&self) -> &Node<T> {
        &self.nodes[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(values: &[u32]) -> Vec<Id> {
        values.iter().copied().map(Id::new).collect()
    }

    #[test]
    fn ids_resolve_through_their_tables() {
        let header =
            InodeHeader { permissions: 0o644, uid_index: 1, gid: GidRef::Index(0), mtime: 42 };
        let node = NodeHeader::from_inode(header, &table(&[0, 1000]), &table(&[50])).unwrap();

        assert_eq!(node.uid, 1000);
        assert_eq!(node.gid, 50);
        assert_eq!(node.permissions, 0o644);
        assert_eq!(node.mtime, 42);
    }

    #[test]
    fn the_group_sentinel_takes_the_owner_id() {
        let header =
            InodeHeader { permissions: 0o755, uid_index: 1, gid: GidRef::SameAsUid, mtime: 0 };
        let node = NodeHeader::from_inode(header, &table(&[0, 1000]), &[]).unwrap();

        assert_eq!(node.uid, 1000);
        assert_eq!(node.gid, 1000);
    }

    #[test]
    fn an_index_past_the_table_falls_back_to_root() {
        let header =
            InodeHeader { permissions: 0o600, uid_index: 9, gid: GidRef::Index(9), mtime: 0 };
        let node = NodeHeader::from_inode(header, &table(&[7]), &table(&[8])).unwrap();

        assert_eq!(node.uid, 0);
        assert_eq!(node.gid, 0);
    }
}
