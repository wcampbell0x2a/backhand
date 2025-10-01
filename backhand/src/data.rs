//! File Data

use std::collections::HashMap;
use std::io::{BufReader, Error, Read, Seek, Write};
use std::ops::DerefMut;
use std::sync::{mpsc, Arc};

use arc_swap::ArcSwap;
use deku::prelude::*;
use orx_concurrent_vec::ConcurrentVec;
use parking_lot::Mutex;
use rayon::iter::{ParallelBridge, ParallelIterator};
use solana_nohash_hasher::IntMap;
use tracing::trace;
use xxhash_rust::xxh64::xxh64;

use crate::compressor::CompressionAction;
use crate::error::BackhandError;
use crate::filesystem::writer::FilesystemCompressor;
use crate::fragment::Fragment;
use crate::reader::WriteSeek;

#[cfg(not(feature = "parallel"))]
use crate::filesystem::reader_no_parallel::SquashfsRawData;
#[cfg(feature = "parallel")]
use crate::filesystem::reader_parallel::SquashfsRawData;

// bitflag for data size field in inode for signifying that the data is uncompressed
const DATA_STORED_UNCOMPRESSED: u32 = 1 << 24;

#[derive(Copy, Clone, Debug, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(endian = "endian", ctx = "endian: deku::ctx::Endian")]
pub struct DataSize(u32);
impl DataSize {
    #[inline]
    pub fn new(size: u32, uncompressed: bool) -> Self {
        let mut value: u32 = size;
        if value > DATA_STORED_UNCOMPRESSED {
            panic!("value is too big");
        }
        if uncompressed {
            value |= DATA_STORED_UNCOMPRESSED;
        }
        Self(value)
    }

    #[inline]
    pub fn new_compressed(size: u32) -> Self {
        Self::new(size, false)
    }

    #[inline]
    pub fn new_uncompressed(size: u32) -> Self {
        Self::new(size, true)
    }

    #[inline]
    pub fn uncompressed(&self) -> bool {
        self.0 & DATA_STORED_UNCOMPRESSED != 0
    }

    #[inline]
    pub fn set_uncompressed(&mut self) {
        self.0 |= DATA_STORED_UNCOMPRESSED
    }

    #[inline]
    pub fn set_compressed(&mut self) {
        self.0 &= !DATA_STORED_UNCOMPRESSED
    }

    #[inline]
    pub fn size(&self) -> u32 {
        self.0 & !DATA_STORED_UNCOMPRESSED
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    // Only Data was added
    Data { blocks_start: u32, block_sizes: Vec<DataSize> },
    // Only Fragment was added
    Fragment { frag_index: u32, block_offset: u32 },
}

struct DataWriterChunkReader<R: std::io::Read + Send + Sync> {
    block_size: usize,
    file_len: usize,
    reader: BufReader<R>,
}
impl<R: std::io::Read + Send + Sync> Iterator for &mut DataWriterChunkReader<R> {
    type Item = Vec<u8>;
    fn next(&mut self) -> Option<Vec<u8>> {
        use std::io::ErrorKind;
        let mut chunk = vec![0; self.block_size];
        let mut read_len = 0;
        loop {
            match self.reader.read(&mut chunk[read_len..]) {
                Ok(0) => {
                    break;
                }
                Ok(n) => {
                    read_len += n;
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_e) => return None,
            }
        }
        self.file_len += read_len;
        chunk.truncate(read_len);
        Some(chunk)
    }
}

pub(crate) struct DataWriter<'a> {
    kind: &'a dyn CompressionAction,
    block_size: u32,
    fs_compressor: FilesystemCompressor,
    /// Cache of HashMap<file_len, HashMap<hash, (file_len, Added)>>
    #[allow(clippy::type_complexity)]
    dup_cache: Mutex<IntMap<u64, IntMap<u64, (usize, Added)>>>,
    no_duplicate_files: bool,
    /// Un-written fragment_bytes
    pub(crate) fragment_bytes: ArcSwap<ConcurrentVec<u8>>,
    pub(crate) fragment_table: ConcurrentVec<Fragment>,
}

impl<'a> DataWriter<'a> {
    pub fn new(
        kind: &'a dyn CompressionAction,
        fs_compressor: FilesystemCompressor,
        block_size: u32,
        no_duplicate_files: bool,
    ) -> Self {
        Self {
            kind,
            block_size,
            fs_compressor,
            dup_cache: Mutex::new(HashMap::default()),
            no_duplicate_files,
            fragment_bytes: ArcSwap::from(Arc::new(ConcurrentVec::new())),
            fragment_table: ConcurrentVec::new(),
        }
    }

