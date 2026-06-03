//! In-memory representation of SquashFS filesystem tree used for writing to image
/// Non-parallel file reading implementation
#[cfg(not(feature = "parallel"))]
pub mod reader_no_parallel;
/// Parallel file reading implementation
#[cfg(feature = "parallel")]
pub mod reader_parallel;

/// Filesystem node types
pub mod node;
/// Filesystem reader
pub mod reader;
/// Filesystem writer
pub mod writer;

use std::path::{Component, Path, PathBuf};

use crate::error::BackhandError;

// normalize the path, always starts with root, solve relative paths and don't
// allow prefix (windows stuff like "C:/")
/// Normalize a path to always start from root, resolving `.` and `..`
pub fn normalize_squashfs_path(src: &Path) -> Result<PathBuf, BackhandError> {
    //always starts with root "/"
    let mut ret = PathBuf::from(Component::RootDir.as_os_str());
    for component in src.components() {
        match component {
            Component::Prefix(..) => return Err(BackhandError::InvalidFilePath),
            //ignore, root, always added on creation
            Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    Ok(ret)
}
