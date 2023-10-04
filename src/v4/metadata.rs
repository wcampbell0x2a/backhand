use std::collections::VecDeque;
use std::io::{self, Cursor, Read, Seek, Write};

use deku::bitvec::BitVec;
use deku::prelude::*;
use tracing::{instrument, trace};

use crate::compressor::FilesystemCompressor;
use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::v4::squashfs::SuperBlock;

pub const METADATA_MAXSIZE: usize = 0x2000;

const METDATA_UNCOMPRESSED: u16 = 1 << 15;

pub(crate) struct MetadataWriter {
    compressor: FilesystemCompressor,
    block_size: u32,
    /// Offset from the beginning of the metadata block last written
    pub(crate) metadata_start: u32,
    // All current bytes that are uncompressed
    pub(crate) uncompressed_bytes: VecDeque<u8>,
    // All current bytes that are compressed or uncompressed
    pub(crate) final_bytes: Vec<(bool, Vec<u8>)>,
    pub kind: Kind,
}

impl MetadataWriter {
    #[instrument(skip_all)]
    pub fn new(compressor: FilesystemCompressor, block_size: u32, kind: Kind) -> Self {
        Self {
            compressor,
            block_size,
            metadata_start: 0,
            uncompressed_bytes: VecDeque::new(),
            final_bytes: vec![],
            kind,
        }
    }

    #[instrument(skip_all)]
    fn add_block(&mut self) -> io::Result<()> {
        // uncompress data that will create the metablock
        let uncompressed_len = self.uncompressed_bytes.len().min(METADATA_MAXSIZE);
        if uncompressed_len == 0 {
            // nothing to add
            return Ok(());
        }

        if self.uncompressed_bytes.as_slices().0.len() < uncompressed_len {
            self.uncompressed_bytes.make_contiguous();
        }
        let uncompressed = &self.uncompressed_bytes.as_slices().0[0..uncompressed_len];

        trace!("time to compress");
        // "Write" the to the saved metablock
        let compressed =
            self.kind
                .inner
                .compressor
                .compress(uncompressed, self.compressor, self.block_size)?;

        // Remove the data consumed, if the uncompressed data is smalled, use it.
        let (compressed, metadata) = if compressed.len() > uncompressed_len {
            let uncompressed = self.uncompressed_bytes.drain(0..uncompressed_len).collect();
            (false, uncompressed)
        } else {
            self.uncompressed_bytes.drain(0..uncompressed_len);
            (true, compressed)
        };

        // Metadata len + bytes + last metadata_start
        self.metadata_start += 2 + metadata.len() as u32;
        trace!("new metadata start: {:#02x?}", self.metadata_start);
        self.final_bytes.push((compressed, metadata));

        trace!("LEN: {:02x?}", self.uncompressed_bytes.len());
        Ok(())
    }

    #[instrument(skip_all)]
    pub fn finalize<W: Write + Seek>(&mut self, out: &mut W) -> Result<(), BackhandError> {
        //add any remaining data
        while !self.uncompressed_bytes.is_empty() {
            self.add_block()?;
        }

        // write all the metadata blocks
        for (compressed, cb) in &self.final_bytes {
            trace!("len: {:02x?}", cb.len());
            //trace!("total: {:02x?}", out.len());
            let mut bv = BitVec::new();
            // if uncompressed, set the highest bit of len
            let len = cb.len() as u16 | if *compressed { 0 } else { 1 << (u16::BITS - 1) };
            len.write(&mut bv, self.kind.inner.data_endian)?;
            out.write_all(bv.as_raw_slice())?;
            out.write_all(cb)?;
        }

        Ok(())
    }
}

impl Write for MetadataWriter {
    #[instrument(skip_all)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // add all of buf into uncompressed
        self.uncompressed_bytes.write_all(buf)?;

        // if there is too much uncompressed data, create a new metadata block
        while self.uncompressed_bytes.len() >= METADATA_MAXSIZE {
            self.add_block()?;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

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
            .decompress(&buf, &mut out, superblock.compressor)?;
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
