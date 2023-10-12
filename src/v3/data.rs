//! File Data

use std::io::{Read, Seek, Write};

use deku::prelude::*;
use tracing::instrument;

use crate::bufread::WriteSeek;
use crate::compressor::{CompressionAction, FilesystemCompressor};
use crate::error::BackhandError;
use crate::v3::filesystem::reader::SquashfsRawData;
use crate::v3::fragment::Fragment;

// bitflag for data size field in inode for signifying that the data is uncompressed
const DATA_STORED_UNCOMPRESSED: u32 = 1 << 24;

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(
    ctx = "endian: deku::ctx::Endian, order: deku::ctx::Order",
    endian = "endian",
    bit_order = "order"
)]
pub struct DataSize(u32);
impl DataSize {
    pub fn new(size: u32, uncompressed: bool) -> Self {
        let mut value: u32 = size;
        if value > DATA_STORED_UNCOMPRESSED {
            panic!("value is too big");
        }
        if uncompressed {
            value |= DATA_STORED_UNCOMPRESSED;
        }
        Self(value)
    }

    pub fn new_compressed(size: u32) -> Self {
        Self::new(size, false)
    }

    pub fn new_uncompressed(size: u32) -> Self {
        Self::new(size, true)
    }

    pub fn uncompressed(&self) -> bool {
        self.0 & DATA_STORED_UNCOMPRESSED != 0
    }

    pub fn set_uncompressed(&mut self) {
        self.0 |= DATA_STORED_UNCOMPRESSED
    }

    pub fn set_compressed(&mut self) {
        self.0 &= !DATA_STORED_UNCOMPRESSED
    }

