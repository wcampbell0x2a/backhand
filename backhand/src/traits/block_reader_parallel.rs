//! Parallel file data block reading, shared by all SquashFS versions

use no_std_io2::io::Read;
use rayon::prelude::*;
use std::collections::VecDeque;
use std::io::{Seek, SeekFrom};
use std::sync::{Arc, Mutex};

use crate::error::BackhandError;
use crate::traits::block_reader::{
    BlockFragment, BlockIterator, BlockReaderVersion, RawDataBlock, decompress, read_raw_data,
};

/// How many blocks to decompress in one parallel batch
const PREFETCH_COUNT: usize = 8;

/// Reads a file's data blocks from the image, without decompressing them
pub struct SquashfsRawData<'a, 'b, V: BlockReaderVersion<'b>> {
    pub(crate) system: &'a V::System,
    pub(crate) file: &'a V::File,
    current_block: BlockIterator<'a, 'b, V>,
    /// Offset in the image of the next whole block to read
    pub(crate) pos: u64,
    /// Buffer pool for reusing memory across threads
    buffer_pool: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Queue of blocks ready to be processed
    prefetched_blocks: VecDeque<(Vec<u8>, RawDataBlock)>,
    num_prefetch: usize,
}

impl<'a, 'b, V: BlockReaderVersion<'b>> SquashfsRawData<'a, 'b, V> {
    pub fn new(system: &'a V::System, file: &'a V::File) -> Result<Self, BackhandError> {
        let pos = V::blocks_start(file);
        let current_block =
            BlockIterator { blocks: V::block_sizes(file), fragment: V::fragment_of(system, file)? };
        Ok(Self {
            system,
            file,
            current_block,
            pos,
            buffer_pool: Arc::new(Mutex::new(Vec::new())),
            prefetched_blocks: VecDeque::new(),
            num_prefetch: rayon::current_num_threads() / 2,
        })
    }

    /// Like [`Self::new`], but reads only the file's whole blocks, ignoring its fragment
    pub fn new_without_fragment(system: &'a V::System, file: &'a V::File) -> Self {
        let pos = V::blocks_start(file);
        let current_block = BlockIterator { blocks: V::block_sizes(file), fragment: None };
        Self {
            system,
            file,
            current_block,
            pos,
            buffer_pool: Arc::new(Mutex::new(Vec::new())),
            prefetched_blocks: VecDeque::new(),
            num_prefetch: rayon::current_num_threads() / 2,
        }
    }

    /// Prefetch multiple blocks in parallel
    fn prefetch_blocks(&mut self) -> Result<(), BackhandError> {
        for _ in 0..self.num_prefetch {
            match self.current_block.next() {
                Some(block_fragment) => {
                    let mut data = self.buffer_pool.lock().unwrap().pop().unwrap_or_default();

                    let block_info = read_raw_data::<V>(
                        self.system,
                        self.file,
                        &mut self.pos,
                        &mut data,
                        &block_fragment,
                    )?;
                    self.prefetched_blocks.push_back((data, block_info));
                }
                None => break, // No more blocks
            }
        }

        Ok(())
    }

    #[inline]
    pub fn next_block(&mut self, buf: &mut Vec<u8>) -> Option<Result<RawDataBlock, BackhandError>> {
        // If no prefetched blocks are available, try to prefetch
        if self.prefetched_blocks.is_empty()
            && let Err(e) = self.prefetch_blocks()
        {
            return Some(Err(e));
        }

        // Return a prefetched block if available
        if let Some((mut data, block_info)) = self.prefetched_blocks.pop_front() {
            core::mem::swap(buf, &mut data);
            // return buffer to our pool
            self.buffer_pool.lock().unwrap().push(data);
            Some(Ok(block_info))
        } else {
            // No more blocks
            None
        }
    }

    // Advance position by one block without reading/decompressing - internal to Seek impl
    #[inline]
    pub(crate) fn skip_block(&mut self) -> bool {
        // only meant to be called by Seek on freshly-reset raw_data with no prefetch yet
        debug_assert!(self.prefetched_blocks.is_empty());
        match self.current_block.next() {
            Some(BlockFragment::Block(block)) => {
                // correctly adds 0 for sparse blocks (size == 0)
                self.pos += u64::from(V::data_size(block));
                true
            }
            Some(BlockFragment::Fragment(_)) => true, // fragment is last, just consume it
            None => false,
        }
    }

    /// Decompress function that can be run in parallel
    pub fn decompress(
        &self,
        data: RawDataBlock,
        input_buf: &mut Vec<u8>,
        output_buf: &mut Vec<u8>,
    ) -> Result<(), BackhandError> {
        decompress::<V>(self.system, self.file, data, input_buf, output_buf)
    }

    #[inline]
    pub fn into_reader(self) -> SquashfsReadFile<'a, 'b, V> {
        SquashfsReadFile::new(self)
    }
}

