//! File Data

use std::io::{Read, Seek, Write};

use tracing::instrument;

use crate::compressor::{compress, CompressionOptions, Compressor};
use crate::fragment::Fragment;

// bitflag for data size field in inode for signifying that the data is uncompressed
pub(crate) const DATA_STORED_UNCOMPRESSED: u32 = 1 << 24;

#[derive(Debug, Clone)]
pub(crate) enum Added {
    // Only Data was added
    Data {
        blocks_start: u32,
        block_sizes: Vec<u32>,
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
                },
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {},
                Err(e) => return Err(e),
            }
        }
        self.file_len += read_len;
        Ok(&self.chunk[..read_len])
    }
}

#[derive(Debug)]
pub(crate) struct DataWriter {
    block_size: u32,
    compressor: Compressor,
    compression_options: Option<CompressionOptions>,
    /// Un-written fragment_bytes
    pub(crate) fragment_bytes: Vec<u8>,
    pub(crate) fragment_table: Vec<Fragment>,
}

impl DataWriter {
    #[instrument(skip_all)]
    pub fn new(
        compressor: Compressor,
        compression_options: Option<CompressionOptions>,
        block_size: u32,
    ) -> Self {
        Self {
            block_size,
            compressor,
            compression_options,
            fragment_bytes: Vec::with_capacity(block_size as usize),
            fragment_table: vec![],
        }
    }

    /// Add to data writer, either a Data or Fragment
    // TODO: support tail-end fragments (off by default in squashfs-tools/mksquashfs)
    pub(crate) fn add_bytes<W: Write + Seek>(
        &mut self,
        reader: impl Read,
        writer: &mut W,
    ) -> (usize, Added) {
        let mut chunk_reader = DataWriterChunkReader {
            chunk: vec![0u8; self.block_size as usize],
            file_len: 0,
            reader,
        };
        //TODO error
        let mut chunk = chunk_reader.read_chunk().unwrap();

        // only one chunks, and not exactly the size of the block
        if chunk.len() != self.block_size as usize {
            // if this doesn't fit in the current fragment bytes, compress and add to data_bytes
            if (chunk.len() + self.fragment_bytes.len()) > self.block_size as usize {
                // TODO: don't always compress?
                let start = writer.stream_position().unwrap(/*TODO*/);
                let cb = compress(
                    &self.fragment_bytes,
                    self.compressor,
                    &self.compression_options,
                    self.block_size,
                )
                .unwrap();
                let size = cb.len() as u32;
                let frag = Fragment {
                    start,
                    size,
                    unused: 0,
                };
                self.fragment_table.push(frag);
                writer.write_all(&cb).unwrap();

                self.fragment_bytes = vec![];
            }

            // add to fragment bytes
            let frag_index = self.fragment_table.len() as u32;
            let block_offset = self.fragment_bytes.len() as u32;
            assert!(self.fragment_bytes.len() < 10_000_000);
            self.fragment_bytes.write_all(chunk).unwrap();

            (
                chunk_reader.file_len,
                Added::Fragment {
                    frag_index,
                    block_offset,
                },
            )
        } else {
            // Add to data bytes
            let blocks_start = writer.stream_position().unwrap(/*TODO*/) as u32;
            let mut block_sizes = vec![];
            loop {
                if chunk.is_empty() {
                    break;
                }
                let cb = compress(
                    chunk,
                    self.compressor,
                    &self.compression_options,
                    self.block_size,
                )
                .unwrap();

                // compression didn't reduce size
                if cb.len() > chunk.len() {
                    // store uncompressed
                    block_sizes.push(DATA_STORED_UNCOMPRESSED | chunk.len() as u32);
                    writer.write_all(chunk).unwrap();
                } else {
                    // store compressed
                    block_sizes.push(cb.len() as u32);
                    writer.write_all(&cb).unwrap();
                }
                chunk = chunk_reader.read_chunk().unwrap();
            }

            (
                chunk_reader.file_len,
                Added::Data {
                    blocks_start,
                    block_sizes,
                },
            )
        }
    }

    /// Compress the fragments that were under length, add to fragment table
    pub fn finalize<W: Write + Seek>(&mut self, writer: &mut W) {
        let start = writer.stream_position().unwrap(/*TODO*/);
        let cb = compress(
            &self.fragment_bytes,
            self.compressor,
            &self.compression_options,
            self.block_size,
        )
        .unwrap();
        let size = cb.len() as u32;
        self.fragment_table.push(Fragment {
            start,
            size,
            unused: 0,
        });
        writer.write_all(&cb).unwrap();
    }
}