    /// Add to data writer, either a pre-compressed Data or Fragment
    // TODO: support tail-end fragments (off by default in squashfs-tools/mksquashfs)
    pub(crate) fn just_copy_it<W: WriteSeek>(
        &self,
        mut reader: SquashfsRawData,
        writer: Arc<Mutex<W>>,
    ) -> Result<(usize, Added), BackhandError> {
        //just clone it, because block sizes where never modified, just copy it
        let mut block_sizes = reader.file.file.block_sizes().to_vec();
        let mut read_buf = vec![];
        let mut decompress_buf = vec![];
        let mut writer = writer.lock();

        // if the first block is not full (fragment), store only a fragment
        // otherwise processed to store blocks
        let blocks_start = writer.stream_position()? as u32;
        let first_block = match reader.next_block(&mut read_buf) {
            Some(Ok(first_block)) => first_block,
            Some(Err(x)) => return Err(x),
            None => return Ok((0, Added::Data { blocks_start, block_sizes })),
        };

        // write and early return if fragment
        if first_block.fragment {
            reader.decompress(first_block, &mut read_buf, &mut decompress_buf)?;
            // if this doesn't fit in the current fragment bytes
            // compress the current fragment bytes and add to data_bytes
            if (decompress_buf.len() + self.fragment_bytes.load().len()) > self.block_size as usize
            {
                self.finalize(writer.deref_mut())?;
            }
            // add to fragment bytes
            let frag_index = self.fragment_table.len() as u32;
            let block_offset = self.fragment_bytes.load().len() as u32;
            let buf_len = decompress_buf.len();
            self.fragment_bytes.load().extend(decompress_buf);

            return Ok((buf_len, Added::Fragment { frag_index, block_offset }));
        }

        //if is a block, just copy it
        writer.write_all(&read_buf)?;
        while let Some(block) = reader.next_block(&mut read_buf) {
            let block = block?;
            if block.fragment {
                reader.decompress(block, &mut read_buf, &mut decompress_buf)?;
                // TODO: support tail-end fragments, for now just treat it like a block
                let cb =
                    self.kind.compress(&decompress_buf, self.fs_compressor, self.block_size)?;
                // compression didn't reduce size
                if cb.len() > decompress_buf.len() {
                    // store uncompressed
                    block_sizes.push(DataSize::new_uncompressed(decompress_buf.len() as u32));
                    writer.write_all(&decompress_buf)?;
                } else {
                    // store compressed
                    block_sizes.push(DataSize::new_compressed(cb.len() as u32));
                    writer.write_all(&cb)?;
                }
            } else {
                //if is a block, just copy it
                writer.write_all(&read_buf)?;
            }
        }
        let file_size = reader.file.file.file_len();
        Ok((file_size, Added::Data { blocks_start, block_sizes }))
    }

