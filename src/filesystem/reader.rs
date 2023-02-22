use std::cell::RefCell;
use std::io::{Read, SeekFrom};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::data::DataSize;
use crate::error::SquashfsError;
use crate::fragment::Fragment;
use crate::inode::BasicFile;
use crate::reader::{ReadSeek, SquashfsReaderWithOffset};
use crate::squashfs::{Cache, Id};
use crate::{Node, Squashfs, SquashfsDir, SquashfsFileReader};

/// Representation of SquashFS filesystem after read from image
#[derive(Debug)]
pub struct FilesystemReader<R: ReadSeek> {
    /// See [`SuperBlock`].`block_size`
    pub block_size: u32,
    /// See [`SuperBlock`].`block_log`
    pub block_log: u16,
    /// See [`SuperBlock`].`compressor`
    pub compressor: Compressor,
    /// See [`Squashfs`].`compression_options`
    pub compression_options: Option<CompressionOptions>,
    /// See [`SuperBlock`].`mod_time`
    pub mod_time: u32,
    /// See [`Squashfs`].`id`
    pub id_table: Option<Vec<Id>>,
    /// Fragments Lookup Table
    pub fragments: Option<Vec<Fragment>>,
    /// Information for the `/` node
    pub root_inode: SquashfsDir,
    /// All files and directories in filesystem
    pub nodes: Vec<Node<SquashfsFileReader>>,
    // File reader
    pub(crate) reader: RefCell<R>,
    // Cache used in the decompression
    pub(crate) cache: RefCell<Cache>,
}

impl<R: ReadSeek> FilesystemReader<R> {
    /// Call [`Squashfs::from_reader`], then [`Squashfs::into_filesystem_reader`]
    pub fn from_reader(reader: R) -> Result<Self, SquashfsError> {
        let squashfs = Squashfs::from_reader(reader)?;
        squashfs.into_filesystem_reader()
    }
}

impl<R: ReadSeek> FilesystemReader<SquashfsReaderWithOffset<R>> {
    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before reading
    pub fn from_reader_with_offset(reader: R, offset: u64) -> Result<Self, SquashfsError> {
        let squashfs = Squashfs::from_reader_with_offset(reader, offset)?;
        squashfs.into_filesystem_reader()
    }
}

impl<R: ReadSeek> FilesystemReader<R> {
    /// Return a file handler for this file
    pub fn file<'a>(&'a self, basic_file: &'a BasicFile) -> FilesystemReaderFile<'a, R> {
        FilesystemReaderFile::new(self, basic_file)
    }

    /// Read and return all the bytes from the file
    pub fn read_file(&self, basic_file: &BasicFile) -> Result<Vec<u8>, SquashfsError> {
        let file = FilesystemReaderFile::new(self, basic_file);
        let mut reader = file.reader();
        let mut bytes = Vec::with_capacity(basic_file.file_size as usize);
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

/// Filesystem handle for file
#[derive(Copy)]
pub struct FilesystemReaderFile<'a, R: ReadSeek> {
    pub(crate) system: &'a FilesystemReader<R>,
    pub(crate) basic: &'a BasicFile,
}

impl<'a, R: ReadSeek> Clone for FilesystemReaderFile<'a, R> {
    fn clone(&self) -> Self {
        Self {
            system: self.system,
            basic: self.basic,
        }
    }
}

impl<'a, R: ReadSeek> FilesystemReaderFile<'a, R> {
    pub fn new(system: &'a FilesystemReader<R>, basic: &'a BasicFile) -> Self {
        Self { system, basic }
    }

    pub fn reader(&self) -> SquashfsReadFile<'a, R> {
        self.raw_data_reader().into_reader()
    }

    pub fn fragment(&self) -> Option<&'a Fragment> {
        if self.basic.frag_index == 0xffffffff {
            None
        } else {
            self.system
                .fragments
                .as_ref()
                .map(|fragments| &fragments[self.basic.frag_index as usize])
        }
    }

    pub(crate) fn raw_data_reader(&self) -> SquashfsRawData<'a, R> {
        SquashfsRawData::new(Self {
            system: self.system,
            basic: self.basic,
        })
    }
}

impl<'a, R: ReadSeek> IntoIterator for FilesystemReaderFile<'a, R> {
    type IntoIter = BlockIterator<'a>;
    type Item = <BlockIterator<'a> as Iterator>::Item;

    fn into_iter(self) -> Self::IntoIter {
        BlockIterator {
            blocks: &self.basic.block_sizes,
            fragment: self.fragment(),
        }
    }
}

pub enum BlockFragment<'a> {
    Block(&'a DataSize),
    Fragment(&'a Fragment),
}

pub struct BlockIterator<'a> {
    pub blocks: &'a [DataSize],
    pub fragment: Option<&'a Fragment>,
}

impl<'a> Iterator for BlockIterator<'a> {
    type Item = BlockFragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.blocks
            .split_first()
            .map(|(first, rest)| {
                self.blocks = rest;
                BlockFragment::Block(first)
            })
            .or_else(|| self.fragment.take().map(BlockFragment::Fragment))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RawDataBlock {
    pub(crate) fragment: bool,
    pub(crate) uncompressed: bool,
}

pub(crate) struct SquashfsRawData<'a, R: ReadSeek> {
    pub(crate) file: FilesystemReaderFile<'a, R>,
    current_block: BlockIterator<'a>,
    pub(crate) pos: u64,
}

