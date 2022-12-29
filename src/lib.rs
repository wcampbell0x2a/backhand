#![doc = include_str!("../README.md")]

pub mod compressor;
mod data;
pub mod dir;
pub mod error;
pub mod filesystem;
pub mod fragment;
pub mod inode;
mod metadata;
mod reader;
pub mod squashfs;
mod tree;
mod writer;

pub use crate::filesystem::Filesystem;
pub use crate::squashfs::Squashfs;
