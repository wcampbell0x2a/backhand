//! The filesystem tree of a v1 or v2 image
pub mod node;
pub mod reader;

// `normalize_squashfs_path` is version-neutral, so v4's is used rather than
// repeated here.
pub use crate::v4::filesystem::normalize_squashfs_path;