    pub fn size(&self) -> u32 {
        self.0 & !DATA_STORED_UNCOMPRESSED
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    // Only Data was added
    Data {
        blocks_start: u32,
        block_sizes: Vec<DataSize>,
    },
    // Only Fragment was added
    Fragment {
        frag_index: u32,
        block_offset: u32,
    },
}

struct DataWriterChunkReader<R: std::io::Read> {
    chunk: Vec<u8>,
    file_len: usize,
    reader: R,
}
impl<R: std::io::Read> DataWriterChunkReader<R> {
    pub fn read_chunk(&mut self) -> std::io::Result<&[u8]> {
        use std::io::ErrorKind;
        let mut buf: &mut [u8] = &mut self.chunk;
        let mut read_len = 0;
        while !buf.is_empty() {
            match self.reader.read(buf) {
                Ok(0) => break,
                Ok(n) => {
                    read_len += n;
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        self.file_len += read_len;
        Ok(&self.chunk[..read_len])
    }
}

pub(crate) struct DataWriter<'a> {
    kind: &'a dyn CompressionAction,
    block_size: u32,
    fs_compressor: FilesystemCompressor,
    /// Un-written fragment_bytes
    pub(crate) fragment_bytes: Vec<u8>,
    pub(crate) fragment_table: Vec<Fragment>,
}

impl<'a> DataWriter<'a> {
    #[instrument(skip_all)]
    pub fn new(
        kind: &'a dyn CompressionAction,
        fs_compressor: FilesystemCompressor,
        block_size: u32,
    ) -> Self {
        Self {
            kind,
            block_size,
            fs_compressor,
            fragment_bytes: Vec::with_capacity(block_size as usize),
            fragment_table: vec![],
        }
    }

    /// Add to data writer, either a pre-compressed Data or Fragment
    // TODO: support tail-end fragments (off by default in squashfs-tools/mksquashfs)
    pub(crate) fn just_copy_it<W: WriteSeek>(
        &mut self,
        mut reader: SquashfsRawData,
        writer: &mut W,
    ) -> Result<(usize, Added), BackhandError> {
        //just clone it, because block sizes where never modified, just copy it
        let mut block_sizes = reader.file.basic.block_sizes.clone();
        let mut read_buf = vec![];
        let mut decompress_buf = vec![];

        // if the first block is not full (fragment), store only a fragment
        // otherwise processed to store blocks
        let blocks_start = writer.stream_position()? as u32;
        let first_block = match reader.next_block(&mut read_buf) {
            Some(Ok(first_block)) => first_block,
            Some(Err(x)) => return Err(x),
            None => {
                return Ok((
                    0,
                    Added::Data {
                        blocks_start,
                        block_sizes,
                    },
                ))
            }
        };
        if first_block.fragment {
            reader.decompress(first_block, &mut read_buf, &mut decompress_buf)?;
            // if this doesn't fit in the current fragment bytes
            // compress the current fragment bytes and add to data_bytes
            if (decompress_buf.len() + self.fragment_bytes.len()) > self.block_size as usize {
                self.finalize(writer)?;
            }
            // add to fragment bytes
            let frag_index = self.fragment_table.len() as u32;
            let block_offset = self.fragment_bytes.len() as u32;
            self.fragment_bytes.write_all(&decompress_buf)?;

            return Ok((
                decompress_buf.len(),
                Added::Fragment {
                    frag_index,
                    block_offset,
                },
            ));
        } else {
            //if is a block, just copy it
            writer.write_all(&read_buf)?;
        }
        while let Some(block) = reader.next_block(&mut read_buf) {
            let block = block?;
            if block.fragment {
                reader.decompress(block, &mut read_buf, &mut decompress_buf)?;
                // TODO: support tail-end fragments, for now just treat it like a block
                let cb =
                    self.kind
                        .compress(&decompress_buf, self.fs_compressor, self.block_size)?;
                // compression didn't reduce size
                if cb.len() > decompress_buf.len() {
                    // store uncompressed
                    block_sizes.push(DataSize::new_uncompressed(decompress_buf.len() as u32));
                    writer.write_all(&decompress_buf)?;
                } else {
                    // store compressed
                    block_sizes.push(DataSize::new_compressed(cb.len() as u32));
                    writer.write_all(&cb)?;
                }
            } else {
                //if is a block, just copy it
                writer.write_all(&read_buf)?;
            }
        }
        let file_size = reader.file.basic.file_size as usize;
        Ok((
            file_size,
            Added::Data {
                blocks_start,
                block_sizes,
            },
        ))
    }

    /// Add to data writer, either a Data or Fragment
    // TODO: support tail-end fragments (off by default in squashfs-tools/mksquashfs)
    pub(crate) fn add_bytes<W: WriteSeek>(
        &mut self,
        reader: impl Read,
        writer: &mut W,
    ) -> Result<(usize, Added), BackhandError> {
        let mut chunk_reader = DataWriterChunkReader {
            chunk: vec![0u8; self.block_size as usize],
            file_len: 0,
            reader,
        };
        let mut chunk = chunk_reader.read_chunk()?;

        // chunk size not exactly the size of the block
        if chunk.len() != self.block_size as usize {
            // if this doesn't fit in the current fragment bytes
            // compress the current fragment bytes and add to data_bytes
            if (chunk.len() + self.fragment_bytes.len()) > self.block_size as usize {
                self.finalize(writer)?;
            }

            // add to fragment bytes
            let frag_index = self.fragment_table.len() as u32;
            let block_offset = self.fragment_bytes.len() as u32;
            self.fragment_bytes.write_all(chunk)?;

            Ok((
                chunk_reader.file_len,
                Added::Fragment {
                    frag_index,
                    block_offset,
                },
            ))
        } else {
            // Add to data bytes
            let blocks_start = writer.stream_position()? as u32;
            let mut block_sizes = vec![];
            while !chunk.is_empty() {
                let cb = self
                    .kind
                    .compress(chunk, self.fs_compressor, self.block_size)?;

                // compression didn't reduce size
                if cb.len() > chunk.len() {
                    // store uncompressed
                    block_sizes.push(DataSize::new_uncompressed(chunk.len() as u32));
                    writer.write_all(chunk)?;
                } else {
                    // store compressed
                    block_sizes.push(DataSize::new_compressed(cb.len() as u32));
                    writer.write_all(&cb)?;
                }
                chunk = chunk_reader.read_chunk()?;
            }

            Ok((
                chunk_reader.file_len,
                Added::Data {
                    blocks_start,
                    block_sizes,
                },
            ))
        }
    }

    /// Compress the fragments that were under length, write to data, add to fragment table, clear
    /// current fragment_bytes
    pub fn finalize<W: Write + Seek>(&mut self, writer: &mut W) -> Result<(), BackhandError> {
        let start = writer.stream_position()?;
        let cb = self
            .kind
            .compress(&self.fragment_bytes, self.fs_compressor, self.block_size)?;

        // compression didn't reduce size
        let size = if cb.len() > self.fragment_bytes.len() {
            // store uncompressed
            writer.write_all(&self.fragment_bytes)?;
            DataSize::new_uncompressed(self.fragment_bytes.len() as u32)
        } else {
            // store compressed
            writer.write_all(&cb)?;
            DataSize::new_compressed(cb.len() as u32)
        };
        self.fragment_table.push(Fragment::new(start, size, 0));
        self.fragment_bytes.clear();
        Ok(())
    }
}
