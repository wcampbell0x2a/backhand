use std::collections::TryReserveError;
use std::{io, string};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackhandError {
    #[error("std io error: {0}")]
    StdIo(#[from] io::Error),

    #[error("deku error: {0:?}")]
    Deku(#[from] deku::DekuError),

    #[error("string error: {0:?}")]
    StringUtf8(#[from] string::FromUtf8Error),

    #[error("string error: {0:?}")]
    StrUtf8(#[from] core::str::Utf8Error),

    #[error("unsupported compression: {0:?}")]
    UnsupportedCompression(String),

    #[error("missing compressor for v4 filesystem")]
    MissingCompressor,

    #[error("file not found")]
    FileNotFound,

    #[error("branch was thought to be unreachable")]
    Unreachable,

    #[error("inode was unexpected in this position")]
    UnexpectedInode,

    #[error("unsupported inode, please fill github issue to add support")]
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

    #[error("allocator try_reserve error")]
    TryReserveError(#[from] TryReserveError),

    #[error("invalid id_table for node")]
    InvalidIdTable,

    #[error("unsupported squashfs version {0}.{1}")]
    UnsupportedSquashfsVersion(u16, u16),

    #[error("numeric conversion failed: {0}")]
    NumericConversion(String),

    #[error("system time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),

    #[error("mutex lock poisoned")]
    MutexPoisoned,

    #[error("uid/gid not found in id table")]
    IdNotFoundInTable,

    #[error("internal state error: {0}")]
    InternalState(String),

    #[error("compression initialization failed: {0}")]
    CompressionInit(String),
}

impl From<BackhandError> for io::Error {
    fn from(value: BackhandError) -> Self {
        use BackhandError::*;
        match value {
            StdIo(io) => io,
            StringUtf8(_) => Self::from(io::ErrorKind::InvalidData),
            StrUtf8(_) => Self::from(io::ErrorKind::InvalidData),
            UnsupportedCompression(_) => Self::from(io::ErrorKind::Unsupported),
            MissingCompressor => Self::from(io::ErrorKind::InvalidData),
            FileNotFound => Self::from(io::ErrorKind::NotFound),
            Unreachable
            | Deku(_)
            | UnexpectedInode
            | UnsupportedInode
            | CorruptedOrInvalidSquashfs
            | InvalidCompressionOption
            | InvalidFilePath
            | UndefineFileName
            | DuplicatedFileName
            | InvalidIdTable
            | UnsupportedSquashfsVersion(_, _)
            | TryReserveError(_)
            | NumericConversion(_)
            | SystemTime(_)
            | MutexPoisoned
            | IdNotFoundInTable
            | InternalState(_)
            | CompressionInit(_) => Self::from(io::ErrorKind::InvalidData),
        }
    }
}
