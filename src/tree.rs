use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::{Seek, Write};
use std::path::Component::*;
use std::path::{Path, PathBuf};

use deku::DekuContainerWrite;
use tracing::trace;

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

    #[allow(clippy::type_complexity)]
    pub fn write<W: Write + Seek>(
        &'b self,
        inode_counter: &'_ mut u32,
        writer: &mut W,
        inode_writer: &'_ mut MetadataWriter,
        dir_writer: &'_ mut MetadataWriter,
        data_writer: &'_ mut DataWriter,
        parent_inode: u32,
    ) -> Result<(Entry, u64), SquashfsError> {
        *inode_counter += 1;
        let this_inode = *inode_counter;

        //if not dir, just return the entry, if dir recursive call write to children and
        //return the entries
        let (path, child_entries) = match &self.inner {
            InnerTreeNode::File(file) => {
                let entry = Entry::file(
                    &self.fullpath,
                    file,
                    writer,
                    this_inode,
                    data_writer,
                    inode_writer,
                );
                return Ok((entry, 0));
            },
            InnerTreeNode::Symlink(symlink) => {
                let entry = Entry::symlink(&self.fullpath, symlink, this_inode, inode_writer);
                return Ok((entry, 0));
            },
            InnerTreeNode::CharacterDevice(char) => {
                let entry = Entry::char(&self.fullpath, char, this_inode, inode_writer);
                return Ok((entry, 0));
            },
            InnerTreeNode::BlockDevice(block) => {
                let entry = Entry::block_device(&self.fullpath, block, this_inode, inode_writer);
                return Ok((entry, 0));
            },
            InnerTreeNode::Dir(path, children) => {
                let children = children
                    .values()
                    .map(|child| {
                        Self::write(
                            child,
                            inode_counter,
                            writer,
                            inode_writer,
                            dir_writer,
                            data_writer,
                            this_inode,
                        )
                        .map(|res| res.0) // only entry
                    })
                    .collect::<Result<_, _>>()?;
                (path, children)
            },
        };

        //only dir executes after this point

        // write dir
        let block_offset = dir_writer.uncompressed_bytes.len() as u16;
        trace!("WRITING DIR: {block_offset:#02x?}");
        let mut total_size = 3;
        for dir in Entry::into_dir(child_entries) {
            trace!("WRITING DIR: {dir:#02x?}");
            let bytes = dir.to_bytes()?;
            total_size += bytes.len() as u16;
            dir_writer.write_all(&bytes)?;
        }

        //trace!("BEFORE: {:#02x?}", child);
        // write parent_inode
        let entry = Entry::path(
            self.name(),
            path,
            this_inode,
            parent_inode,
            dir_writer,
            inode_writer,
            total_size,
        );
        let root_inode = ((entry.start as u64) << 16) | ((entry.offset as u64) & 0xffff);

        trace!("[{:?}] entry: {entry:#02x?}", self.name());
        trace!("[{:?}] node: None", self.name());
        Ok((entry, root_inode))
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
