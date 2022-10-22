use std::{io, string};

use thiserror::Error;

use crate::Compressor;

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

    #[error("squashfs field not initialized")]
    FieldNotInitialized,
}
