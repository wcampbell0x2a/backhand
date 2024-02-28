//! Errors

use std::collections::TryReserveError;
use std::{io, string};

use thiserror::Error;

use crate::compressor::Compressor;
use crate::inode::InodeInner;

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

    #[error("invalid squashfs compression options")]
    InvalidCompressionOption,

    #[error("Invalid file path in the squashfs image")]
    InvalidFilePath,

    #[error("file inside squashfs image have no name")]
    UndefineFileName,

    #[error("file duplicated in squashfs image")]
    DuplicatedFileName,

    #[error("allocator try_reserve error")]
    TryReserveError(#[from] TryReserveError),

    #[error("invalid id_table for node")]
    InvalidIdTable,
}

impl From<BackhandError> for io::Error {
    fn from(value: BackhandError) -> Self {
        use BackhandError::*;
        match value {
            StdIo(io) => io,
            StringUtf8(_) => Self::from(io::ErrorKind::InvalidData),
            StrUtf8(_) => Self::from(io::ErrorKind::InvalidData),
            UnsupportedCompression(_) => Self::from(io::ErrorKind::Unsupported),
            FileNotFound => Self::from(io::ErrorKind::NotFound),
            Unreachable
            | Deku(_)
            | UnexpectedInode(_)
            | UnsupportedInode(_)
            | CorruptedOrInvalidSquashfs
            | InvalidCompressionOption
            | InvalidFilePath
            | UndefineFileName
            | DuplicatedFileName
            | InvalidIdTable
            | TryReserveError(_) => Self::from(io::ErrorKind::InvalidData),
        }
    }
}
