//! Library and binaries for the reading, creating, and modification
//! of [SquashFS](https://en.wikipedia.org/wiki/SquashFS) file systems.
//!
//! ## Library
//! Add the following to your `Cargo.toml` file:
//! ```toml
//! [dependencies]
//! backhand = "0.6.0"
//! ```
//!
//! Most usage of this library just requires the usage of [`Filesystem`], although this library gives
//! you access to some of the innner workings of the reading and writing Squashfs through the
//! [`Squashfs`] struct.
//!
//!### Reading/Writing/Modifying Firmware
//!```rust,ignore
//! # use std::fs::File;
//! # use backhand::{Filesystem, FilesystemHeader};
//!
//! // read
//! let file = File::open("file.squashfs").unwrap();
//! let mut filesystem = Filesystem::from_reader(file).unwrap();
//!
//! // add file with data from slice
//! let d = FilesystemHeader::default();
//! let bytes = &mut b"Fear is the mind-killer.".as_slice();
//! filesystem.push_file(bytes, "a/d/e/new_file", d);
//!
//! // add file with data from file
//! let mut new_file = File::open("dune").unwrap();
//! filesystem.push_file(&mut new_file, "/root/dune", d);
//!
//! // modify file
//! let file = filesystem.mut_file("/a/b/c/d/e/first_file").unwrap();
//! file.bytes = b"The sleeper must awaken.\n".to_vec();
//!
//! // write
//! let bytes = filesystem.to_bytes().unwrap();
//! ```

pub mod compressor;
mod data;
pub mod dir;
mod entry;
pub mod error;
pub mod filesystem;
pub mod fragment;
pub mod inode;
mod metadata;
mod reader;
pub mod squashfs;
mod tree;

pub use crate::filesystem::{FilesystemHeader, FilesystemWriter, FilesystemReader};
pub use crate::squashfs::Squashfs;
