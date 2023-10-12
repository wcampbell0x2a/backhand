use std::io::{Read, SeekFrom};
use std::sync::Mutex;

use super::node::Nodes;
use crate::bufread::BufReadSeek;
use crate::compressor::{CompressionOptions, Compressor};
use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::v4::data::DataSize;
use crate::v4::fragment::Fragment;
use crate::v4::id::Id;
use crate::v4::inode::BasicFile;
use crate::v4::squashfs::Cache;
use crate::v4::{Node, Squashfs, SquashfsFileReader};

/// Representation of SquashFS filesystem after read from image
/// - Use [`Self::from_reader`] to read into `Self` from a `reader`
///
/// # Read direct into [`Self`]
/// Usual workflow, reading from image into a default squashfs [`Self`]. See [InnerNode] for more
/// details for `.nodes`.
/// ```rust,no_run
/// # use std::fs::File;
/// # use std::io::BufReader;
/// # use backhand::{
/// #     FilesystemReader, InnerNode, Squashfs, SquashfsBlockDevice, SquashfsCharacterDevice,
/// #     SquashfsDir, SquashfsSymlink,
/// # };
/// // Read into filesystem
/// let file = BufReader::new(File::open("image.squashfs").unwrap());
/// let filesystem = FilesystemReader::from_reader(file).unwrap();
///
/// // Iterate through nodes
/// // (See src/bin/unsquashfs.rs for more examples on extraction)
/// for node in filesystem.files() {
///     // extract
///     match &node.inner {
///         InnerNode::File(_) => (),
///         InnerNode::Symlink(_) => (),
///         InnerNode::Dir(_) => (),
///         InnerNode::CharacterDevice(_) => (),
///         InnerNode::BlockDevice(_) => (),
///     }
/// }
/// ```
///
/// # Read from [`Squashfs`]
/// Performance wise, you may want to read into a [`Squashfs`] first, if for instance you are
/// optionally not extracting and only listing some Superblock fields.
/// ```rust,no_run
/// # use std::fs::File;
/// # use std::io::BufReader;
/// # use backhand::{
/// #     FilesystemReader, InnerNode, Squashfs, SquashfsBlockDevice, SquashfsCharacterDevice,
/// #     SquashfsDir, SquashfsSymlink,
/// # };
/// // Read into Squashfs
/// let file = BufReader::new(File::open("image.squashfs").unwrap());
/// let squashfs = Squashfs::from_reader_with_offset(file, 0).unwrap();
///
/// // Display the Superblock info
/// let superblock = squashfs.superblock;
/// println!("{superblock:#08x?}");
///
/// // Now read into filesystem
/// let filesystem = squashfs.into_filesystem_reader().unwrap();
/// ```
/// [InnerNode]: [`crate::InnerNode`]
pub struct FilesystemReader<'b> {
    pub kind: Kind,
    /// The size of a data block in bytes. Must be a power of two between 4096 (4k) and 1048576 (1 MiB).
    pub block_size: u32,
    /// The log2 of the block size. If the two fields do not agree, the archive is considered corrupted.
    pub block_log: u16,
    /// Compressor used for data
    pub compressor: Compressor,
    /// Optional Compressor used for data stored in image
    pub compression_options: Option<CompressionOptions>,
    /// Last modification time of the archive. Count seconds since 00:00, Jan 1st 1970 UTC (not counting leap seconds).
    /// This is unsigned, so it expires in the year 2106 (as opposed to 2038).
    pub mod_time: u32,
    /// ID's stored for gui(s) and uid(s)
    pub id_table: Vec<Id>,
    /// Fragments Lookup Table
    pub fragments: Option<Vec<Fragment>>,
    /// All files and directories in filesystem
    pub root: Nodes<SquashfsFileReader>,
    // File reader
    pub(crate) reader: Mutex<Box<dyn BufReadSeek + 'b>>,
    // Cache used in the decompression
    pub(crate) cache: Mutex<Cache>,
}