/// A [`Read`] + [`Seek`] handle over one file's decompressed contents
pub struct SquashfsReadFile<'a, 'b, V: BlockReaderVersion<'b>> {
    raw_data: SquashfsRawData<'a, 'b, V>,
    buffer_pool: Arc<Mutex<Vec<Vec<u8>>>>,
    decompressed_blocks: VecDeque<Vec<u8>>,
    current_block_position: usize,
    cursor_pos: u64,
    prefetch_count: usize,
}

impl<'a, 'b, V: BlockReaderVersion<'b>> SquashfsReadFile<'a, 'b, V> {
    fn new(raw_data: SquashfsRawData<'a, 'b, V>) -> Self {
        let buffer_pool = Arc::new(Mutex::new(Vec::new()));
        Self {
            raw_data,
            buffer_pool,
            decompressed_blocks: VecDeque::new(),
            current_block_position: 0,
            cursor_pos: 0,
            prefetch_count: PREFETCH_COUNT,
        }
    }

    #[inline]
    fn file_len64(&self) -> u64 {
        V::file_len(self.raw_data.file) as u64
    }

    /// Fill the decompressed blocks queue with data
    fn fill_decompressed_queue(&mut self) -> Result<(), BackhandError> {
        // If we already have data, no need to fill
        if !self.decompressed_blocks.is_empty()
            && self.current_block_position < self.decompressed_blocks.front().unwrap().len()
        {
            return Ok(());
        }

        // If we're in the middle of a block, advance to the next one
        if !self.decompressed_blocks.is_empty() {
            self.decompressed_blocks.pop_front();
            self.current_block_position = 0;

            // If we still have data, no need to fill
            if !self.decompressed_blocks.is_empty() {
                return Ok(());
            }
        }

        // We need to decompress more blocks
        // Collect blocks to decompress
        let mut read_blocks = Vec::new();
        let mut buf_pool = self.buffer_pool.lock().unwrap();

        for _ in 0..self.prefetch_count {
            let mut input_buf = buf_pool.pop().unwrap_or_default();

            if let Some(block_result) = self.raw_data.next_block(&mut input_buf) {
                let block_info = block_result?;
                read_blocks.push((input_buf, block_info));
            } else {
                // Return unused buffer to the pool
                buf_pool.push(input_buf);
                break;
            }
        }

        // Release lock before parallel processing
        drop(buf_pool);

        if read_blocks.is_empty() {
            return Ok(());
        }

        // Use Rayon to decompress blocks in parallel
        let raw_data = &self.raw_data;
        let buffer_pool = &self.buffer_pool;

        let decompressed_results: Vec<Result<Vec<u8>, BackhandError>> = read_blocks
            .into_par_iter()
            .map(|(mut input_buf, block_info)| {
                let mut output_buf = Vec::new();
                let result = raw_data.decompress(block_info, &mut input_buf, &mut output_buf);

                // Return input buffer to the pool
                buffer_pool.lock().unwrap().push(input_buf);

                result.map(|_| output_buf)
            })
            .collect();

        // Process results
        for result in decompressed_results {
            let output_buf = result?;
            self.decompressed_blocks.push_back(output_buf);
        }

        self.current_block_position = 0;
        Ok(())
    }

    /// Available bytes in the current block
    #[inline]
    fn available_in_current_block(&self) -> &[u8] {
        if self.decompressed_blocks.is_empty() {
            &[]
        } else {
            &self.decompressed_blocks.front().unwrap()[self.current_block_position..]
        }
    }

    /// Read available bytes from the current block
    #[inline]
    fn read_available(&mut self, buf: &mut [u8]) -> usize {
        let available = self.available_in_current_block();
        let bytes_left = self.file_len64().saturating_sub(self.cursor_pos);
        let read_len = bytes_left.min(buf.len().min(available.len()) as u64) as usize;

        if read_len > 0 {
            buf[..read_len].copy_from_slice(&available[..read_len]);
            self.cursor_pos += read_len as u64;
            self.current_block_position += read_len;
        }

        read_len
    }
}

