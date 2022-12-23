//! # Squashfs-deku
//! Read and extract information from a Squashfs image

pub mod compressor;
mod data;
pub mod dir;
pub mod error;
pub mod fragment;
pub mod inode;
mod metadata;
mod reader;
pub mod squashfs;
mod tree;
pub mod writer;

pub use crate::squashfs::Squashfs;
