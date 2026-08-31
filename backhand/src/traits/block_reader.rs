//! Version-agnostic file data block reading

use no_std_io2::io::Read;
use std::io::{Seek, SeekFrom};
use std::sync::{Mutex, RwLock};

use solana_nohash_hasher::IntMap;

use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::traits::types::Compressor;
use crate::v4::reader::BufReadSeek;

/// Decompressed fragment blocks, keyed by their start position in the image
#[derive(Default, Clone, Debug)]
pub struct Cache {
    pub(crate) fragment_cache: IntMap<u64, Vec<u8>>,
}

/// The version-specific surface the shared block reader depends on
pub trait BlockReaderVersion<'b> {
    /// A block's on-disk length plus its "stored uncompressed" flag
    type DataSize: Copy + Sync + 'b;
    /// A fragment block's location and length
    type Fragment: Sync + 'b;
    /// A file inode, holding the block list and fragment reference
    type File: Sync + 'b;
    /// The filesystem the file belongs to, owning the image reader and fragment cache
    type System: Sync + 'b;

    /// Length of this block as stored in the image, in bytes
    fn data_size(data_size: &Self::DataSize) -> u32;

    /// Whether this block is stored uncompressed and needs no decompression
    fn data_uncompressed(data_size: &Self::DataSize) -> bool;

    /// Offset of the fragment block from the start of the image
    fn fragment_start(fragment: &Self::Fragment) -> u64;

    /// Length and compression flag of the fragment block
    fn fragment_size(fragment: &Self::Fragment) -> Self::DataSize;

    /// Uncompressed length of the file, in bytes
    fn file_len(file: &Self::File) -> usize;

    /// The file's whole blocks, excluding any trailing fragment
    fn block_sizes(file: &Self::File) -> &[Self::DataSize];

    /// Offset of the file's first data block from the start of the image
    fn blocks_start(file: &Self::File) -> u64;

    /// Offset of the file's tail within its fragment block
    fn block_offset(file: &Self::File) -> u32;

    /// Format description, providing the decompression implementation
    fn kind(system: &Self::System) -> &Kind;

    /// Uncompressed size of a full data block, from the superblock
    fn block_size(system: &Self::System) -> u32;

    /// Compressor id from the superblock, or `None` when the version records none
    fn compressor(system: &Self::System) -> Option<Compressor>;

    /// The image reader, shared between all files of the filesystem
    fn reader(system: &Self::System) -> &Mutex<Box<dyn BufReadSeek + 'b>>;

    /// Decompressed fragment blocks, shared between all files of the filesystem
    fn cache(system: &Self::System) -> &RwLock<Cache>;

    /// The fragment this file's tail lives in, or `None` when it has no fragment
    fn fragment_of<'a>(
        system: &'a Self::System,
        file: &'a Self::File,
    ) -> Result<Option<&'a Self::Fragment>, crate::error::BackhandError>;
}

/// One unit of file data: either a whole block or the file's tail within a fragment block
pub enum BlockFragment<'a, 'b, V: BlockReaderVersion<'b>> {
    Block(&'a V::DataSize),
    Fragment(&'a V::Fragment),
}

/// Walks a file's whole blocks in order, then yields its fragment tail if it has one
pub struct BlockIterator<'a, 'b, V: BlockReaderVersion<'b>> {
    pub blocks: &'a [V::DataSize],
    pub fragment: Option<&'a V::Fragment>,
}

impl<'a, 'b, V: BlockReaderVersion<'b>> Iterator for BlockIterator<'a, 'b, V> {
    type Item = BlockFragment<'a, 'b, V>;

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

/// A block as read from the image, before decompression
#[derive(Clone, Copy, Debug)]
pub struct RawDataBlock {
    pub(crate) fragment: bool,
    pub(crate) uncompressed: bool,
}

/// A handle to one file within a filesystem, from which its data can be read
pub struct FilesystemReaderFile<'a, 'b: 'a, V: BlockReaderVersion<'b>> {
    pub(crate) system: &'a V::System,
    pub(crate) file: &'a V::File,
}

// Derived `Copy`/`Clone` would demand `V: Copy`, which the marker types need not be.
impl<'b, V: BlockReaderVersion<'b>> Clone for FilesystemReaderFile<'_, 'b, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'b, V: BlockReaderVersion<'b>> Copy for FilesystemReaderFile<'_, 'b, V> {}

impl<'a, 'b, V: BlockReaderVersion<'b>> FilesystemReaderFile<'a, 'b, V> {
    pub fn new(system: &'a V::System, file: &'a V::File) -> Self {
        Self { system, file }
    }

    pub fn fragment(&self) -> Option<&'a V::Fragment> {
        self.fragment_checked().ok().flatten()
    }

    pub(crate) fn fragment_checked(&self) -> Result<Option<&'a V::Fragment>, BackhandError> {
        V::fragment_of(self.system, self.file)
    }
}

