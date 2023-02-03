use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{Seek, Write};
use std::path::Component::*;
use std::path::{Path, PathBuf};

use deku::DekuContainerWrite;
use tracing::trace;

use crate::data::{Added, DataWriter};
use crate::entry::Entry;
use crate::error::SquashfsError;
use crate::filesystem::{
    FilesystemWriter, InnerNode, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir,
    SquashfsFileWriter, SquashfsSymlink,
};
use crate::metadata::MetadataWriter;
use crate::FilesystemHeader;

fn normalized_components(path: &Path) -> Vec<&OsStr> {
    let mut v = Vec::new();

    for c in path.components() {
        match c {
            Prefix(p) => v.push(p.as_os_str()),
            RootDir => v.clear(),
            CurDir => {},
            ParentDir => {
                v.pop();
            },
            Normal(n) => v.push(n),
        }
    }

    v
}

#[derive(Debug)]
pub(crate) struct TreeNode<'a, 'b> {
    pub fullpath: PathBuf,
    inode_id: u32,
    pub inner: InnerTreeNode<'a, 'b>,
}

#[derive(Debug)]
pub(crate) enum InnerTreeNode<'a, 'b> {
    FilePhase1(&'b SquashfsFileWriter<'a>),
    FilePhase2(usize, Added, &'b FilesystemHeader),
    Symlink(&'b SquashfsSymlink),
    Dir(&'b SquashfsDir, BTreeMap<OsString, TreeNode<'a, 'b>>),
    CharacterDevice(&'b SquashfsCharacterDevice),
    BlockDevice(&'b SquashfsBlockDevice),
}

impl<'a, 'b> TreeNode<'a, 'b> {
    pub(crate) fn name(&self) -> &OsStr {
        if let Some(path) = self.fullpath.as_path().file_name() {
            path
        } else {
            "/".as_ref()
        }
    }

    pub(crate) fn from_inner_node(
        fullpath: PathBuf,
        inner_node: &'b InnerNode<SquashfsFileWriter<'a>>,
    ) -> Self {
        let inner = match inner_node {
            InnerNode::File(file) => InnerTreeNode::FilePhase1(file),
            InnerNode::Symlink(sym) => InnerTreeNode::Symlink(sym),
            InnerNode::Dir(dir) => InnerTreeNode::Dir(dir, BTreeMap::new()),
            InnerNode::CharacterDevice(char) => InnerTreeNode::CharacterDevice(char),
            InnerNode::BlockDevice(block) => InnerTreeNode::BlockDevice(block),
        };
        Self {
            fullpath,
            inode_id: 0,
            inner,
        }
    }

    fn insert(&mut self, components: &[&OsStr], node: TreeNode<'a, 'b>) {
        let dir = match &mut self.inner {
            InnerTreeNode::Dir(_, dir) => dir,
            _ => todo!("Error node inside non-Dir"),
        };

        let (first, rest) = match components {
            [first, rest @ ..] => (first, rest),
            [] => todo!("Error node have no name"),
        };
        let is_file = rest.is_empty();
        let children = dir.get_mut(*first);
        match (is_file, children) {
            //this file already exists
            (true, Some(_file)) => {
                //TODO directory is allowed to be duplicated??? ignore the second file?
            },
            //this file don't exist in this dir, add it
            (true, None) => {
                dir.insert(first.into(), node);
            },
            //not a file, dir, and it already exists
            (false, Some(dir)) => dir.insert(rest, node),
            //not a file, dir, but the dir don't exits
            _ => todo!("Error Dir don't exists"),
        }
    }

    pub fn children(&self) -> Option<&BTreeMap<OsString, TreeNode<'a, 'b>>> {
        match &self.inner {
            InnerTreeNode::Dir(_, dir) => Some(dir),
            _ => None,
        }
    }
    pub fn have_children(&self) -> bool {
        self.children().map(|c| !c.is_empty()).unwrap_or(false)
    }

    pub fn calculate_inode(&mut self, inode_counter: &'_ mut u32) {
        self.inode_id = *inode_counter;
        *inode_counter += 1;

        if let InnerTreeNode::Dir(_, dir) = &mut self.inner {
            dir.values_mut()
                .for_each(|child| child.calculate_inode(inode_counter));
        }
    }

    pub fn write_data<W: Write + Seek>(
        &mut self,
        writer: &mut W,
        data_writer: &mut DataWriter,
    ) -> Result<(), SquashfsError> {
        match &mut self.inner {
            InnerTreeNode::FilePhase1(file) => {
                let (filesize, added) =
                    data_writer.add_bytes(file.reader.borrow_mut().as_mut(), writer)?;
                self.inner = InnerTreeNode::FilePhase2(filesize, added, &file.header);
            },
            InnerTreeNode::Dir(_path, dir) => {
                dir.values_mut()
                    .try_for_each(|child| child.write_data(writer, data_writer))?;
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
    pub fn write_inode_dir(
        &'b self,
        inode_writer: &'_ mut MetadataWriter,
        dir_writer: &'_ mut MetadataWriter,
        parent_inode: u32,
    ) -> Result<(Option<Entry>, u64), SquashfsError> {
        // If no children, just return since it doesn't have anything recursive/new
        // directories
        if !self.have_children() {
            return Ok((None, 0));
        }

        let (path_node, dir) = match &self.inner {
            InnerTreeNode::Dir(path_node, dir) => (path_node, dir),
            _ => unreachable!(),
        };

        // ladies and gentlemen, we have a directory
        let mut write_entries = vec![];

        // store parent Inode, this is used for child Dirs, as they will need this to reference
        // back to this
        let this_inode = self.inode_id;

        // tree has children, this is a Dir, get information of every child node
        for child in dir.values() {
            let (l_dir_entries, _) = child.write_inode_dir(inode_writer, dir_writer, this_inode)?;
            if let Some(entry) = l_dir_entries {
                write_entries.push(entry);
            }
        }

        // write child inodes
        for node in dir.values().filter(|c| !c.have_children()) {
            let entry = match &node.inner {
                InnerTreeNode::Dir(path, _) => Entry::path(
                    node.name(),
                    path,
                    node.inode_id,
                    this_inode,
                    inode_writer,
                    3, // Empty path
                    dir_writer.uncompressed_bytes.len() as u16,
                    dir_writer.metadata_start,
                ),
                InnerTreeNode::FilePhase2(filesize, added, header) => Entry::file(
                    node.name(),
                    header,
                    node.inode_id,
                    inode_writer,
                    *filesize,
                    added,
                ),
                InnerTreeNode::Symlink(symlink) => {
                    Entry::symlink(node.name(), symlink, node.inode_id, inode_writer)
                },
                InnerTreeNode::CharacterDevice(char) => {
                    Entry::char(node.name(), char, node.inode_id, inode_writer)
                },
                InnerTreeNode::BlockDevice(block) => {
                    Entry::block_device(node.name(), block, node.inode_id, inode_writer)
                },
                _ => unreachable!(),
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
            let bytes = dir.to_bytes()?;
            total_size += bytes.len() as u16;
            dir_writer.write_all(&bytes)?;
        }

        //trace!("BEFORE: {:#02x?}", child);
        let entry = Entry::path(
            self.name(),
            path_node,
            this_inode,
            parent_inode,
            inode_writer,
            total_size,
            block_offset,
            block_index,
        );
        let root_inode = ((entry.start as u64) << 16) | ((entry.offset as u64) & 0xffff);

        trace!("[{:?}] entries: {:#02x?}", self.name(), &entry);
        Ok((Some(entry), root_inode))
    }
}

impl<'a, 'b> From<&'b FilesystemWriter<'a>> for TreeNode<'a, 'b> {
    fn from(fs: &'b FilesystemWriter<'a>) -> Self {
        let mut tree = TreeNode {
            fullpath: "/".into(),
            inner: InnerTreeNode::Dir(&fs.root_inode, BTreeMap::new()),
            inode_id: 0,
        };
        //all nodes, except root
        for node in fs.nodes.iter() {
            let path = node.path.as_path();
            let comp = normalized_components(path);

            if comp.is_empty() {
                //ignore root
                continue;
            }
            let fullpath = comp.iter().collect();
            let node = Self::from_inner_node(fullpath, &node.inner);
            tree.insert(&comp, node);
        }

        let mut inode = 1;
        tree.calculate_inode(&mut inode);
        tree
    }
}
