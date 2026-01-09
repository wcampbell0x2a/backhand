use no_std_io2::io::Read;
use rayon::prelude::*;
use std::collections::VecDeque;
use std::io::{Seek, SeekFrom};
use std::sync::{Arc, Mutex};

use super::reader::{BlockFragment, BlockIterator, FilesystemReaderFile};
use crate::error::BackhandError;

const PREFETCH_COUNT: usize = 8;

#[derive(Clone, Copy)]
pub(crate) struct RawDataBlock {
    pub(crate) fragment: bool,
    pub(crate) uncompressed: bool,
}

pub(crate) struct SquashfsRawData<'a, 'b> {
    pub(crate) file: FilesystemReaderFile<'a, 'b>,
    current_block: BlockIterator<'a>,
    pub(crate) pos: u64,
    /// Buffer pool for reusing memory across threads
    buffer_pool: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Queue of blocks ready to be processed
    prefetched_blocks: VecDeque<(Vec<u8>, RawDataBlock)>,
    num_prefetch: usize,
}

impl<'a, 'b> SquashfsRawData<'a, 'b> {
    pub fn new(file: FilesystemReaderFile<'a, 'b>) -> Self {
        let pos = file.file.blocks_start();
        let current_block = file.into_iter();
        Self {
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

                    let block_info = self.read_raw_data(&mut data, &block_fragment)?;
                    self.prefetched_blocks.push_back((data, block_info));
                }
                None => break, // No more blocks
            }
        }

        Ok(())
    }

    fn read_raw_data(
        &mut self,
        data: &mut Vec<u8>,
        block: &BlockFragment<'a>,
    ) -> Result<RawDataBlock, BackhandError> {
        match block {
            BlockFragment::Block(block) => {
                let block_size = block.size() as usize;
                // sparse file, don't read from reader, just fill with superblock.block size of 0's
                if block_size == 0 {
                    *data = vec![0; self.file.system.block_size as usize];
                    return Ok(RawDataBlock { fragment: false, uncompressed: true });
                }
                data.resize(block_size, 0);
                //NOTE: storing/restoring the file-pos is not required at the
                //moment of writing, but in the future, it may.
                {
                    let mut reader = self.file.system.reader.lock().unwrap();
                    reader.seek(SeekFrom::Start(self.pos))?;
                    reader.read_exact(data)?;
                    self.pos = reader.stream_position()?;
                }
                Ok(RawDataBlock { fragment: false, uncompressed: block.uncompressed() })
            }
            BlockFragment::Fragment(fragment) => {
                // if in the cache, just read from the cache bytes and return the fragment bytes
                {
                    let cache = self.file.system.cache.read().unwrap();
                    if let Some(cache_bytes) = cache.fragment_cache.get(&fragment.start) {
                        //if in cache, just return the cache, don't read it
                        let range = self.fragment_range();
                        tracing::trace!("fragment in cache: {:02x}:{range:02x?}", fragment.start);
                        data.resize(range.end - range.start, 0);
                        data.copy_from_slice(&cache_bytes[range]);

                        //cache is store uncompressed
                        return Ok(RawDataBlock { fragment: true, uncompressed: true });
                    }
                }

                // if not in the cache, read the entire fragment bytes to store into
                // the cache. Once that is done, if uncompressed just return the bytes
                // that were read that are for the file
                tracing::trace!("fragment: reading from data");
                let frag_size = fragment.size.size() as usize;
                data.resize(frag_size, 0);
                {
                    let mut reader = self.file.system.reader.lock().unwrap();
                    reader.seek(SeekFrom::Start(fragment.start))?;
                    reader.read_exact(data)?;
                }

                // if already decompressed, store
                if fragment.size.uncompressed() {
                    self.file
                        .system
                        .cache
                        .write()
                        .unwrap()
                        .fragment_cache
                        .insert(self.file.fragment().unwrap().start, data.clone());

                    //apply the fragment offset
                    let range = self.fragment_range();
                    data.drain(range.end..);
                    data.drain(..range.start);
                }
                Ok(RawDataBlock { fragment: true, uncompressed: fragment.size.uncompressed() })
            }
        }
    }

    #[inline]
    pub fn next_block(&mut self, buf: &mut Vec<u8>) -> Option<Result<RawDataBlock, BackhandError>> {
        // If no prefetched blocks are available, try to prefetch
        if self.prefetched_blocks.is_empty() {
            if let Err(e) = self.prefetch_blocks() {
                return Some(Err(e));
            }
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

    // skip a block without reading/decompressing, just advance file position
    #[inline]
    pub fn skip_block(&mut self) -> bool {
        if !self.prefetched_blocks.is_empty() {
            // skip_block is only meant to be used before prefetch starts
            false
        } else {
            match self.current_block.next() {
                Some(BlockFragment::Block(block)) => {
                    self.pos += block.size() as u64; // correctly adds 0 for sparse blocks (size == 0)
                    true
                }
                Some(BlockFragment::Fragment(_)) => true, // fragment is last, just consume it
                None => false,
            }
        }
    }

    #[inline]
    fn fragment_range(&self) -> core::ops::Range<usize> {
        let block_len = self.file.system.block_size as usize;
        let block_num = self.file.file.block_sizes().len();
        let file_size = self.file.file.file_len();
        let frag_len = file_size - (block_num * block_len);
        let frag_start = self.file.file.block_offset() as usize;
        let frag_end = frag_start + frag_len;
        frag_start..frag_end
    }

    /// Decompress function that can be run in parallel
    pub fn decompress(
        &self,
        data: RawDataBlock,
        input_buf: &mut Vec<u8>,
        output_buf: &mut Vec<u8>,
    ) -> Result<(), BackhandError> {
        // append to the output_buf is not allowed, it need to be empty
        assert!(output_buf.is_empty());
        // input is already decompress, so just swap the input/output, so the
        // output_buf contains the final data.
        if data.uncompressed {
            core::mem::swap(input_buf, output_buf);
        } else {
            output_buf.reserve(self.file.system.block_size as usize);
            self.file.system.kind.inner.compressor.decompress(
                input_buf,
                output_buf,
                self.file.system.compressor,
            )?;
            // store the cache, so decompression is not duplicated
            if data.fragment {
                self.file
                    .system
                    .cache
                    .write()
                    .unwrap()
                    .fragment_cache
                    .insert(self.file.fragment().unwrap().start, output_buf.clone());

                //apply the fragment offset
                let range = self.fragment_range();
                output_buf.drain(range.end..);
                output_buf.drain(..range.start);
            }
        }
        Ok(())
    }

    #[inline]
    pub fn into_reader(self) -> SquashfsReadFile<'a, 'b> {
        SquashfsReadFile::new(self)
    }
}

pub struct SquashfsReadFile<'a, 'b> {
    raw_data: SquashfsRawData<'a, 'b>,
    buffer_pool: Arc<Mutex<Vec<Vec<u8>>>>,
    decompressed_blocks: VecDeque<Vec<u8>>,
    current_block_position: usize,
    cursor_pos: u64,
    prefetch_count: usize,
}

impl<'a, 'b> SquashfsReadFile<'a, 'b> {
    fn new(raw_data: SquashfsRawData<'a, 'b>) -> Self {
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
        self.raw_data.file.file.file_len() as u64
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
                match block_result {
                    Ok(block_info) => read_blocks.push((input_buf, block_info)),
                    Err(e) => return Err(e),
                }
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
            match result {
                Ok(output_buf) => self.decompressed_blocks.push_back(output_buf),
                Err(e) => return Err(e),
            }
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

impl Read for SquashfsReadFile<'_, '_> {
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

impl Seek for SquashfsReadFile<'_, '_> {
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
        self.raw_data = self.raw_data.file.raw_data_reader();
        self.decompressed_blocks.clear();
        self.current_block_position = 0;
        self.cursor_pos = 0;

        if new_pos < file_len {
            // skip full blocks without decompressing them
            let block_size = 1u64 << self.raw_data.file.system.block_log; // block_size is 0 in v3
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
