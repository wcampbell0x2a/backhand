use std::io::{Read, Seek};

use deku::prelude::*;

use super::squashfs::SuperBlock;
use crate::error::BackhandError;
use crate::kinds::Kind;

pub const METADATA_MAXSIZE: usize = 0x2000;

const METDATA_UNCOMPRESSED: u16 = 1 << 15;

pub fn read_block<R: Read + Seek + ?Sized>(
    reader: &mut R,
    superblock: &SuperBlock,
    kind: &Kind,
) -> Result<Vec<u8>, BackhandError> {
    let mut deku_reader = Reader::new(&mut *reader);
    let metadata_len = u16::from_reader_with_ctx(&mut deku_reader, kind.inner.data_endian)?;

    let byte_len = len(metadata_len);
    tracing::trace!("len: 0x{:02x?}", byte_len);
    let mut buf = vec![0u8; byte_len as usize];
    reader.read_exact(&mut buf)?;

    let is_block_compressed = is_compressed(metadata_len);
    let is_superblock_uncompressed = superblock.inodes_uncompressed();
    let bytes = if is_block_compressed && !is_superblock_uncompressed {
        let mut out = Vec::with_capacity(8 * 1024);
        kind.inner.compressor.decompress(&buf, &mut out, super::compressor::Compressor::Gzip)?;
        out
    } else {
        tracing::trace!("uncompressed (superblock flag or block flag)");
        buf
    };

    tracing::trace!("uncompressed size: 0x{:02x?}", bytes.len());
    Ok(bytes)
}

/// Check is_compressed bit within raw `len`
pub fn is_compressed(len: u16) -> bool {
    len & METDATA_UNCOMPRESSED == 0
}

/// Get actual length of `data` following `len` from unedited `len`
pub fn len(len: u16) -> u16 {
    len & !(METDATA_UNCOMPRESSED)
}