    /// Add to data writer, either a Data or Fragment
    ///
    /// If `self.dup_cache` is on, return alrady added `(usize, Added)` if duplicate
    /// is found
    // TODO: support tail-end fragments (off by default in squashfs-tools/mksquashfs)
    pub(crate) fn add_bytes<W: WriteSeek + Send + Sync>(
        &self,
        reader: impl Read + Send + Sync,
        writer: Arc<Mutex<W>>,
    ) -> Result<(usize, Added), BackhandError> {
        let mut chunk_reader = DataWriterChunkReader {
            block_size: self.block_size as usize,
            file_len: 0,
            reader: BufReader::with_capacity(bytesize::MIB as usize * 8, reader),
        };

        // read entire chunk (file)
        let chunk = (&mut chunk_reader)
            .next()
            .ok_or(BackhandError::StdIo(Error::other("could not read file chunk")))?;

        // chunk size not exactly the size of the block
        if chunk.len() != self.block_size as usize {
            // if this doesn't fit in the current fragment bytes
            // compress the current fragment bytes and add to data_bytes
            let mut writer_lock = writer.lock();
            if (chunk.len() + self.fragment_bytes.load().len()) > self.block_size as usize {
                self.finalize(writer_lock.deref_mut())?;
            }

            // add to fragment bytes
            let frag_index = self.fragment_table.len() as u32;
            let block_offset = self.fragment_bytes.load().len() as u32;
            self.fragment_bytes.load().extend(chunk);

            return Ok((chunk_reader.file_len, Added::Fragment { frag_index, block_offset }));
        }

        // If duplicate file checking is enabled, use the old data position as this file if it hashes the same
        if self.no_duplicate_files {
            if let Some(c) = self.dup_cache.lock().get(&(chunk.len() as u64)) {
                let hash = xxh64(&chunk, 0);
                if let Some(res) = c.get(&hash) {
                    trace!("duplicate file data found");
                    return Ok(res.clone());
                }
            }
        }

        // Save information needed to add to duplicate_cache later
        let chunk_len = chunk.len();
        let hash = xxh64(&chunk, 0);

        let (write_tx, write_rx) = mpsc::channel::<(u32, Vec<u8>)>();
        // Spawn writer thread
        let mut blocks_start = Ok(0);
        let mut block_sizes: Result<Vec<_>, _> = Ok(vec![]);
        rayon::scope(|scope| -> Result<_, BackhandError> {
            let blocks_start = &mut blocks_start;
            scope.spawn(move |_| {
                let mut wait_idx = 0;
                let mut chunk_wait = IntMap::default();
                let mut writer_lock = writer.lock();
                *blocks_start = writer_lock.stream_position().map(|pos| pos as u32);
                if blocks_start.is_err() {
                    return;
                }
                while let Ok((chunk_idx, chunk)) = write_rx.recv() {
                    if chunk_idx == wait_idx {
                        if let Err(e) = writer_lock.write_all(&chunk) {
                            *blocks_start = Err(e);
                            return;
                        }
                        wait_idx += 1;
                    } else {
                        chunk_wait.insert(chunk_idx, chunk);
                    }
                    while let Some(chunk) = chunk_wait.remove(&wait_idx) {
                        if let Err(e) = writer_lock.write_all(&chunk) {
                            *blocks_start = Err(e);
                            return;
                        }
                        wait_idx += 1;
                    }
                }
                if !chunk_wait.is_empty() {
                    panic!("Did not write all file chunks");
                }
            });
            let all_chunks = ([chunk].into_iter().chain(&mut chunk_reader))
                .take_while(|chunk| !chunk.is_empty());
            scope.spawn(|_| {
                let sizes: Result<Vec<_>, _> = ParallelIterator::map(
                    all_chunks.enumerate().par_bridge(),
                    |(idx, chunk)| -> Result<_, BackhandError> {
                        let idx = idx as u32;
                        let cb = self.kind.compress(&chunk, self.fs_compressor, self.block_size)?;

                        // compression didn't reduce size
                        if cb.len() > chunk.len() {
                            // store uncompressed
                            let len = chunk.len();
                            write_tx.send((idx, chunk)).unwrap();
                            Ok((idx, DataSize::new_uncompressed(len as u32)))
                        } else {
                            // store compressed
                            let len = cb.len();
                            write_tx.send((idx, cb)).unwrap();
                            Ok((idx, DataSize::new_compressed(len as u32)))
                        }
                    },
                )
                .collect::<Result<_, _>>();
                drop(write_tx);

                let mut sizes = match sizes {
                    Ok(sizes) => sizes,
                    Err(e) => {
                        block_sizes = Err(e);
                        return;
                    }
                };

                // It would be nice to reuse this buffer instead of recreating it, with some form of zipped sort
                sizes.sort_unstable_by_key(|(idx, _)| *idx);
                block_sizes = Ok(sizes.into_iter().map(|(_, block_size)| block_size).collect());
            });
            Ok(())
        })
        .unwrap();

        // Add to duplicate information cache
        let added = (
            chunk_reader.file_len,
            Added::Data { blocks_start: blocks_start?, block_sizes: block_sizes? },
        );

        // If duplicate files checking is enbaled, then add this to it's memory
        if self.no_duplicate_files {
            let mut dup_cache = self.dup_cache.lock();
            if let Some(entry) = dup_cache.get_mut(&(chunk_len as u64)) {
                entry.insert(hash, added.clone());
            } else {
                let mut hashmap = IntMap::default();
                hashmap.insert(hash, added.clone());
                dup_cache.insert(chunk_len as u64, hashmap);
            }
        }
        Ok(added)
    }

