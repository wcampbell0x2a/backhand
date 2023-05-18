use std::io::{self, Read, Seek, Write};

use deku::bitvec::{BitVec, BitView};
use deku::prelude::*;
use tracing::{instrument, trace};

use crate::compression::Compressor;
use crate::error::BackhandError;
use crate::filesystem::writer::FilesystemCompressor;
use crate::kinds::Kind;
use crate::{SuperBlock, SuperBlockTrait};

pub const METADATA_MAXSIZE: usize = 0x2000;

const METDATA_UNCOMPRESSED: u16 = 1 << 15;

pub(crate) struct MetadataWriter {
    compressor: FilesystemCompressor,
    block_size: u32,
    /// Offset from the beginning of the metadata block last written
    pub(crate) metadata_start: u32,
    // All current bytes that are uncompressed
    pub(crate) uncompressed_bytes: Vec<u8>,
    // All current bytes that are compressed
    pub(crate) compressed_bytes: Vec<Vec<u8>>,
    pub kind: Kind,
}

impl MetadataWriter {
    #[instrument(skip_all)]
    pub fn new(compressor: FilesystemCompressor, block_size: u32, kind: Kind) -> Self {
        Self {
            compressor,
            block_size,
            metadata_start: 0,
            uncompressed_bytes: vec![],
            compressed_bytes: vec![],
            kind,
        }
    }

    #[instrument(skip_all)]
    pub fn finalize<W: Write + Seek>(&mut self, out: &mut W) -> Result<(), BackhandError> {
        for cb in &self.compressed_bytes {
            trace!("len: {:02x?}", cb.len());
            //trace!("total: {:02x?}", out.len());
            let mut bv = BitVec::new();
            (cb.len() as u16).write(&mut bv, self.kind.inner.data_endian)?;
            out.write_all(bv.as_raw_slice())?;
            out.write_all(cb)?;
        }

        let b = self.kind.inner.compressor.compress(
            &self.uncompressed_bytes,
            self.compressor,
            self.block_size,
        )?;

        trace!("len: {:02x?}", b.len());
        let mut bv = BitVec::new();
        (b.len() as u16).write(&mut bv, self.kind.inner.data_endian)?;
        out.write_all(bv.as_raw_slice())?;
        out.write_all(&b)?;
        Ok(())
    }
}

impl Write for MetadataWriter {
    #[instrument(skip_all)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // add all of buf into uncompressed
        self.uncompressed_bytes.write_all(buf)?;

        while self.uncompressed_bytes.len() >= METADATA_MAXSIZE {
            trace!("time to compress");
            // "Write" the to the saved metablock
            let b = self.kind.inner.compressor.compress(
                &self.uncompressed_bytes[..METADATA_MAXSIZE],
                self.compressor,
                self.block_size,
            )?;

            // Metadata len + bytes + last metadata_start
            self.metadata_start += 2 + b.len() as u32;
            trace!("new metadata start: {:#02x?}", self.metadata_start);
            self.uncompressed_bytes = self.uncompressed_bytes[METADATA_MAXSIZE..].to_vec();
            self.compressed_bytes.push(b);
        }
        trace!("LEN: {:02x?}", self.uncompressed_bytes.len());

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[instrument(skip_all)]
pub fn read_block<R: Read + ?Sized>(
    reader: &mut R,
    compressor: Compressor,
    kind: &Kind,
) -> Result<Vec<u8>, BackhandError> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;

    let bv = buf.view_bits::<deku::bitvec::Msb0>();
    trace!("{:02x?}", buf);
    let (_, metadata_len) = u16::read(bv, kind.inner.data_endian)?;

    let byte_len = len(metadata_len);
    tracing::trace!("len: 0x{:02x?}", byte_len);
    let mut buf = vec![0u8; byte_len as usize];
    reader.read_exact(&mut buf)?;

    let bytes = if is_compressed(metadata_len) {
        tracing::trace!("compressed");
        let mut out = Vec::with_capacity(8 * 1024);
        kind.inner
            .compressor
            .decompress(&buf, &mut out, compressor)?;
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
