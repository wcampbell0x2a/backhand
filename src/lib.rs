//! Library and binaries for the reading, creating, and modification
//! of [SquashFS](https://en.wikipedia.org/wiki/SquashFS) file systems.
//!
//! ## Library
//! Add the following to your `Cargo.toml` file:
//! ```toml
//! [dependencies]
//! backhand = "0.13.0"
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
//! ## Feature flags
#![doc = document_features::document_features!()]

#[doc = include_str!("../README.md")]
type _ReadmeTest = ();

pub mod bufread;
pub mod compressor;
pub mod error;
mod flags;
pub mod kinds;
pub mod v3;
pub mod v4;
use compressor::CompressionOptions;
use kind::Kind;
use kinds::Version;

pub use crate::bufread::BufReadSeek;
pub use crate::compressor::{CompressionExtra, ExtraXz, FilesystemCompressor};
pub use crate::v4::data::DataSize;
pub use crate::v4::export::Export;
pub use crate::v4::filesystem::node::{
    InnerNode, Node, NodeHeader, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir,
    SquashfsFileReader, SquashfsFileWriter, SquashfsSymlink,
};
pub use crate::v4::filesystem::reader::{FilesystemReader, FilesystemReaderFile, SquashfsReadFile};
pub use crate::v4::filesystem::writer::FilesystemWriter;
pub use crate::v4::fragment::Fragment;
pub use crate::v4::id::Id;
pub use crate::v4::inode::{BasicFile, Inode};
pub use crate::v4::squashfs::{
    Squashfs, SuperBlock, DEFAULT_BLOCK_SIZE, DEFAULT_PAD_LEN, MAX_BLOCK_SIZE, MIN_BLOCK_SIZE,
};

pub use crate::error::BackhandError;

/// Support the wonderful world of vendor formats
pub mod kind {
    pub use crate::kinds::{Endian, Kind, Magic, AVM_BE_V4_0, BE_V4_0, LE_V4_0};
}

/// Compression Choice and Options
pub mod compression {
    pub use crate::compressor::{
        CompressionAction, CompressionOptions, Compressor, DefaultCompressor, Gzip, Lz4, Lzo, Xz,
        Zstd,
    };
}

pub enum MultiSuperBlock {
    V3(v3::SuperBlock),
    V4(v4::SuperBlock),
}

impl MultiSuperBlock {
    pub fn superblock_and_compression_options<'a>(
        reader: &mut Box<dyn BufReadSeek + 'a>,
        kind: &Kind,
    ) -> Result<(Self, Option<CompressionOptions>), BackhandError> {
        match kind.inner.version {
            Version::V3_0 => {
                let (s, comp) = v3::Squashfs::superblock_and_compression_options(reader, kind)?;
                Ok((MultiSuperBlock::V3(s), comp))
            }
            Version::V4_0 => {
                let (s, comp) = v4::Squashfs::superblock_and_compression_options(reader, kind)?;
                Ok((MultiSuperBlock::V4(s), comp))
            }
        }
    }
}

pub enum MultiSquashfs<'a> {
    V3(v3::Squashfs<'a>),
    V4(v4::Squashfs<'a>),
}

impl<'a> MultiSquashfs<'a> {
    pub fn from_reader_with_offset_and_kind(
        reader: impl BufReadSeek + 'static,
        offset: u64,
        kind: Kind,
    ) -> Result<MultiSquashfs<'a>, BackhandError> {
        match kind.inner.version {
            Version::V3_0 => {
                let squashfs =
                    v3::Squashfs::from_reader_with_offset_and_kind(reader, offset, kind)?;
                Ok(MultiSquashfs::V3(squashfs))
            }
            Version::V4_0 => {
                let squashfs =
                    v4::Squashfs::from_reader_with_offset_and_kind(reader, offset, kind)?;
                Ok(MultiSquashfs::V4(squashfs))
            }
        }
    }

    pub fn into_filesystem_reader(self) -> Result<MultiFilesystemReader<'a>, BackhandError> {
        let a = match self {
            Self::V3(v3) => MultiFilesystemReader::V3(v3.into_filesystem_reader()?),
            Self::V4(v4) => MultiFilesystemReader::V4(v4.into_filesystem_reader()?),
        };

        Ok(a)
    }
}

pub enum MultiFilesystemReader<'a> {
    V3(v3::FilesystemReader<'a>),
    V4(v4::FilesystemReader<'a>),
}

impl<'a> MultiFilesystemReader<'a> {
    pub fn from_reader_with_offset_and_kind(
        reader: impl BufReadSeek + 'static,
        offset: u64,
        kind: Kind,
    ) -> Result<Self, BackhandError> {
        match kind.inner.version {
            Version::V3_0 => {
                let fs =
                    v3::FilesystemReader::from_reader_with_offset_and_kind(reader, offset, kind)?;
                Ok(Self::V3(fs))
            }
            Version::V4_0 => {
                let fs =
                    v4::FilesystemReader::from_reader_with_offset_and_kind(reader, offset, kind)?;
                Ok(Self::V4(fs))
            }
        }
    }
}
