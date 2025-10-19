use no_std_io2::io::Read;
use std::io::SeekFrom;

use super::reader::{BlockFragment, BlockIterator, FilesystemReaderFile};
use crate::error::BackhandError;

#[derive(Clone, Copy)]
pub(crate) struct RawDataBlock {
    pub(crate) fragment: bool,
    pub(crate) uncompressed: bool,
}

pub(crate) struct SquashfsRawData<'a, 'b> {
    pub(crate) file: FilesystemReaderFile<'a, 'b>,
    current_block: BlockIterator<'a>,
    pub(crate) pos: u64,
}

impl<'a, 'b> SquashfsRawData<'a, 'b> {
    pub fn new(file: FilesystemReaderFile<'a, 'b>) -> Self {
        let pos = file.file.blocks_start();
        let current_block = file.into_iter();
        Self { file, current_block, pos }
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
        self.current_block.next().map(|next| self.read_raw_data(buf, &next))
    }

    #[inline]
    fn fragment_range(&self) -> std::ops::Range<usize> {
        let block_len = self.file.system.block_size as usize;
        let block_num = self.file.file.block_sizes().len();
        let file_size = self.file.file.file_len();
        let frag_len = file_size - (block_num * block_len);
        let frag_start = self.file.file.block_offset() as usize;
        let frag_end = frag_start + frag_len;
        frag_start..frag_end
    }

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
            std::mem::swap(input_buf, output_buf);
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
        let block_size = self.file.system.block_size as usize;
        let bytes_available = self.file.file.file_len();
        SquashfsReadFile::new(block_size, self, 0, bytes_available)
    }
}

pub struct SquashfsReadFile<'a, 'b> {
    raw_data: SquashfsRawData<'a, 'b>,
    buf_read: Vec<u8>,
    buf_decompress: Vec<u8>,
    //offset of buf_decompress to start reading
    last_read: usize,
    bytes_available: usize,
}

impl<'a, 'b> SquashfsReadFile<'a, 'b> {
    fn new(
        block_size: usize,
        raw_data: SquashfsRawData<'a, 'b>,
        last_read: usize,
        bytes_available: usize,
    ) -> Self {
        Self {
            raw_data,
            buf_read: Vec::with_capacity(block_size),
            buf_decompress: vec![],
            last_read,
            bytes_available,
        }
    }

    #[inline]
    fn available(&self) -> &[u8] {
        &self.buf_decompress[self.last_read..]
    }

    #[inline]
    fn read_available(&mut self, buf: &mut [u8]) -> usize {
        let available = self.available();
        let read_len = buf.len().min(available.len()).min(self.bytes_available);
        buf[..read_len].copy_from_slice(&available[..read_len]);
        self.bytes_available -= read_len;
        self.last_read += read_len;
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
        self.last_read = 0;
        Ok(())
    }
}

impl Read for SquashfsReadFile<'_, '_> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // file was fully consumed
        if self.bytes_available == 0 {
            self.buf_read.clear();
            self.buf_decompress.clear();
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
