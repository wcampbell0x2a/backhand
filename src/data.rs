//! File Data

use std::io::Write;

use tracing::instrument;

use crate::compressor::{compress, CompressionOptions, Compressor};
use crate::fragment::Fragment;

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
            fragment_bytes: vec![],
            fragment_table: vec![],
        }
    }

    /// Add to data writer, either a Data or Fragment
    // TODO: support tail-end fragments (off by default in squashfs-tools/mksquashfs)
    pub(crate) fn add_bytes(&mut self, bytes: &[u8]) -> Added {
        let mut chunks = bytes.chunks(self.block_size as usize);

        // only one chunks, and not exactly the size of the block
        if chunks.len() == 1 && bytes.len() != self.block_size as usize {
            let chunk = chunks.next().unwrap();

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

                self.fragment_bytes = vec![];
            }

            // add to fragment bytes
            let frag_index = self.fragment_table.len() as u32;
            let block_offset = self.fragment_bytes.len() as u32;
            self.fragment_bytes.write_all(chunk).unwrap();

            Added::Fragment {
                frag_index,
                block_offset,
            }
        } else {
            // Add to data bytes
            let blocks_start = self.data_bytes.len() as u32 + self.data_start;
            let mut block_sizes = vec![];
            for chunk in chunks {
                let cb = compress(chunk, self.compressor, &self.compression_options).unwrap();
                block_sizes.push(cb.len() as u32);
                self.data_bytes.write_all(&cb).unwrap();
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
