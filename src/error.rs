//! Errors

use std::{io, string};

use thiserror::Error;

use crate::compressor::Compressor;
use crate::inode::InodeInner;

#[derive(Error, Debug)]
pub enum SquashfsError {
    #[error("std io error: {0}")]
    StdIo(#[from] io::Error),

    #[error("deku error: {0:?}")]
    Deku(#[from] deku::DekuError),

    #[error("string error: {0:?}")]
    StringUtf8(#[from] string::FromUtf8Error),

    #[error("string error: {0:?}")]
    StrUtf8(#[from] std::str::Utf8Error),

    #[error("unsupported compression: {0:?}")]
    UnsupportedCompression(Compressor),

    #[error("file not found")]
    FileNotFound,

    #[error("branch was thought to be unreachable")]
    Unreachable,

    #[error("inode {0:?} was unexpected in this position")]
    UnexpectedInode(InodeInner),

    #[error("unsupported inode: {0:?}, please fill github issue to add support")]
    UnsupportedInode(InodeInner),

    #[error("corrupted or invalid squashfs image")]
    CorruptedOrInvalidSquashfs,
}

impl From<SquashfsError> for io::Error {
    fn from(value: SquashfsError) -> Self {
        match value {
            SquashfsError::StdIo(io) => io,
            SquashfsError::Deku(e) => e.into(),
            SquashfsError::StringUtf8(e) => Self::new(io::ErrorKind::InvalidData, e),
            SquashfsError::StrUtf8(e) => Self::new(io::ErrorKind::InvalidData, e),
            e @ SquashfsError::UnsupportedCompression(_) => {
                Self::new(io::ErrorKind::Unsupported, e)
            },
            e @ SquashfsError::FileNotFound => Self::new(io::ErrorKind::NotFound, e),
            e @ SquashfsError::Unreachable => Self::new(io::ErrorKind::InvalidData, e),
            e @ SquashfsError::UnexpectedInode(_) => Self::new(io::ErrorKind::InvalidData, e),
            e @ SquashfsError::UnsupportedInode(_) => Self::new(io::ErrorKind::InvalidData, e),
            e @ SquashfsError::CorruptedOrInvalidSquashfs => {
                Self::new(io::ErrorKind::InvalidData, e)
            },
        }
    }
}
