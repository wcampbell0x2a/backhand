use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::{Seek, Write};
use std::path::Component::*;
use std::path::{Path, PathBuf};

use deku::DekuContainerWrite;
use tracing::{instrument, trace};

use crate::data::DataWriter;
use crate::entry::Entry;
use crate::error::SquashfsError;
use crate::filesystem::{
    FilesystemWriter, InnerNode, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir,
    SquashfsFileWriter, SquashfsSymlink,
};
use crate::metadata::MetadataWriter;

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
    pub inner: InnerTreeNode<'a, 'b>,
}

#[derive(Debug)]
pub(crate) enum InnerTreeNode<'a, 'b> {
    File(&'b SquashfsFileWriter<'a>),
    Symlink(&'b SquashfsSymlink),
    Dir(&'b SquashfsDir, BTreeMap<PathBuf, TreeNode<'a, 'b>>),
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
            InnerNode::File(file) => InnerTreeNode::File(file),
            InnerNode::Symlink(sym) => InnerTreeNode::Symlink(sym),
            InnerNode::Dir(dir) => InnerTreeNode::Dir(dir, BTreeMap::new()),
            InnerNode::CharacterDevice(char) => InnerTreeNode::CharacterDevice(char),
            InnerNode::BlockDevice(block) => InnerTreeNode::BlockDevice(block),
        };
        Self { fullpath, inner }
    }

    fn insert(
        &mut self,
        fullpath: &mut PathBuf,
        components: &[&OsStr],
        og_node: &'b InnerNode<SquashfsFileWriter<'a>>,
    ) {
        let (first, rest) = match components {
            [first, rest @ ..] => (first, rest),
            _ => {
                todo!("Error node have no name")
            },
        };
        fullpath.push(first);
        let dir = match &mut self.inner {
            InnerTreeNode::Dir(_, dir) => dir,
            _ => todo!("Error node inside non-Dir"),
        };

        let is_file = rest.is_empty();
        let children = dir.get_mut(fullpath);
        match (is_file, children) {
            //this file already exists
            (true, Some(_file)) => {
                //TODO directory is allowed to be duplicated???
                //todo!("Error File already exists in the tree")
            },
            //this file don't exist in this dir, add it
            (true, None) => {
                dir.insert(
                    fullpath.to_owned(),
                    Self::from_inner_node(fullpath.to_owned(), og_node),
                );
            },
            //not a file, dir, and it already exists
            (false, Some(dir)) => dir.insert(fullpath, rest, og_node),
            //not a file, dir, but the dir don't exits
            _ => todo!("Error Dir don't exists"),
        }
    }

    pub fn children(&self) -> Option<&BTreeMap<PathBuf, TreeNode<'a, 'b>>> {
        match &self.inner {
            InnerTreeNode::Dir(_, dir) => Some(dir),
            _ => None,
        }
    }
    pub fn have_children(&self) -> bool {
        self.children().map(|c| !c.is_empty()).unwrap_or(false)
    }

    /// Create SquashFS file system from each node of Tree
    ///
    /// This works my recursively creating Inodes and Dirs for each node in the tree. This also
    /// keeps track of parent directories by calling this function on all nodes of a dir to get only
    /// the nodes, but going into the child dirs in the case that it contains a child dir.
    #[instrument(skip_all)]
    #[allow(clippy::type_complexity)]
    pub fn write<W: Write + Seek>(
        &'b self,
        inode_counter: &'_ mut u32,
        writer: &mut W,
        inode_writer: &'_ mut MetadataWriter,
        dir_writer: &'_ mut MetadataWriter,
        data_writer: &'_ mut DataWriter,
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
        let this_inode = *inode_counter;
        *inode_counter += 1;

        // tree has children, this is a Dir, get information of every child node
        for child in dir.values() {
            let (l_dir_entries, _) = child.write(
                inode_counter,
                writer,
                inode_writer,
                dir_writer,
                data_writer,
                this_inode,
            )?;
            if let Some(entry) = l_dir_entries {
                write_entries.push(entry);
            }
        }

        // write child inodes
        for node in dir.values().filter(|c| !c.have_children()) {
            let node_path = PathBuf::from(node.name());
            let entry = match &node.inner {
                InnerTreeNode::Dir(path, _) => Entry::path(
                    node.name(),
                    path,
                    *inode_counter,
                    this_inode,
                    inode_writer,
                    3, // Empty path
                    dir_writer.uncompressed_bytes.len() as u16,
                    dir_writer.metadata_start,
                ),
                InnerTreeNode::File(file) => Entry::file(
                    &node_path,
                    file,
                    writer,
                    *inode_counter,
                    data_writer,
                    inode_writer,
                ),
                InnerTreeNode::Symlink(symlink) => {
                    Entry::symlink(&node_path, symlink, *inode_counter, inode_writer)
                },
                InnerTreeNode::CharacterDevice(char) => {
                    Entry::char(&node_path, char, *inode_counter, inode_writer)
                },
                InnerTreeNode::BlockDevice(block) => {
                    Entry::block_device(&node_path, block, *inode_counter, inode_writer)
                },
            };
            write_entries.push(entry);
            *inode_counter += 1;
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
        };
        //all nodes, except root
        for node in &fs.nodes {
            let path = node.path.as_path();
            let comp = normalized_components(path);
            if comp.is_empty() {
                //ignore root
                continue;
            }
            tree.insert(&mut PathBuf::new(), &comp, &node.inner);
        }

        tree
    }
}
