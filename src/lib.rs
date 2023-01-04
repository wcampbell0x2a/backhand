#![doc = include_str!("../README.md")]

pub mod compressor;
mod data;
mod dir;
mod entry;
pub mod error;
pub mod filesystem;
mod fragment;
mod inode;
mod metadata;
mod reader;
pub mod squashfs;
mod tree;

pub use crate::filesystem::Filesystem;
pub use crate::squashfs::Squashfs;