    /// Compress the fragments that were under length, write to data, add to fragment table, clear
    /// current fragment_bytes
    pub fn finalize<W: Write + Seek>(&self, writer: &mut W) -> Result<(), BackhandError> {
        let bytes_vec = self.fragment_bytes.swap(Arc::new(ConcurrentVec::new())).clone_to_vec();
        let cb = self.kind.compress(&bytes_vec, self.fs_compressor, self.block_size)?;

        // compression didn't reduce size
        let start = writer.stream_position()?;
        let size = if cb.len() > bytes_vec.len() {
            // store uncompressed
            writer.write_all(&bytes_vec)?;
            DataSize::new_uncompressed(bytes_vec.len() as u32)
        } else {
            // store compressed
            writer.write_all(&cb)?;
            DataSize::new_compressed(cb.len() as u32)
        };
        self.fragment_table.push(Fragment::new(start, size, 0));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::{
        compression::{Compressor, DefaultCompressor},
        DEFAULT_BLOCK_SIZE,
    };

    #[test]
    #[cfg(feature = "gzip")]
    fn duplicate_check() {
        let data_writer = DataWriter::new(
            &DefaultCompressor,
            FilesystemCompressor::new(Compressor::Gzip, None).unwrap(),
            DEFAULT_BLOCK_SIZE,
            true,
        );
        let bytes = [0xff_u8; DEFAULT_BLOCK_SIZE as usize * 2];
        let writer = Arc::new(Mutex::new(Cursor::new(vec![])));
        let added_1 = data_writer.add_bytes(&bytes[..], writer.clone()).unwrap();
        let added_2 = data_writer.add_bytes(&bytes[..], writer).unwrap();
        assert_eq!(added_1, added_2);
    }

    #[test]
    #[cfg(feature = "gzip")]
    fn no_duplicate_check() {
        let data_writer = DataWriter::new(
            &DefaultCompressor,
            FilesystemCompressor::new(Compressor::Gzip, None).unwrap(),
            DEFAULT_BLOCK_SIZE,
            false,
        );
        let bytes = [0xff_u8; DEFAULT_BLOCK_SIZE as usize * 2];
        let writer = Arc::new(Mutex::new(Cursor::new(vec![])));
        let added_1 = data_writer.add_bytes(&bytes[..], writer.clone()).unwrap();
        let added_2 = data_writer.add_bytes(&bytes[..], writer).unwrap();
        assert_ne!(added_1, added_2);
    }

    #[test]
    fn empty_chunk_reader() {
        let bytes = [0; 0];
        let mut chunk_reader = DataWriterChunkReader {
            block_size: 128,
            file_len: 0,
            reader: BufReader::new(Cursor::new(&bytes)),
        };
        let chunk = (&mut chunk_reader).next().unwrap();
        assert_eq!(chunk.len(), 0);
    }

    #[test]
    fn chunk_reader() {
        let bytes = [0xff_u8; 128 * 2 + 1];
        let mut chunk_reader = DataWriterChunkReader {
            block_size: 128,
            file_len: 0,
            reader: BufReader::new(Cursor::new(&bytes)),
        };
        let chunk = (&mut chunk_reader).next().unwrap();
        assert_eq!(chunk.len(), 128);
        let chunk = (&mut chunk_reader).next().unwrap();
        assert_eq!(chunk.len(), 128);
        let chunk = (&mut chunk_reader).next().unwrap();
        assert_eq!(chunk.len(), 1);
        let chunk = (&mut chunk_reader).next().unwrap();
        assert!(chunk.is_empty());
    }

    #[test]
    fn add_file() {
        let data_writer = DataWriter::new(
            &DefaultCompressor,
            FilesystemCompressor::new(Compressor::None, None).unwrap(),
            DEFAULT_BLOCK_SIZE,
            false,
        );
        let bytes = [0xff_u8; DEFAULT_BLOCK_SIZE as usize * 2];
        let writer = Arc::new(Mutex::new(Cursor::new(vec![])));
        let added_1 = data_writer.add_bytes(&bytes[..], writer).unwrap();
        assert_eq!(added_1.0, bytes.len());
        let Added::Data { blocks_start: 0, block_sizes } = added_1.1 else {
            panic!("Expected added data, got {:?}", added_1.1)
        };
        assert_eq!(block_sizes, vec![DataSize(DEFAULT_BLOCK_SIZE), DataSize(DEFAULT_BLOCK_SIZE)]);
    }
}
