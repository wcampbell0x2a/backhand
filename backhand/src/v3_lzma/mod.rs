//! SquashFS v3 with LZMA compression support

pub mod compressor;
pub mod standard_compressor;

pub use crate::v3::{
    data, dir, export, filesystem, fragment, id, inode, metadata, reader, squashfs, unix_string,
};

pub use compressor::LzmaAdaptiveCompressor;
pub use standard_compressor::LzmaStandardCompressor;
