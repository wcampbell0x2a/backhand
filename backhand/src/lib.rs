//! Library and binaries for the reading, creating, and modification
//! of [SquashFS](https://en.wikipedia.org/wiki/SquashFS) file systems.
//!
//! ## Library
//! Add the following to your `Cargo.toml` file:
//! ```toml
//! [dependencies]
//! backhand = "0.25.0"
//! ```
//!
//! ### Reading
//! For reading an image and extracting its details and contents, use
//! [`FilesystemReader::from_reader`].
//!
//! ### Writing
//! For creating a modified or new image, use [`FilesystemWriter::from_fs_reader`].
//! [`FilesystemWriter`] can also be created from scratch, without a previous image to base itself
//! on.
//!
//!### Example
//!```rust,no_run
//! # use std::fs::File;
//! # use std::io::{Cursor, BufReader};
//! # use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
//! // read
//! let file = BufReader::new(File::open("file.squashfs").unwrap());
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
//!
//! # Features
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
type _ReadmeTest = ();

pub mod error;
mod kinds;
pub mod traits;
#[cfg(feature = "v3")]
pub mod v3;
#[cfg(feature = "v3_lzma")]
pub mod v3_lzma;
pub mod v4;

#[cfg(feature = "v3")]
pub use crate::v3::V3;
pub use crate::v4::V4;
pub use crate::v4::data::DataSize;
pub use crate::v4::export::Export;
pub use crate::v4::filesystem::node::{
    InnerNode, Node, NodeHeader, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir,
    SquashfsFileReader, SquashfsFileWriter, SquashfsSymlink,
};
pub use crate::v4::filesystem::reader::{FilesystemReader, FilesystemReaderFile};
#[cfg(not(feature = "parallel"))]
pub use crate::v4::filesystem::reader_no_parallel::SquashfsReadFile;
#[cfg(feature = "parallel")]
pub use crate::v4::filesystem::reader_parallel::SquashfsReadFile;
pub use crate::v4::filesystem::writer::{
    CompressionExtra, ExtraXz, FilesystemCompressor, FilesystemWriter,
};
pub use crate::v4::fragment::Fragment;
pub use crate::v4::id::Id;
pub use crate::v4::inode::{BasicFile, Inode};
pub use crate::v4::reader::BufReadSeek;
pub use crate::v4::squashfs::{
    DEFAULT_BLOCK_SIZE, DEFAULT_PAD_LEN, Flags, MAX_BLOCK_SIZE, MIN_BLOCK_SIZE, Squashfs,
    SuperBlock,
};

pub use crate::error::BackhandError;
pub use crate::traits::squashfs::create_squashfs_from_kind;
pub use crate::traits::{FilesystemReaderTrait, GenericSquashfs, SquashfsVersion};

/// Support the wonderful world of vendor formats
pub mod kind {
    pub use crate::kinds::{AVM_BE_V4_0, BE_V4_0, Endian, Kind, LE_V4_0, Magic};
    #[cfg(feature = "v3")]
    pub use crate::kinds::{BE_V3_0, LE_V3_0};
    #[cfg(feature = "v3_lzma")]
    pub use crate::kinds::{
        BE_V3_0_LZMA, LE_V3_0_LZMA, NETGEAR_BE_V3_0_LZMA, NETGEAR_BE_V3_0_LZMA_STANDARD,
    };
}

/// Compression Choice and Options
pub mod compression {
    pub use crate::traits::CompressionAction;
    pub use crate::v4::compressor::{
        CompressionOptions, Compressor, DefaultCompressor, Gzip, Lz4, Lzo, Xz, Zstd,
    };
}
