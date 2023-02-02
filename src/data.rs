//! File Data

use std::io::{Read, Write};

use tracing::instrument;

use crate::compressor::{compress, CompressionOptions, Compressor};
use crate::fragment::Fragment;

// bitflag for data size field in inode for signifying that the data is uncompressed
pub(crate) const DATA_STORED_UNCOMPRESSED: u32 = 1 << 24;

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
    data_start: u32,
    compressor: Compressor,
    compression_options: Option<CompressionOptions>,
    pub(crate) data_bytes: Vec<u8>,
    /// Un-written fragment_bytes
    pub(crate) fragment_bytes: Vec<u8>,
    pub(crate) fragment_table: Vec<Fragment>,
}

impl DataWriter {
    #[instrument(skip_all)]
    pub fn new(
        compressor: Compressor,
        compression_options: Option<CompressionOptions>,
        data_start: u32,
        block_size: u32,
    ) -> Self {
        Self {
            block_size,
            data_start,
            compressor,
            compression_options,
            data_bytes: vec![],
            fragment_bytes: Vec::with_capacity(block_size as usize),
            fragment_table: vec![],
        }
    }

    /// Add to data writer, either a Data or Fragment
    // TODO: support tail-end fragments (off by default in squashfs-tools/mksquashfs)
    pub(crate) fn add_bytes(&mut self, reader: impl Read) -> (usize, Added) {
        let mut chunk_reader = DataWriterChunkReader {
            chunk: vec![0u8; self.block_size as usize],
            file_len: 0,
            reader,
        };
        //TODO error
        let mut chunk = chunk_reader.read_chunk().unwrap();

        // chunk size not exactly the size of the block
        if chunk.len() != self.block_size as usize {
            // if this doesn't fit in the current fragment bytes
            // compress the current fragment bytes and add to data_bytes
            if (chunk.len() + self.fragment_bytes.len()) > self.block_size as usize {
                self.finalize();
            }

            // add to fragment bytes
            let frag_index = self.fragment_table.len() as u32;
            let block_offset = self.fragment_bytes.len() as u32;
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
            let blocks_start = self.data_bytes.len() as u32 + self.data_start;
            let mut block_sizes = vec![];
            while !chunk.is_empty() {
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
                    self.data_bytes.write_all(chunk).unwrap();
                } else {
                    // store compressed
                    block_sizes.push(cb.len() as u32);
                    self.data_bytes.write_all(&cb).unwrap();
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

    /// Compress the fragments that were under length, write to data, add to fragment table, clear
    /// current fragment_bytes
    pub fn finalize(&mut self) {
        let start = self.data_bytes.len() as u64 + self.data_start as u64;
        let cb = compress(
            &self.fragment_bytes,
            self.compressor,
            &self.compression_options,
            self.block_size,
        )
        .unwrap();

        // compression didn't reduce size
        let size = if cb.len() > self.fragment_bytes.len() {
            // store uncompressed
            self.data_bytes.write_all(&self.fragment_bytes).unwrap();
            println!("u: {:02x?} {:02x?}", cb.len(), self.fragment_bytes.len());
            DATA_STORED_UNCOMPRESSED | self.fragment_bytes.len() as u32
        } else {
            // store compressed
            println!("c: {:02x?} {:02x?}", cb.len(), self.fragment_bytes.len());
            self.data_bytes.write_all(&cb).unwrap();
            cb.len() as u32
        };
        self.fragment_table.push(Fragment {
            start,
            size,
            unused: 0,
        });
        self.fragment_bytes.clear();
    }
}
