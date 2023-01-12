//! File Data

use std::io::{SeekFrom, Write};

use tracing::instrument;

use crate::compressor::{compress, CompressionOptions, Compressor};
use crate::fragment::Fragment;
use crate::reader::ReadSeek;

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
    pub(crate) fn add_bytes(&mut self, reader: &mut dyn ReadSeek) -> Added {
        //go to the end, calculating the file len
        let len = reader.seek(SeekFrom::End(0)).unwrap();
        //go back to the start
        reader.seek(SeekFrom::Start(0)).unwrap();

        // only one chunks, and not exactly the size of the block
        if len < self.block_size.into() {
            let mut chunk = vec![0u8; len as usize];
            reader.read_exact(&mut chunk).unwrap();

            // if this doesn't fit in the current fragment bytes, compress and add to data_bytes
            if (chunk.len() + self.fragment_bytes.len()) > self.block_size as usize {
                // TODO: don't always compress?
                let start = self.data_bytes.len() as u64 + self.data_start as u64;
                let cb = compress(
                    &self.fragment_bytes,
                    self.compressor,
                    &self.compression_options,
                )
                .unwrap();
                let size = cb.len() as u32;
                let frag = Fragment {
                    start,
                    size,
                    unused: 0,
                };
                self.fragment_table.push(frag);
                self.data_bytes.write_all(&cb).unwrap();

                self.fragment_bytes = Vec::with_capacity(self.block_size as usize);
            }

            // add to fragment bytes
            let frag_index = self.fragment_table.len() as u32;
            let block_offset = self.fragment_bytes.len() as u32;
            self.fragment_bytes.write_all(&chunk).unwrap();

            Added::Fragment {
                frag_index,
                block_offset,
            }
        } else {
            // Add to data bytes
            let mut block_sizes = vec![];
            let blocks_start = self.data_bytes.len() as u32 + self.data_start;
            let mut add_data = |bytes: &[u8]| {
                let cb = compress(bytes, self.compressor, &self.compression_options).unwrap();
                block_sizes.push(cb.len() as u32);
                self.data_bytes.write_all(&cb).unwrap();
            };

            //the chunk buffer
            let mut chunk_buf = vec![0u8; self.block_size as usize];
            //the number of full chunks
            let chunk_number = len as usize / self.block_size as usize;
            //the len of the last non full chunk
            let chunk_last_len = len as usize % self.block_size as usize;

            //add the full chunks
            for _ in 0..chunk_number {
                reader.read_exact(&mut chunk_buf).unwrap();
                add_data(&chunk_buf);
            }
            //add the last (if any) chunk, that may not be full
            if chunk_last_len != 0 {
                reader
                    .read_exact(&mut chunk_buf[0..chunk_last_len])
                    .unwrap();
                add_data(&chunk_buf[0..chunk_last_len]);
            }

            Added::Data {
                blocks_start,
                block_sizes,
            }
        }
    }

    /// Compress the fragments that were under length, add to fragment table
    pub fn finalize(&mut self) {
        let start = self.data_bytes.len() as u64 + self.data_start as u64;
        let cb = compress(
            &self.fragment_bytes,
            self.compressor,
            &self.compression_options,
        )
        .unwrap();
        let size = cb.len() as u32;
        self.fragment_table.push(Fragment {
            start,
            size,
            unused: 0,
        });
        self.data_bytes.write_all(&cb).unwrap();
    }
}
