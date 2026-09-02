//! SquashFS v1 and v2
//!
//! One module covers both versions. They share the superblock prefix, the
//! directory table, the metadata block format and the id tables; only the inode
//! layout really differs, and [`inode::Inode`] normalizes that away. The
//! reference tools make the same choice, in `unsquash-12.c`.

pub mod compressor;
pub mod data;
pub mod dir;
pub mod filesystem;
pub mod fragment;
pub mod inode;
pub mod metadata;
pub mod reader;
pub mod squashfs;

pub use inode::Layout;