impl<'b> FilesystemReader<'b> {
    /// Call [`Squashfs::from_reader`], then [`Squashfs::into_filesystem_reader`]
    ///
    /// With default kind: [`crate::kind::LE_V4_0`] and offset `0`.
    pub fn from_reader<R: BufReadSeek + 'b>(reader: R) -> Result<Self, BackhandError> {
        let squashfs = Squashfs::from_reader_with_offset(reader, 0)?;
        squashfs.into_filesystem_reader()
    }

    /// Allocate two properly sized buffers for [`FilesystemReaderFile::reader`]
    pub fn alloc_read_buffers(&self) -> (Vec<u8>, Vec<u8>) {
        let buf_read = Vec::with_capacity(self.block_size as usize);
        let buf_decompress = vec![];

        (buf_read, buf_decompress)
    }

    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before reading
    pub fn from_reader_with_offset<R: BufReadSeek + 'b>(
        reader: R,
        offset: u64,
    ) -> Result<Self, BackhandError> {
        let squashfs = Squashfs::from_reader_with_offset(reader, offset)?;
        squashfs.into_filesystem_reader()
    }

    /// Same as [`Self::from_reader_with_offset`], but setting custom `kind`
    pub fn from_reader_with_offset_and_kind<R: BufReadSeek + 'b>(
        reader: R,
        offset: u64,
        kind: Kind,
    ) -> Result<Self, BackhandError> {
        let squashfs = Squashfs::from_reader_with_offset_and_kind(reader, offset, kind)?;
        squashfs.into_filesystem_reader()
    }

    /// Return a file handler for this file
    pub fn file<'a>(&'a self, basic_file: &'a BasicFile) -> FilesystemReaderFile<'a, 'b> {
        FilesystemReaderFile::new(self, basic_file)
    }

    /// Iterator of all files, including the root
    ///
    /// # Example
    /// Used when extracting a file from the image, for example using [`FilesystemReaderFile`]:
    /// ```rust,no_run
    /// # use std::fs::File;
    /// # use std::io::BufReader;
    /// # use backhand::{
    /// #     FilesystemReader, InnerNode, Squashfs, SquashfsBlockDevice, SquashfsCharacterDevice,
    /// #     SquashfsDir, SquashfsSymlink,
    /// # };
    /// # let file = BufReader::new(File::open("image.squashfs").unwrap());
    /// # let filesystem = FilesystemReader::from_reader(file).unwrap();
    /// // [snip: creating FilesystemReader]
    ///
    /// // alloc required space for file data readers
    /// let (mut buf_read, mut buf_decompress) = filesystem.alloc_read_buffers();
    ///
    /// for node in filesystem.files() {
    ///     // extract
    ///     match &node.inner {
    ///         InnerNode::File(file) => {
    ///             let mut reader = filesystem
    ///                 .file(&file.basic)
    ///                 .reader(&mut buf_read, &mut buf_decompress);
    ///             // Then, do something with the reader
    ///         },
    ///         _ => (),
    ///     }
    /// }
    /// ```
    pub fn files(&self) -> impl Iterator<Item = &Node<SquashfsFileReader>> {
        self.root.nodes.iter()
    }
}

/// Filesystem handle for file
#[derive(Copy, Clone)]
pub struct FilesystemReaderFile<'a, 'b> {
    pub(crate) system: &'a FilesystemReader<'b>,
    pub(crate) basic: &'a BasicFile,
}

impl<'a, 'b> FilesystemReaderFile<'a, 'b> {
    pub fn new(system: &'a FilesystemReader<'b>, basic: &'a BasicFile) -> Self {
        Self { system, basic }
    }

