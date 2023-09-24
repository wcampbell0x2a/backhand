use std::io::{self, Cursor, Read, Seek, Write};

use deku::prelude::*;
use tracing::{instrument, trace};

use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::v3::squashfs::SuperBlock;

pub const METADATA_MAXSIZE: usize = 0x2000;

const METDATA_UNCOMPRESSED: u16 = 1 << 15;

#[instrument(skip_all)]
pub fn read_block<R: Read + ?Sized>(
    reader: &mut R,
    superblock: &SuperBlock,
    kind: &Kind,
) -> Result<Vec<u8>, BackhandError> {
    let buf: &mut [u8] = &mut [0u8; 2];
    reader.read_exact(buf)?;

    trace!("{:02x?}", buf);
    let mut cursor = Cursor::new(buf);
    let mut deku_reader = Reader::new(&mut cursor);
    let metadata_len = u16::from_reader_with_ctx(&mut deku_reader, kind.inner.data_endian)?;

    let byte_len = len(metadata_len);
    tracing::trace!("len: 0x{:02x?}", byte_len);
    let mut buf = vec![0u8; byte_len as usize];
    reader.read_exact(&mut buf)?;

    let bytes = if is_compressed(metadata_len) {
        tracing::trace!("compressed");
        let mut out = Vec::with_capacity(8 * 1024);
        kind.inner
            .compressor
            .decompress(&buf, &mut out, crate::compression::Compressor::Gzip)?;
        out
    } else {
        tracing::trace!("uncompressed");
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

pub fn set_if_uncompressed(len: u16) -> u16 {
    len | METDATA_UNCOMPRESSED
}
