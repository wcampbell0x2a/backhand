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
//! For reading an image and extracting its details and contents, use
//! [`FilesystemReader::from_reader`].
//! For creating a modified or new image, use [`FilesystemWriter::from_fs_reader`].
//! [`FilesystemWriter`] can also be created from scratch, without a previous image to base itself
//! on.
//!
//!### Reading/Writing/Modifying Firmware
//!```rust,no_run
//! # use std::fs::File;
//! # use backhand::{FilesystemReader, FilesystemWriter, FilesystemHeader};
//!
//! // read
//! let file = File::open("file.squashfs").unwrap();
//! let read_filesystem = FilesystemReader::from_reader(file).unwrap();
//!
//! // convert to writer
//! let mut write_filesystem = FilesystemWriter::from_fs_reader(&read_filesystem).unwrap();
//!
//! // add file with data from slice
//! let d = FilesystemHeader::default();
//! let bytes = &mut b"Fear is the mind-killer.".as_slice();
//! write_filesystem.push_file(bytes, "a/d/e/new_file", d);
//!
//! // add file with data from file
//! let mut new_file = File::open("dune").unwrap();
//! write_filesystem.push_file(&mut new_file, "/root/dune", d);
//!
//! // convert into bytes
//! let bytes = write_filesystem.to_bytes().unwrap();
//! ```

#[doc = include_str!("../README.md")]
type _ReadmeTest = ();

pub mod compressor;
mod data;
pub mod dir;
mod entry;
pub mod error;
pub mod filesystem;
pub mod fragment;
pub mod inode;
mod metadata;
pub mod reader;
pub mod squashfs;
mod tree;

pub use crate::filesystem::{FilesystemHeader, FilesystemReader, FilesystemWriter};
pub use crate::squashfs::Squashfs;