impl<'b, V: BlockReaderVersion<'b>> Read for SquashfsReadFile<'_, 'b, V> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Check if we're at the end of the file
        if self.cursor_pos >= self.file_len64() {
            return Ok(0);
        }

        // Ensure we have data to read
        if self.fill_decompressed_queue().is_err() {
            return Err(std::io::Error::other("Failed to decompress data"));
        }

        // If we have no more blocks, we're done
        if self.decompressed_blocks.is_empty() {
            return Ok(0);
        }

        // Read available data
        Ok(self.read_available(buf))
    }
}

impl<'b, V: BlockReaderVersion<'b>> Seek for SquashfsReadFile<'_, 'b, V> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let file_len = self.file_len64();
        let new_pos = u64::try_from(match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => file_len as i64 + n,
            SeekFrom::Current(n) => self.cursor_pos as i64 + n,
        })
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;

        if new_pos == self.cursor_pos {
            return Ok(new_pos);
        }

        // can we seek within already-loaded data, inclusive of end positions?
        if let Some(block) = self.decompressed_blocks.front() {
            let block_start = self.cursor_pos.min(file_len) - self.current_block_position as u64;
            if new_pos >= block_start && new_pos - block_start <= block.len() as u64 {
                // seek within already-loaded data
                self.current_block_position = (new_pos - block_start) as usize;
                self.cursor_pos = new_pos;
                return Ok(new_pos);
            }
        }

        // reset to initial start-of-file state and then skip forward
        self.raw_data = SquashfsRawData::new(self.raw_data.system, self.raw_data.file)
            .map_err(std::io::Error::other)?;
        self.decompressed_blocks.clear();
        self.current_block_position = 0;
        self.cursor_pos = 0;

        if new_pos < file_len {
            // skip full blocks without decompressing them
            let block_size = u64::from(V::block_size(self.raw_data.system));
            while new_pos >= self.cursor_pos + block_size {
                let _skipped = self.raw_data.skip_block();
                debug_assert!(_skipped);
                self.cursor_pos += block_size;
            }
            // no block or fragment loaded yet - load now if necessary, else wait for read
            if new_pos != self.cursor_pos {
                self.fill_decompressed_queue()?;
                if let Some(block) = self.decompressed_blocks.front() {
                    debug_assert!(new_pos <= self.cursor_pos + block.len() as u64);
                }
                self.current_block_position = (new_pos - self.cursor_pos) as usize;
                self.cursor_pos = new_pos;
            }
        } else {
            // drain block iterator to ensure consistent end-of-file state
            while self.raw_data.skip_block() {}
            self.cursor_pos = new_pos; // note, may be greater than file_len
        }

        Ok(new_pos)
    }
}
