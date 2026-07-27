//! Sequential file data block reading, shared by all SquashFS versions

use no_std_io2::io::Read;
use std::io::{Seek, SeekFrom};

use crate::error::BackhandError;
use crate::traits::block_reader::{
    BlockFragment, BlockIterator, BlockReaderVersion, RawDataBlock, decompress, read_raw_data,
};

/// Reads a file's data blocks from the image, without decompressing them
pub struct SquashfsRawData<'a, 'b, V: BlockReaderVersion<'b>> {
    pub(crate) system: &'a V::System,
    pub(crate) file: &'a V::File,
    current_block: BlockIterator<'a, 'b, V>,
    /// Offset in the image of the next whole block to read
    pub(crate) pos: u64,
}

impl<'a, 'b, V: BlockReaderVersion<'b>> SquashfsRawData<'a, 'b, V> {
    pub fn new(system: &'a V::System, file: &'a V::File) -> Result<Self, BackhandError> {
        let pos = V::blocks_start(file);
        let current_block =
            BlockIterator { blocks: V::block_sizes(file), fragment: V::fragment_of(system, file)? };
        Ok(Self { system, file, current_block, pos })
    }

    /// Like [`Self::new`], but reads only the file's whole blocks, ignoring its fragment
    pub fn new_without_fragment(system: &'a V::System, file: &'a V::File) -> Self {
        let pos = V::blocks_start(file);
        let current_block = BlockIterator { blocks: V::block_sizes(file), fragment: None };
        Self { system, file, current_block, pos }
    }

    #[inline]
    pub fn next_block(&mut self, buf: &mut Vec<u8>) -> Option<Result<RawDataBlock, BackhandError>> {
        self.current_block
            .next()
            .map(|next| read_raw_data::<V>(self.system, self.file, &mut self.pos, buf, &next))
    }

    // Advance position by one block without reading/decompressing - internal to Seek impl
    #[inline]
    pub(crate) fn skip_block(&mut self) -> bool {
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
    buf_read: Vec<u8>,
    buf_decompress: Vec<u8>,
    //offset of buf_decompress to start reading
    current_block_position: usize,
    cursor_pos: u64,
}

impl<'a, 'b, V: BlockReaderVersion<'b>> SquashfsReadFile<'a, 'b, V> {
    fn new(raw_data: SquashfsRawData<'a, 'b, V>) -> Self {
        let block_size = V::block_size(raw_data.system) as usize;
        Self {
            raw_data,
            buf_read: Vec::with_capacity(block_size),
            buf_decompress: Vec::with_capacity(block_size),
            current_block_position: 0,
            cursor_pos: 0,
        }
    }

    #[inline]
    fn file_len64(&self) -> u64 {
        V::file_len(self.raw_data.file) as u64
    }

    #[inline]
    fn available(&self) -> &[u8] {
        &self.buf_decompress[self.current_block_position..]
    }

    #[inline]
    fn read_available(&mut self, buf: &mut [u8]) -> usize {
        let available = self.available();
        let bytes_left = self.file_len64().saturating_sub(self.cursor_pos);
        let read_len = bytes_left.min(buf.len().min(available.len()) as u64) as usize;
        buf[..read_len].copy_from_slice(&available[..read_len]);
        self.cursor_pos += read_len as u64;
        self.current_block_position += read_len;
        read_len
    }

    #[inline]
    fn read_next_block(&mut self) -> Result<(), BackhandError> {
        let block = match self.raw_data.next_block(&mut self.buf_read) {
            Some(block) => block?,
            None => return Ok(()),
        };
        self.buf_decompress.clear();
        self.raw_data.decompress(block, &mut self.buf_read, &mut self.buf_decompress)?;
        self.current_block_position = 0;
        Ok(())
    }
}

impl<'b, V: BlockReaderVersion<'b>> Read for SquashfsReadFile<'_, 'b, V> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // file was fully consumed
        if self.cursor_pos >= self.file_len64() {
            return Ok(0);
        }
        //no data available, read the next block
        if self.available().is_empty() {
            self.read_next_block()?;
        }

        //return data from the read block/fragment
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
        if self.cursor_pos <= file_len && !self.buf_decompress.is_empty() {
            self.cursor_pos -= self.current_block_position as u64;
            self.current_block_position = 0;
            if new_pos >= self.cursor_pos
                && new_pos - self.cursor_pos <= self.buf_decompress.len() as u64
            {
                // seek within already-loaded data
                self.current_block_position = (new_pos - self.cursor_pos) as usize;
                self.cursor_pos = new_pos;
                return Ok(new_pos);
            }
        }

        // reset to initial start-of-file state and then skip forward
        self.raw_data = SquashfsRawData::new(self.raw_data.system, self.raw_data.file)
            .map_err(std::io::Error::other)?;
        self.buf_read.clear();
        self.buf_decompress.clear();
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
                self.read_next_block()?;
                debug_assert!(new_pos <= self.cursor_pos + self.buf_decompress.len() as u64);
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
