use core::fmt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;

use deku::bitvec::BitVec;
use deku::DekuWrite;
use tracing::trace;

use crate::data::{Added, DataWriter};
use crate::entry::Entry;
use crate::inode::{BasicFile, InodeHeader};
use crate::kind::Kind;
use crate::metadata::MetadataWriter;
use crate::reader::WriteSeek;
use crate::{BackhandError, FilesystemCompressor, FilesystemReaderFile, SuperBlock};

/// File information for Node
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct NodeHeader {
    pub permissions: u16,
    pub uid: u16,
    pub gid: u16,
    pub mtime: u32,
}

impl NodeHeader {
    pub fn new(permissions: u16, uid: u16, gid: u16, mtime: u32) -> Self {
        Self {
            permissions,
            uid,
            gid,
            mtime,
        }
    }
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node<T> {
    pub fullpath: PathBuf,
    pub path: Rc<OsStr>,
    pub header: NodeHeader,
    pub inner: InnerNode<T>,
    pub(crate) inode_id: Option<u32>,
}

impl<T> Node<T> {
    pub(crate) fn new(
        fullpath: PathBuf,
        path: Rc<OsStr>,
        header: NodeHeader,
        inner: InnerNode<T>,
    ) -> Self {
        Self {
            fullpath,
            path,
            header,
            inner,
            inode_id: None,
        }
    }

    pub fn new_root(header: NodeHeader) -> Self {
        let fullpath = PathBuf::from("/");
        let path = Rc::from(fullpath.as_os_str());
        let inner = InnerNode::Dir(SquashfsDir::default());
        Self {
            fullpath,
            path,
            header,
            inner,
            inode_id: None,
        }
    }

    pub(crate) fn iter_inner_nodes<'a>(&'a self) -> impl Iterator<Item = &'a Node<T>> {
        let inner: Box<dyn Iterator<Item = &'a Node<T>>> = Box::new(
            self.inner_nodes()
                .into_iter()
                .flat_map(|nodes| nodes.values()),
        );
        [self].into_iter().chain(inner)
    }

    pub(crate) fn mut_inner_nodes(&mut self) -> Option<&mut HashMap<Rc<OsStr>, Node<T>>> {
        match &mut self.inner {
            InnerNode::Dir(node) => Some(&mut node.children),
            _ => None,
        }
    }

    pub fn inner_nodes(&self) -> Option<&HashMap<Rc<OsStr>, Node<T>>> {
        match &self.inner {
            InnerNode::Dir(node) => Some(&node.children),
            _ => None,
        }
    }

    fn have_children(&self) -> bool {
        self.inner_nodes().map_or(false, |nodes| !nodes.is_empty())
    }

    ///number of nodes in this tree
    pub(crate) fn inode_number(&self) -> usize {
        match &self.inner {
            InnerNode::Dir(node) => {
                let num_children: usize = node.children.values().map(Node::inode_number).sum();
                num_children + 1
            },
            _ => 1,
        }
    }

    pub(crate) fn calculate_inode(&mut self, inode_counter: &mut u32) {
        self.inode_id = Some(*inode_counter);
        *inode_counter += 1;

        self.mut_inner_nodes()
            .into_iter()
            .flat_map(|nodes| nodes.values_mut())
            .for_each(|child| child.calculate_inode(inode_counter));
    }
}

impl<'a> Node<SquashfsFileWriter<'a>> {
    pub(crate) fn write_data<W: WriteSeek>(
        &mut self,
        compressor: &FilesystemCompressor,
        block_size: u32,
        writer: &mut W,
        data_writer: &mut DataWriter,
    ) -> Result<(), BackhandError> {
        match &mut self.inner {
            InnerNode::File(file) => {
                let (filesize, added) = match &file {
                    SquashfsFileWriter::UserDefined(file) => {
                        data_writer.add_bytes(file.borrow_mut().as_mut(), writer)?
                    },
                    SquashfsFileWriter::SquashfsFile(file) => {
                        // if the source file and the destination files are both
                        // squashfs files and use the same compressor and block_size
                        // just copy the data, don't compress->decompress
                        if file.system.compressor == compressor.id
                            && file.system.compression_options == compressor.options
                            && file.system.block_size == block_size
                        {
                            data_writer.just_copy_it(file.raw_data_reader(), writer)?
                        } else {
                            let mut buf_read = Vec::with_capacity(file.system.block_size as usize);
                            let mut buf_decompress =
                                Vec::with_capacity(file.system.block_size as usize);
                            data_writer.add_bytes(
                                file.reader(&mut buf_read, &mut buf_decompress),
                                writer,
                            )?
                        }
                    },
                    SquashfsFileWriter::Consumed(_, _) => unreachable!(),
                };
                *file = SquashfsFileWriter::Consumed(filesize, added);
            },
            InnerNode::Dir(dir) => {
                dir.children.values_mut().try_for_each(|child| {
                    child.write_data(compressor, block_size, writer, data_writer)
                })?;
            },
            _ => (),
        }
        Ok(())
    }

