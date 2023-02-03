use std::io::{self, Read, Seek, Write};

use tracing::{instrument, trace};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::error::SquashfsError;
use crate::squashfs::SuperBlock;

pub const METADATA_MAXSIZE: usize = 0x2000;

const METDATA_UNCOMPRESSED: u16 = 1 << 15;

#[derive(Debug)]
pub(crate) struct MetadataWriter {
    compressor: Compressor,
    compression_options: Option<CompressionOptions>,
    block_size: u32,
    /// Offset from the beginning of the metadata block last written
    pub(crate) metadata_start: u32,
    // All current bytes that are uncompressed
    pub(crate) uncompressed_bytes: Vec<u8>,
    // All current bytes that are compressed
    pub(crate) compressed_bytes: Vec<Vec<u8>>,
}

impl MetadataWriter {
    #[instrument(skip_all)]
    pub fn new(
        compressor: Compressor,
        compression_options: Option<CompressionOptions>,
        block_size: u32,
    ) -> Self {
        Self {
            compressor,
            compression_options,
            block_size,
            metadata_start: 0,
            uncompressed_bytes: vec![],
            compressed_bytes: vec![],
        }
    }

    #[instrument(skip_all)]
    pub fn finalize<W: Write + Seek>(&mut self, out: &mut W) -> Result<(), SquashfsError> {
        for cb in &self.compressed_bytes {
            trace!("len: {:02x?}", cb.len());
            //trace!("total: {:02x?}", out.len());
            out.write_all(&(cb.len() as u16).to_le_bytes())?;
            out.write_all(cb)?;
        }

        let b = compressor::compress(
            &self.uncompressed_bytes,
            self.compressor,
            &self.compression_options,
            self.block_size,
        )?;

        trace!("len: {:02x?}", b.len());
        //trace!("total: {:02x?}", out.len());
        out.write_all(&(b.len() as u16).to_le_bytes())?;
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
            let b = compressor::compress(
                &self.uncompressed_bytes[..METADATA_MAXSIZE],
                self.compressor,
                &self.compression_options,
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
    superblock: &SuperBlock,
) -> Result<Vec<u8>, SquashfsError> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    let metadata_len = u16::from_le_bytes(buf);

    let byte_len = len(metadata_len);
    tracing::trace!("len: 0x{:02x?}", byte_len);
    let mut buf = vec![0u8; byte_len as usize];
    reader.read_exact(&mut buf)?;

    let bytes = if is_compressed(metadata_len) {
        tracing::trace!("compressed");
        compressor::decompress(buf, superblock.compressor)?
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
