use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Component::*;
use std::path::{Path, PathBuf};

use crate::filesystem::{Filesystem, InnerNode};

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

// TODO this can be either have a node or children not both, can be a Enum?
#[derive(Debug)]
pub(crate) struct TreeNode<'a> {
    pub fullpath: PathBuf,
    pub node: Option<&'a InnerNode>,
    pub children: BTreeMap<PathBuf, TreeNode<'a>>,
}

impl<'a> TreeNode<'a> {
    pub(crate) fn name(&self) -> OsString {
        if let Some(path) = self.fullpath.as_path().file_name() {
            path.into()
        } else {
            "/".into()
        }
    }

    fn insert(&mut self, fullpath: &mut PathBuf, components: &[&OsStr], og_node: &'a InnerNode) {
        if let Some((first, rest)) = components.split_first() {
            fullpath.push(first);

            // no rest, we have the file
            let node = if rest.is_empty() { Some(og_node) } else { None };
            let entry = self
                .children
                .entry(fullpath.to_path_buf())
                .or_insert(TreeNode {
                    fullpath: fullpath.clone(),
                    node,
                    children: BTreeMap::new(),
                });
            entry.insert(fullpath, rest, og_node);
        }
    }
}

impl<'a> From<&'a Filesystem> for TreeNode<'a> {
    fn from(fs: &'a Filesystem) -> Self {
        let mut tree = TreeNode {
            fullpath: "/".into(),
            node: None,
            children: BTreeMap::new(),
        };
        for node in &fs.nodes {
            let path = node.path.as_path();
            let comp = normalized_components(path);
            tree.insert(&mut PathBuf::new(), &comp, &node.inner);
        }

        tree
    }
}
