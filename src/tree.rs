use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Component::*;
use std::path::{Path, PathBuf};

use crate::filesystem::{
    FilesystemWriter, InnerNode, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir,
    SquashfsFileWriter, SquashfsSymlink,
};

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
            _ => todo!("Error node have no name"),
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
}

impl<'a, 'b> From<&'b FilesystemWriter<'a>> for TreeNode<'a, 'b> {
    fn from(fs: &'b FilesystemWriter<'a>) -> Self {
        let mut tree = TreeNode {
            fullpath: "/".into(),
            inner: InnerTreeNode::Dir(&fs.root_inode, BTreeMap::new()),
        };
        for node in &fs.nodes {
            let path = node.path.as_path();
            let comp = normalized_components(path);
            tree.insert(&mut PathBuf::new(), &comp, &node.inner);
        }

        tree
    }
}
