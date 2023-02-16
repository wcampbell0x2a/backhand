//! Library and binaries for the reading, creating, and modification
//! of [SquashFS](https://en.wikipedia.org/wiki/SquashFS) file systems.
//!
//! ## Library
//! Add the following to your `Cargo.toml` file:
//! ```toml
//! [dependencies]
//! backhand = "0.9.1"
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
//! # use std::io::Cursor;
//! # use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
//! // read
//! let file = File::open("file.squashfs").unwrap();
//! let read_filesystem = FilesystemReader::from_reader(file).unwrap();
//!
//! // convert to writer
//! let mut write_filesystem = FilesystemWriter::from_fs_reader(&read_filesystem).unwrap();
//!
//! // add file with data from slice
//! let d = NodeHeader::default();
//! let bytes = Cursor::new(b"Fear is the mind-killer.");
//! write_filesystem.push_file(bytes, "a/d/e/new_file", d);
//!
//! // add file with data from file
//! let new_file = File::open("dune").unwrap();
//! write_filesystem.push_file(new_file, "/root/dune", d);
//!
//! // replace a existing file
//! let bytes = Cursor::new(b"The sleeper must awaken.\n");
//! write_filesystem
//!     .replace_file("/a/b/c/d/e/first_file", bytes)
//!     .unwrap();
//!
//! // write into a new file
//! let mut output = File::create("modified.squashfs").unwrap();
//! write_filesystem.write(&mut output).unwrap();
//! ```

#[doc = include_str!("../README.md")]
type _ReadmeTest = ();

mod compressor;
mod data;
mod dir;
mod entry;
mod error;
mod filesystem;
mod fragment;
mod inode;
mod metadata;
mod reader;
mod squashfs;
mod tree;

pub use crate::filesystem::{
    FilesystemReader, FilesystemReaderFile, FilesystemWriter, InnerNode, Node, NodeHeader,
    SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir, SquashfsFileReader,
    SquashfsFileSource, SquashfsFileWriter, SquashfsSymlink,
};
pub use crate::reader::ReadSeek;
pub use crate::squashfs::Squashfs;

/// Compression Choice and Options
pub mod compression {
    pub use crate::compressor::{CompressionOptions, Compressor, Gzip, Lz4, Lzo, Xz, Zstd};
}

/// [`Squashfs`] internal structs
pub mod internal {
    pub use crate::fragment::Fragment;
    pub use crate::inode::Inode;
    pub use crate::squashfs::{Export, Id, SuperBlock};
}
