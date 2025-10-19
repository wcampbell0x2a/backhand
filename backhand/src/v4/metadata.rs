use no_std_io2::io::{Read, Seek, Write};
use std::collections::VecDeque;
use std::io;

use deku::prelude::*;
use tracing::trace;

use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::v4::filesystem::writer::FilesystemCompressor;
use crate::v4::squashfs::SuperBlock;

pub const METADATA_MAXSIZE: usize = 0x2000;

const METDATA_UNCOMPRESSED: u16 = 1 << 15;

pub(crate) struct MetadataWriter<'a> {
    compression_action: &'a (dyn crate::traits::CompressionAction<
        Compressor = super::compressor::Compressor,
        FilesystemCompressor = super::filesystem::writer::FilesystemCompressor,
        SuperBlock = super::squashfs::SuperBlock,
        Error = crate::BackhandError,
    > + Send
             + Sync),
    compressor: FilesystemCompressor,
    block_size: u32,
    data_endian: deku::ctx::Endian,
    /// Offset from the beginning of the metadata block last written
    pub(crate) metadata_start: u32,
    // All current bytes that are uncompressed
    pub(crate) uncompressed_bytes: VecDeque<u8>,
    // All current bytes that are compressed or uncompressed
    pub(crate) final_bytes: Vec<(bool, Vec<u8>)>,
}

impl<'a> MetadataWriter<'a> {
    pub fn new(
        compression_action: &'a (dyn crate::traits::CompressionAction<
            Compressor = super::compressor::Compressor,
            FilesystemCompressor = super::filesystem::writer::FilesystemCompressor,
            SuperBlock = super::squashfs::SuperBlock,
            Error = crate::BackhandError,
        > + Send
                 + Sync),
        compressor: FilesystemCompressor,
        block_size: u32,
        data_endian: deku::ctx::Endian,
    ) -> Self {
        Self {
            compression_action,
            compressor,
            block_size,
            data_endian,
            metadata_start: 0,
            uncompressed_bytes: VecDeque::new(),
            final_bytes: vec![],
        }
    }

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
            self.compression_action.compress(uncompressed, self.compressor, self.block_size)?;

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

    pub fn finalize<W: Write + Seek>(&mut self, mut out: W) -> Result<(), BackhandError> {
        //add any remaining data
        while !self.uncompressed_bytes.is_empty() {
            self.add_block()?;
        }

        // write all the metadata blocks
        for (compressed, compressed_bytes) in &self.final_bytes {
            trace!("len: {:02x?}", compressed_bytes.len());
            // if uncompressed, set the highest bit of len
            let len =
                compressed_bytes.len() as u16 | if *compressed { 0 } else { 1 << (u16::BITS - 1) };
            let mut writer = Writer::new(&mut out);
            len.to_writer(&mut writer, self.data_endian)?;
            out.write_all(compressed_bytes)?;
        }

        Ok(())
    }
}

impl Write for MetadataWriter<'_> {
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

pub fn read_block<R: Read + Seek>(
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

    let bytes = if is_compressed(metadata_len) {
        tracing::trace!("compressed");
        let mut out = Vec::with_capacity(8 * 1024);
        kind.inner.compressor.decompress(&buf, &mut out, Some(superblock.compressor.into()))?;
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
