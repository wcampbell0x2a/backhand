//! Errors

use std::{io, string};

use thiserror::Error;

/// Errors generated from library
#[derive(Error, Debug)]
pub enum BackhandError {
    #[error("std io error: {0}")]
    StdIo(#[from] io::Error),

    #[error("deku error: {0:?}")]
    Deku(#[from] deku::DekuError),

    #[error("string error: {0:?}")]
    StringUtf8(#[from] string::FromUtf8Error),

    #[error("string error: {0:?}")]
    StrUtf8(#[from] std::str::Utf8Error),

    #[error("unsupported compression")]
    UnsupportedCompression,

    #[error("file not found")]
    FileNotFound,

    #[error("branch was thought to be unreachable")]
    Unreachable,

    #[error("inode was unexpected in this position")]
    UnexpectedInode,

    #[error("unsupported inode: please fill github issue to add support")]
    UnsupportedInode,

    #[error("corrupted or invalid squashfs image")]
    CorruptedOrInvalidSquashfs,

    #[error("invalid squashfs compression options")]
    InvalidCompressionOption,

    #[error("Invalid file path in the squashfs image")]
    InvalidFilePath,

    #[error("file inside squashfs image have no name")]
    UndefineFileName,

    #[error("file duplicated in squashfs image")]
    DuplicatedFileName,
}

impl From<BackhandError> for io::Error {
    fn from(value: BackhandError) -> Self {
        use BackhandError::*;
        match value {
            StdIo(io) => io,
            Deku(e) => e.into(),
            StringUtf8(e) => Self::new(io::ErrorKind::InvalidData, e),
            StrUtf8(e) => Self::new(io::ErrorKind::InvalidData, e),
            e @ UnsupportedCompression => Self::new(io::ErrorKind::Unsupported, e),
            e @ FileNotFound => Self::new(io::ErrorKind::NotFound, e),
            e @ (Unreachable
            | UnexpectedInode
            | UnsupportedInode
            | CorruptedOrInvalidSquashfs
            | InvalidCompressionOption
            | InvalidFilePath
            | UndefineFileName
            | DuplicatedFileName) => Self::new(io::ErrorKind::InvalidData, e),
        }
    }
}
