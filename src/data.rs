use std::io::Write;

use tracing::instrument;

use crate::compressor::{compress, CompressionOptions, Compressor};

#[derive(Debug)]
pub(crate) struct DataWriter {
    compressor: Compressor,
    compression_options: Option<CompressionOptions>,
    pub(crate) data_bytes: Vec<u8>,
}

impl DataWriter {
    #[instrument(skip_all)]
    pub fn new(compressor: Compressor, compression_options: Option<CompressionOptions>) -> Self {
        Self {
            compressor,
            compression_options,
            data_bytes: vec![],
        }
    }

    // TODO: support fragments
    pub(crate) fn add_bytes(&mut self, bytes: &[u8]) -> (u32, Vec<u32>) {
        // TODO: use const
        let chunks = bytes.chunks(0x20000);

        // only have one chunk, use fragment
        //if chunks.len() == 1 {
        //    todo!();
        //    self.fragment_bytes.append(&mut chunks[0].to_vec());
        //}

        let blocks_start = self.data_bytes.len();
        let mut block_sizes = vec![];
        for chunk in chunks {
            let cb = compress(chunk.to_vec(), self.compressor, &self.compression_options).unwrap();
            block_sizes.push(cb.len() as u32);
            self.data_bytes.write_all(&cb).unwrap();
        }

        (blocks_start as u32, block_sizes)
    }
}