impl<'a, R: ReadSeek> SquashfsRawData<'a, R> {
    pub fn new(file: FilesystemReaderFile<'a, R>) -> Self {
        let pos = file.basic.blocks_start.into();
        let current_block = file.clone().into_iter();
        Self {
            file,
            current_block,
            pos,
        }
    }

    fn read_raw_data(
        &mut self,
        data: &mut Vec<u8>,
        block: &BlockFragment<'a>,
    ) -> Result<RawDataBlock, SquashfsError> {
        match block {
            BlockFragment::Block(block) => {
                let block_size = block.size() as usize;
                data.resize(block_size, 0);
                //NOTE: storing/restoring the file-pos is not required at the
                //moment of writing, but in the future, it may.
                let mut reader = self.file.system.reader.borrow_mut();
                reader.seek(SeekFrom::Start(self.pos))?;
                reader.read_exact(data)?;
                self.pos = reader.stream_position()?;
                Ok(RawDataBlock {
                    fragment: false,
                    uncompressed: block.uncompressed(),
                })
            },
            BlockFragment::Fragment(fragment) => {
                let cache = self.file.system.cache.borrow();
                if let Some(cache_bytes) = cache.fragment_cache.get(&fragment.start) {
                    //if in cache, just return the cache, don't read it
                    let cache_size = cache_bytes.len();
                    data.resize(cache_size, 0);
                    data[..cache_size].copy_from_slice(cache_bytes);
                    //cache is store uncompressed
                    Ok(RawDataBlock {
                        fragment: true,
                        uncompressed: true,
                    })
                } else {
                    //otherwise read and return it
                    let frag_size = fragment.size.size() as usize;
                    data.resize(frag_size, 0);
                    let mut reader = self.file.system.reader.borrow_mut();
                    reader.seek(SeekFrom::Start(fragment.start))?;
                    reader.read_exact(data)?;
                    Ok(RawDataBlock {
                        fragment: true,
                        uncompressed: fragment.size.uncompressed(),
                    })
                }
            },
        }
    }

    pub fn next_block(&mut self, buf: &mut Vec<u8>) -> Option<Result<RawDataBlock, SquashfsError>> {
        self.current_block
            .next()
            .map(|next| self.read_raw_data(buf, &next))
    }

    fn fragment_range(&self) -> std::ops::Range<usize> {
        let block_len = self.file.system.block_size as usize;
        let block_num = self.file.basic.block_sizes.len();
        let file_size = self.file.basic.file_size as usize;
        let frag_len = file_size - (block_num * block_len);
        let frag_start = self.file.basic.block_offset as usize;
        let frag_end = frag_start + frag_len;
        frag_start..frag_end
    }

    pub fn decompress(
        &self,
        data: RawDataBlock,
        input_buf: &mut Vec<u8>,
        output_buf: &mut Vec<u8>,
    ) -> Result<(), SquashfsError> {
        //input is already decompress, so just swap the input/output, so the
        //output_buf contains the final data.
        if data.uncompressed {
            std::mem::swap(input_buf, output_buf);
        } else {
            output_buf.reserve(self.file.system.block_size as usize);
            compressor::decompress(input_buf, output_buf, self.file.system.compressor)?;
            // store the cache, so decompression is not duplicated
            if data.fragment {
                self.file
                    .system
                    .cache
                    .borrow_mut()
                    .fragment_cache
                    .insert(self.file.fragment().unwrap().start, output_buf.clone());
            }
        }
        //apply the fragment offset
        if data.fragment {
            let range = self.fragment_range();
            output_buf.drain(range.end..);
            output_buf.drain(..range.start);
        }
        Ok(())
    }

    pub fn into_reader(self) -> SquashfsReadFile<'a, R> {
        let bytes_available = self.file.basic.file_size as usize;
        let buf_read = Vec::with_capacity(self.file.system.block_size as usize);
        let buf_decompress = Vec::with_capacity(self.file.system.block_size as usize);
        SquashfsReadFile {
            raw_data: self,
            buf_read,
            buf_decompress,
            last_read: 0,
            bytes_available,
        }
    }
}

pub struct SquashfsReadFile<'a, R: ReadSeek> {
    raw_data: SquashfsRawData<'a, R>,
    buf_read: Vec<u8>,
    buf_decompress: Vec<u8>,
    //offset of buf_decompress to start reading
    last_read: usize,
    bytes_available: usize,
}

impl<'a, R: ReadSeek> SquashfsReadFile<'a, R> {
    pub fn available(&self) -> &[u8] {
        &self.buf_decompress[self.last_read..]
    }

    pub fn read_available(&mut self, buf: &mut [u8]) -> usize {
        let available = self.available();
        let read_len = buf.len().min(available.len()).min(self.bytes_available);
        buf[..read_len].copy_from_slice(&available[..read_len]);
        self.bytes_available -= read_len;
        self.last_read += read_len;
        read_len
    }

    pub fn read_next_block(&mut self) -> Result<(), SquashfsError> {
        let block = match self.raw_data.next_block(&mut self.buf_read) {
            Some(block) => block?,
            None => return Ok(()),
        };
        self.raw_data
            .decompress(block, &mut self.buf_read, &mut self.buf_decompress)?;
        self.last_read = 0;
        Ok(())
    }
}

impl<'a, R: ReadSeek> Read for SquashfsReadFile<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // file was fully consumed
        if self.bytes_available == 0 {
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