    /// Create SquashFS file system from each node of Tree
    ///
    /// This works my recursively creating Inodes and Dirs for each node in the tree. This also
    /// keeps track of parent directories by calling this function on all nodes of a dir to get only
    /// the nodes, but going into the child dirs in the case that it contains a child dir.
    pub(crate) fn write_inode_dir(
        &self,
        inode_writer: &'_ mut MetadataWriter,
        dir_writer: &'_ mut MetadataWriter,
        parent_inode: u32,
        superblock: SuperBlock,
        kind: Kind,
    ) -> Result<(Option<Entry>, u64), BackhandError> {
        // If no children, just return since it doesn't have anything recursive/new
        // directories
        if !self.have_children() {
            return Ok((None, 0));
        }

        let dir = self.inner_nodes().unwrap();

        // ladies and gentlemen, we have a directory
        let mut write_entries = vec![];

        // store parent Inode, this is used for child Dirs, as they will need this to reference
        // back to this
        let this_inode = self.inode_id.unwrap();

        // tree has children, this is a Dir, get information of every child node
        for child in dir.values() {
            let (l_dir_entries, _) =
                child.write_inode_dir(inode_writer, dir_writer, this_inode, superblock, kind)?;
            if let Some(entry) = l_dir_entries {
                write_entries.push(entry);
            }
        }

        // write child inodes
        for node in dir.values().filter(|c| !c.have_children()) {
            let node_id = node.inode_id.unwrap();
            let entry = match &node.inner {
                InnerNode::Dir(_dir) => Entry::path(
                    &node.path,
                    node.header,
                    node_id,
                    this_inode,
                    inode_writer,
                    3, // Empty path
                    dir_writer.uncompressed_bytes.len() as u16,
                    dir_writer.metadata_start,
                    &superblock,
                    kind,
                ),
                InnerNode::File(SquashfsFileWriter::Consumed(filesize, added)) => Entry::file(
                    &node.path,
                    node.header,
                    node_id,
                    inode_writer,
                    *filesize,
                    added,
                    &superblock,
                    kind,
                ),
                InnerNode::File(_) => unreachable!(),
                InnerNode::Symlink(symlink) => Entry::symlink(
                    &node.path,
                    node.header,
                    symlink,
                    node_id,
                    inode_writer,
                    &superblock,
                    kind,
                ),
                InnerNode::CharacterDevice(char) => Entry::char(
                    &node.path,
                    node.header,
                    char,
                    node_id,
                    inode_writer,
                    &superblock,
                    kind,
                ),
                InnerNode::BlockDevice(block) => Entry::block_device(
                    &node.path,
                    node.header,
                    block,
                    node_id,
                    inode_writer,
                    &superblock,
                    kind,
                ),
            };
            write_entries.push(entry);
        }

        // write dir
        let block_index = dir_writer.metadata_start;
        let block_offset = dir_writer.uncompressed_bytes.len() as u16;
        trace!("WRITING DIR: {block_offset:#02x?}");
        let mut total_size = 3;
        for dir in Entry::into_dir(write_entries) {
            trace!("WRITING DIR: {dir:#02x?}");

            let mut bv = BitVec::new();
            dir.write(&mut bv, kind)?;
            let bytes = bv.as_raw_slice();
            dir_writer.write_all(bv.as_raw_slice())?;

            total_size += bytes.len() as u16;
        }

        //trace!("BEFORE: {:#02x?}", child);
        let entry = Entry::path(
            &self.path,
            self.header,
            this_inode,
            parent_inode,
            inode_writer,
            total_size,
            block_offset,
            block_index,
            &superblock,
            kind,
        );
        let root_inode = ((entry.start as u64) << 16) | ((entry.offset as u64) & 0xffff);

        trace!("[{:?}] entries: {:#02x?}", &self.path, &entry);
        Ok((Some(entry), root_inode))
    }
}

/// Filesystem node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerNode<T> {
    /// Either [`SquashfsFileReader`] or [`SquashfsFileWriter`]
    File(T),
    Symlink(SquashfsSymlink),
    Dir(SquashfsDir<T>),
    CharacterDevice(SquashfsCharacterDevice),
    BlockDevice(SquashfsBlockDevice),
}

/// Unread file for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsFileReader {
    pub basic: BasicFile,
}

/// Read file from other SquashfsFile or an user file
pub enum SquashfsFileWriter<'a> {
    UserDefined(RefCell<Box<dyn Read + 'a>>),
    SquashfsFile(FilesystemReaderFile<'a>),
    Consumed(usize, Added),
}

impl<'a> fmt::Debug for SquashfsFileWriter<'a> {
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
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsDir<T> {
    pub children: HashMap<Rc<OsStr>, Node<T>>,
}
impl<T> Default for SquashfsDir<T> {
    fn default() -> Self {
        Self {
            children: HashMap::default(),
        }
    }
}

/// Character Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsCharacterDevice {
    pub device_number: u32,
}

/// Block Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsBlockDevice {
    pub device_number: u32,
}