impl<'a, 'b, V: BlockReaderVersion<'b>> IntoIterator for FilesystemReaderFile<'a, 'b, V> {
    type IntoIter = BlockIterator<'a, 'b, V>;
    type Item = BlockFragment<'a, 'b, V>;

    fn into_iter(self) -> Self::IntoIter {
        BlockIterator { blocks: V::block_sizes(self.file), fragment: self.fragment() }
    }
}

/// Byte range of a file's tail within its fragment block
///
/// Errors when the file's recorded sizes disagree with the fragment block.
#[inline]
pub(crate) fn fragment_range<'b, V: BlockReaderVersion<'b>>(
    system: &V::System,
    file: &V::File,
    frag_buf_len: usize,
) -> Result<std::ops::Range<usize>, BackhandError> {
    let block_len = V::block_size(system) as usize;
    let block_num = V::block_sizes(file).len();
    let file_size = V::file_len(file);
    let frag_start = V::block_offset(file) as usize;

    (|| {
        let frag_len = file_size.checked_sub(block_num.checked_mul(block_len)?)?;
        let frag_end = frag_start.checked_add(frag_len)?;
        (frag_end <= frag_buf_len).then_some(frag_start..frag_end)
    })()
    .ok_or(BackhandError::CorruptedOrInvalidSquashfs)
}

/// Read one block or fragment from the image into `data`, without decompressing it
///
/// Advances `pos` past a whole block; fragments leave it untouched.
pub(crate) fn read_raw_data<'a, 'b, V: BlockReaderVersion<'b>>(
    system: &V::System,
    file: &V::File,
    pos: &mut u64,
    data: &mut Vec<u8>,
    block: &BlockFragment<'a, 'b, V>,
) -> Result<RawDataBlock, BackhandError> {
    let block_size = V::block_size(system) as usize;
    match block {
        BlockFragment::Block(block) => {
            let block_len = V::data_size(block) as usize;
            // sparse file, don't read from reader, just fill with superblock.block size of 0's
            if block_len == 0 {
                *data = vec![0; block_size];
                return Ok(RawDataBlock { fragment: false, uncompressed: true });
            }
            if block_len > block_size {
                return Err(BackhandError::CorruptedOrInvalidSquashfs);
            }
            data.resize(block_len, 0);
            //NOTE: storing/restoring the file-pos is not required at the
            //moment of writing, but in the future, it may.
            {
                let mut reader = V::reader(system).lock().unwrap();
                reader.seek(SeekFrom::Start(*pos))?;
                reader.read_exact(data)?;
                *pos = reader.stream_position()?;
            }
            Ok(RawDataBlock { fragment: false, uncompressed: V::data_uncompressed(block) })
        }
        BlockFragment::Fragment(fragment) => {
            let frag_start = V::fragment_start(fragment);
            let frag_data_size = V::fragment_size(fragment);
            // if in the cache, just read from the cache bytes and return the fragment bytes
            {
                let cache = V::cache(system).read().unwrap();
                if let Some(cache_bytes) = cache.fragment_cache.get(&frag_start) {
                    //if in cache, just return the cache, don't read it
                    let range = fragment_range::<V>(system, file, cache_bytes.len())?;
                    tracing::trace!("fragment in cache: {:02x}:{range:02x?}", frag_start);
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
            let frag_len = V::data_size(&frag_data_size) as usize;
            if frag_len > block_size {
                return Err(BackhandError::CorruptedOrInvalidSquashfs);
            }
            data.resize(frag_len, 0);
            {
                let mut reader = V::reader(system).lock().unwrap();
                reader.seek(SeekFrom::Start(frag_start))?;
                reader.read_exact(data)?;
            }

            // if already decompressed, store
            let uncompressed = V::data_uncompressed(&frag_data_size);
            if uncompressed {
                let range = fragment_range::<V>(system, file, data.len())?;
                V::cache(system).write().unwrap().fragment_cache.insert(frag_start, data.clone());

                //apply the fragment offset
                data.drain(range.end..);
                data.drain(..range.start);
            }
            Ok(RawDataBlock { fragment: true, uncompressed })
        }
    }
}

/// Decompress one block into `output_buf`, which must be empty
pub(crate) fn decompress<'b, V: BlockReaderVersion<'b>>(
    system: &V::System,
    file: &V::File,
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
        let block_size = V::block_size(system) as usize;
        output_buf.reserve(block_size);
        V::kind(system).decompress(input_buf, output_buf, V::compressor(system), block_size)?;
        // store the cache, so decompression is not duplicated
        if data.fragment {
            let fragment =
                V::fragment_of(system, file)?.ok_or(BackhandError::CorruptedOrInvalidSquashfs)?;
            //apply the fragment offset
            let range = fragment_range::<V>(system, file, output_buf.len())?;
            V::cache(system)
                .write()
                .unwrap()
                .fragment_cache
                .insert(V::fragment_start(fragment), output_buf.clone());

            output_buf.drain(range.end..);
            output_buf.drain(..range.start);
        }
    }
    Ok(())
}
