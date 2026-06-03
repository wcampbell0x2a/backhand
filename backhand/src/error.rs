use std::collections::TryReserveError;
use std::{io, string};

use thiserror::Error;

/// Errors produced by backhand operations
#[derive(Error, Debug)]
pub enum BackhandError {
    /// Standard I/O error
    #[error("std io error: {0}")]
    StdIo(#[from] io::Error),

    /// Deku parsing/writing error
    #[error("deku error: {0:?}")]
    Deku(#[from] deku::DekuError),

    /// UTF-8 string conversion error
    #[error("string error: {0:?}")]
    StringUtf8(#[from] string::FromUtf8Error),

    /// UTF-8 str conversion error
    #[error("string error: {0:?}")]
    StrUtf8(#[from] core::str::Utf8Error),

    /// Compression type not supported
    #[error("unsupported compression: {0:?}")]
    UnsupportedCompression(String),

    /// No compressor set for v4 filesystem
    #[error("missing compressor for v4 filesystem")]
    MissingCompressor,

    /// File not found in filesystem
    #[error("file not found")]
    FileNotFound,

    /// Code path thought to be unreachable was reached
    #[error("branch was thought to be unreachable")]
    Unreachable,

    /// Inode type unexpected in this position
    #[error("inode was unexpected in this position")]
    UnexpectedInode,

    /// Inode type not yet supported
    #[error("unsupported inode, please fill github issue to add support")]
    UnsupportedInode,

    /// Image data is corrupted or invalid
    #[error("corrupted or invalid squashfs image")]
    CorruptedOrInvalidSquashfs,

    /// Invalid compression options in image
    #[error("invalid squashfs compression options")]
    InvalidCompressionOption,

    /// Invalid file path in image
    #[error("Invalid file path in the squashfs image")]
    InvalidFilePath,

    /// File entry has no name
    #[error("file inside squashfs image have no name")]
    UndefineFileName,

    /// Duplicate file path in image
    #[error("file duplicated in squashfs image")]
    DuplicatedFileName,

    /// Memory allocation failure
    #[error("allocator try_reserve error")]
    TryReserveError(#[from] TryReserveError),

    /// ID lookup table is invalid
    #[error("invalid id_table for node")]
    InvalidIdTable,

    /// SquashFS version not supported
    #[error("unsupported squashfs version {0}.{1}")]
    UnsupportedSquashfsVersion(u16, u16),

    /// Numeric type conversion failed
    #[error("numeric conversion failed: {0}")]
    NumericConversion(String),

    /// System time error
    #[error("system time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),

    /// Mutex lock was poisoned
    #[error("mutex lock poisoned")]
    MutexPoisoned,

    /// UID/GID not found in ID table
    #[error("uid/gid not found in id table")]
    IdNotFoundInTable,

    /// Internal state inconsistency
    #[error("internal state error: {0}")]
    InternalState(String),

    /// Compression initialization failed
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
