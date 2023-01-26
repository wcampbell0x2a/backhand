use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Component::*;
use std::path::{Path, PathBuf};

use crate::filesystem::{FilesystemWriter, InnerNode, SquashfsFileWriter};

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
    pub node: Option<&'b InnerNode<SquashfsFileWriter<'a>>>,
    pub children: BTreeMap<PathBuf, TreeNode<'a, 'b>>,
}

impl<'a, 'b> TreeNode<'a, 'b> {
    pub(crate) fn name(&self) -> OsString {
        if let Some(path) = self.fullpath.as_path().file_name() {
            path.into()
        } else {
            "/".into()
        }
    }

    fn insert(
        &mut self,
        fullpath: &mut PathBuf,
        components: &[&OsStr],
        og_node: &'b InnerNode<SquashfsFileWriter<'a>>,
    ) {
        if let Some((first, rest)) = components.split_first() {
            fullpath.push(first);

            // no rest, we have the file
            let node = rest.is_empty().then(|| og_node);
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

impl<'a, 'b> From<&'b FilesystemWriter<'a>> for TreeNode<'a, 'b> {
    fn from(fs: &'b FilesystemWriter<'a>) -> Self {
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