    /// Create [`SquashfsReadFile`] that impls [`std::io::Read`] from [`FilesystemReaderFile`].
    /// This can be used to then call functions from [`std::io::Read`]
    /// to de-compress and read the data from this file.
    ///
    /// # Arguments
    ///
    /// * `buf_read` - Pre-allocated buffer of `block_size`, from [alloc_read_buffers].
    ///     Calls [Vec::clear] after final [Read::read] call.
    /// * `buf_decompress` - Pre-allocated buffer of `block_size`, from [alloc_read_buffers].
    ///     Calls [Vec::clear] after final [Read::read] call.
    ///
    /// [alloc_read_buffers]: FilesystemReader::alloc_read_buffers
    /// [Read::read]: std::io::Read::read
    /// [Vec::clear]: Vec::clear
    pub fn reader(
        &self,
        buf_read: &'a mut Vec<u8>,
        buf_decompress: &'a mut Vec<u8>,
    ) -> SquashfsReadFile<'a, 'b> {
        self.raw_data_reader().into_reader(buf_read, buf_decompress)
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

    pub(crate) fn raw_data_reader(&self) -> SquashfsRawData<'a, 'b> {
        SquashfsRawData::new(Self {
            system: self.system,
            basic: self.basic,
        })
    }
}

impl<'a, 'b> IntoIterator for FilesystemReaderFile<'a, 'b> {
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

pub(crate) struct SquashfsRawData<'a, 'b> {
    pub(crate) file: FilesystemReaderFile<'a, 'b>,
    current_block: BlockIterator<'a>,
    pub(crate) pos: u64,
}

impl<'a, 'b> SquashfsRawData<'a, 'b> {
    pub fn new(file: FilesystemReaderFile<'a, 'b>) -> Self {
        let pos = file.basic.blocks_start.into();
        let current_block = file.into_iter();
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
    ) -> Result<RawDataBlock, BackhandError> {
        match block {
            BlockFragment::Block(block) => {
                let block_size = block.size() as usize;
                data.resize(block_size, 0);
                //NOTE: storing/restoring the file-pos is not required at the
                //moment of writing, but in the future, it may.
                let mut reader = self.file.system.reader.lock().unwrap();
                reader.seek(SeekFrom::Start(self.pos))?;
                reader.read_exact(data)?;
                self.pos = reader.stream_position()?;
                Ok(RawDataBlock {
                    fragment: false,
                    uncompressed: block.uncompressed(),
                })
            }
            BlockFragment::Fragment(fragment) => {
                let cache = self.file.system.cache.lock().unwrap();
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
                    let mut reader = self.file.system.reader.lock().unwrap();
                    reader.seek(SeekFrom::Start(fragment.start))?;
                    reader.read_exact(data)?;
                    Ok(RawDataBlock {
                        fragment: true,
                        uncompressed: fragment.size.uncompressed(),
                    })
                }
            }
        }
    }

    pub fn next_block(&mut self, buf: &mut Vec<u8>) -> Option<Result<RawDataBlock, BackhandError>> {
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
    ) -> Result<(), BackhandError> {
        //append to the output_buf is not allowed, it need to be empty
        assert!(output_buf.is_empty());
        //input is already decompress, so just swap the input/output, so the
        //output_buf contains the final data.
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
                    .lock()
                    .unwrap()
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

    pub fn into_reader(
        self,
        buf_read: &'a mut Vec<u8>,
        buf_decompress: &'a mut Vec<u8>,
    ) -> SquashfsReadFile<'a, 'b> {
        let bytes_available = self.file.basic.file_size as usize;
        SquashfsReadFile {
            raw_data: self,
            buf_read,
            buf_decompress,
            last_read: 0,
            bytes_available,
        }
    }
}

pub struct SquashfsReadFile<'a, 'b> {
    raw_data: SquashfsRawData<'a, 'b>,
    buf_read: &'a mut Vec<u8>,
    buf_decompress: &'a mut Vec<u8>,
    //offset of buf_decompress to start reading
    last_read: usize,
    bytes_available: usize,
}

impl<'a, 'b> SquashfsReadFile<'a, 'b> {
    fn available(&self) -> &[u8] {
        &self.buf_decompress[self.last_read..]
    }

    fn read_available(&mut self, buf: &mut [u8]) -> usize {
        let available = self.available();
        let read_len = buf.len().min(available.len()).min(self.bytes_available);
        buf[..read_len].copy_from_slice(&available[..read_len]);
        self.bytes_available -= read_len;
        self.last_read += read_len;
        read_len
    }

    fn read_next_block(&mut self) -> Result<(), BackhandError> {
        let block = match self.raw_data.next_block(self.buf_read) {
            Some(block) => block?,
            None => return Ok(()),
        };
        self.buf_decompress.clear();
        self.raw_data
            .decompress(block, self.buf_read, self.buf_decompress)?;
        self.last_read = 0;
        Ok(())
    }
}

impl<'a, 'b> Read for SquashfsReadFile<'a, 'b> {
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
