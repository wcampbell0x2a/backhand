use std::collections::TryReserveError;
use std::{io, string};

#[cfg(feature = "error-strings")]
use thiserror::Error;

/// The unified error type for reading, creating, and modifying a SquashFS image
///
/// The variants are the same in every build. With the `error-strings` feature
/// off, `Display` and `Debug` print nothing, so a caller must match on the
/// variant to tell one fault from another.
#[cfg_attr(feature = "error-strings", derive(Error, Debug))]
pub enum BackhandError {
    #[cfg_attr(feature = "error-strings", error("std io error: {0}"))]
    StdIo(#[cfg_attr(feature = "error-strings", from)] io::Error),

    #[cfg_attr(feature = "error-strings", error("deku error: {0:?}"))]
    Deku(#[cfg_attr(feature = "error-strings", from)] deku::DekuError),

    #[cfg_attr(feature = "error-strings", error("string error: {0:?}"))]
    StringUtf8(#[cfg_attr(feature = "error-strings", from)] string::FromUtf8Error),

    #[cfg_attr(feature = "error-strings", error("string error: {0:?}"))]
    StrUtf8(#[cfg_attr(feature = "error-strings", from)] core::str::Utf8Error),

    #[cfg_attr(feature = "error-strings", error("unsupported compression: {0:?}"))]
    UnsupportedCompression(String),

    #[cfg_attr(feature = "error-strings", error("missing compressor for v4 filesystem"))]
    MissingCompressor,

    #[cfg_attr(feature = "error-strings", error("file not found"))]
    FileNotFound,

    #[cfg_attr(feature = "error-strings", error("branch was thought to be unreachable"))]
    Unreachable,

    #[cfg_attr(feature = "error-strings", error("inode was unexpected in this position"))]
    UnexpectedInode,

    #[cfg_attr(
        feature = "error-strings",
        error("unsupported inode, please fill github issue to add support")
    )]
    UnsupportedInode,

    #[cfg_attr(feature = "error-strings", error("corrupted or invalid squashfs image"))]
    CorruptedOrInvalidSquashfs,

    #[cfg_attr(feature = "error-strings", error("invalid squashfs compression options"))]
    InvalidCompressionOption,

    #[cfg_attr(feature = "error-strings", error("Invalid file path in the squashfs image"))]
    InvalidFilePath,

    #[cfg_attr(feature = "error-strings", error("file inside squashfs image have no name"))]
    UndefineFileName,

    #[cfg_attr(feature = "error-strings", error("file duplicated in squashfs image"))]
    DuplicatedFileName,

    #[cfg_attr(feature = "error-strings", error("allocator try_reserve error"))]
    TryReserveError(#[cfg_attr(feature = "error-strings", from)] TryReserveError),

    #[cfg_attr(feature = "error-strings", error("invalid id_table for node"))]
    InvalidIdTable,

    #[cfg_attr(feature = "error-strings", error("unsupported squashfs version {0}.{1}"))]
    UnsupportedSquashfsVersion(u16, u16),

    #[cfg_attr(feature = "error-strings", error("numeric conversion failed: {0}"))]
    NumericConversion(String),

    #[cfg_attr(feature = "error-strings", error("system time error: {0}"))]
    SystemTime(#[cfg_attr(feature = "error-strings", from)] std::time::SystemTimeError),

    #[cfg_attr(feature = "error-strings", error("mutex lock poisoned"))]
    MutexPoisoned,

    #[cfg_attr(feature = "error-strings", error("uid/gid not found in id table"))]
    IdNotFoundInTable,

    #[cfg_attr(feature = "error-strings", error("internal state error: {0}"))]
    InternalState(String),

    #[cfg_attr(feature = "error-strings", error("compression initialization failed: {0}"))]
    CompressionInit(String),
}

#[cfg(not(feature = "error-strings"))]
mod stripped {
    use super::BackhandError;
    use core::fmt;
    use std::collections::TryReserveError;
    use std::{io, string};

    impl fmt::Display for BackhandError {
        fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
            Ok(())
        }
    }

    impl fmt::Debug for BackhandError {
        fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
            Ok(())
        }
    }

    impl std::error::Error for BackhandError {}

    impl From<io::Error> for BackhandError {
        fn from(value: io::Error) -> Self {
            Self::StdIo(value)
        }
    }

    impl From<deku::DekuError> for BackhandError {
        fn from(value: deku::DekuError) -> Self {
            Self::Deku(value)
        }
    }

    impl From<string::FromUtf8Error> for BackhandError {
        fn from(value: string::FromUtf8Error) -> Self {
            Self::StringUtf8(value)
        }
    }

    impl From<core::str::Utf8Error> for BackhandError {
        fn from(value: core::str::Utf8Error) -> Self {
            Self::StrUtf8(value)
        }
    }

    impl From<TryReserveError> for BackhandError {
        fn from(value: TryReserveError) -> Self {
            Self::TryReserveError(value)
        }
    }

    impl From<std::time::SystemTimeError> for BackhandError {
        fn from(value: std::time::SystemTimeError) -> Self {
            Self::SystemTime(value)
        }
    }
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
