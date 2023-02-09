//! In-memory representation of SquashFS filesystem tree used for writing to image

use core::fmt;
use std::cell::RefCell;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use deku::DekuContainerWrite;
use tracing::{info, instrument, trace};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::data::{DataSize, DataWriter};
use crate::error::SquashfsError;
use crate::fragment::Fragment;
use crate::inode::{BasicFile, InodeHeader};
use crate::metadata::{self, MetadataWriter};
use crate::reader::{ReadSeek, SquashfsReaderWithOffset, WriteSeek};
use crate::squashfs::{Cache, Id, SuperBlock};
use crate::tree::TreeNode;
use crate::{fragment, Squashfs};

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
pub(crate) struct RawDataBlock<'b> {
    pub(crate) data: &'b [u8],
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
    fn read_raw_data<'b>(
        &mut self,
        data: &'b mut Vec<u8>,
        block: BlockFragment<'a>,
    ) -> Result<RawDataBlock<'b>, SquashfsError> {
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
                    data,
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
                        data,
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
                        data,
                        fragment: true,
                        uncompressed: fragment.size.uncompressed(),
                    })
                }
            },
        }
    }
    pub fn next_block<'b>(
        &mut self,
        buf: &'b mut Vec<u8>,
    ) -> Option<Result<RawDataBlock<'b>, SquashfsError>> {
        self.current_block
            .next()
            .map(|next| self.read_raw_data(buf, next))
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
    pub fn decompress<'b>(
        &self,
        block: &RawDataBlock<'b>,
        buf: &'b mut Vec<u8>,
    ) -> Result<&'b [u8], SquashfsError> {
        match block {
            RawDataBlock {
                data,
                fragment,
                uncompressed: false,
            } => {
                buf.clear();
                compressor::decompress(data, buf, self.file.system.compressor)?;
                // store the cache, so decompression is not duplicated
                if *fragment {
                    self.file
                        .system
                        .cache
                        .borrow_mut()
                        .fragment_cache
                        .insert(self.file.fragment().unwrap().start, buf.clone());
                    let range = self.fragment_range();
                    Ok(&buf[range])
                } else {
                    Ok(buf.as_slice())
                }
            },
            RawDataBlock {
                data,
                fragment: true,
                uncompressed: true,
            } => {
                let range = self.fragment_range();
                Ok(&data[range])
            },
            RawDataBlock {
                data,
                fragment: false,
                uncompressed: true,
            } => Ok(data),
        }
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
        if block.uncompressed {
            self.raw_data.decompress(&block, &mut self.buf_decompress)?;
        } else {
            //data is already decompress, so just swap the read and decompress
            //buffers, so the buf_decompress contains the final data.
            std::mem::swap(&mut self.buf_read, &mut self.buf_decompress);
        }
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

/// Used in situations that FilesystemWriter is created without a previously
/// existing squashfs, if used, just panic.
pub struct DummyReadSeek;
impl Read for DummyReadSeek {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        unreachable!()
    }
}
impl Seek for DummyReadSeek {
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        unreachable!()
    }
}

/// Representation of SquashFS filesystem to be written back to an image
#[derive(Debug)]
pub struct FilesystemWriter<'a, R: ReadSeek = DummyReadSeek> {
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
    /// Information for the `/` node
    pub root_inode: SquashfsDir,
    /// All files and directories in filesystem, including root
    pub nodes: Vec<Node<SquashfsFileWriter<'a, R>>>,
}

impl<'a, R: ReadSeek> FilesystemWriter<'a, R> {
    /// use the same configuration then an existing SquashFsFile
    pub fn from_fs_reader(reader: &'a FilesystemReader<R>) -> Result<Self, SquashfsError> {
        let nodes = reader
            .nodes
            .iter()
            .map(|x| {
                let inner = match &x.inner {
                    InnerNode::File(file) => {
                        let reader = reader.file(&file.basic);
                        InnerNode::File(SquashfsFileWriter {
                            header: file.header,
                            reader: SquashfsFileSource::SquashfsFile(reader),
                        })
                    },
                    InnerNode::Symlink(x) => InnerNode::Symlink(x.clone()),
                    InnerNode::Dir(x) => InnerNode::Dir(x.clone()),
                    InnerNode::CharacterDevice(x) => InnerNode::CharacterDevice(x.clone()),
                    InnerNode::BlockDevice(x) => InnerNode::BlockDevice(x.clone()),
                };
                Ok(Node {
                    path: x.path.clone(),
                    inner,
                })
            })
            .collect::<Result<_, SquashfsError>>()?;
        Ok(Self {
            block_size: reader.block_size,
            block_log: reader.block_log,
            compressor: reader.compressor,
            compression_options: reader.compression_options,
            mod_time: reader.mod_time,
            id_table: reader.id_table.clone(),
            root_inode: reader.root_inode.clone(),
            nodes,
        })
    }

    /// Insert `reader` into filesystem with `path` and metadata `header`.
    ///
    /// This will make parent directories as needed with the same metadata of `header`
    pub fn push_file<P: Into<PathBuf>>(
        &mut self,
        reader: impl Read + 'a,
        path: P,
        header: NodeHeader,
    ) -> Result<(), SquashfsError> {
        let path = path.into();
        if path.parent().is_some() {
            let mut full_path = "".to_string();
            let components: Vec<_> = path.components().map(|comp| comp.as_os_str()).collect();
            'component: for dir in components.iter().take(components.len() - 1) {
                // add to path
                full_path.push('/');
                full_path.push_str(dir.to_str().ok_or(SquashfsError::OsStringToStr)?);

                // check if exists
                for node in &mut self.nodes {
                    if let InnerNode::Dir(_) = &node.inner {
                        if node.path.as_os_str().to_str()
                            == Some(dir.to_str().ok_or(SquashfsError::OsStringToStr)?)
                        {
                            break 'component;
                        }
                    }
                }

                // not found, add to dir
                let new_dir = InnerNode::Dir(SquashfsDir { header });
                let node = Node::new(PathBuf::from(full_path.clone()), new_dir);
                self.nodes.push(node);
            }
        }

        let reader = RefCell::new(Box::new(reader));
        let new_file = InnerNode::File(SquashfsFileWriter {
            header,
            reader: SquashfsFileSource::UserDefined(reader),
        });
        let node = Node::new(path, new_file);
        self.nodes.push(node);

        Ok(())
    }

    /// Take a mutable reference to existing file at `find_path`
    pub fn mut_file<S: Into<PathBuf>>(
        &mut self,
        find_path: S,
    ) -> Option<&mut SquashfsFileWriter<'a, R>> {
        let find_path = find_path.into();
        find_path.strip_prefix("/").unwrap();
        for node in &mut self.nodes {
            if let InnerNode::File(file) = &mut node.inner {
                if node.path == find_path {
                    return Some(file);
                }
            }
        }

        None
    }

    /// Replace an existing file
    pub fn replace_file<S: Into<PathBuf>>(
        &mut self,
        find_path: S,
        reader: impl Read + 'a,
    ) -> Result<(), SquashfsError> {
        let file = self
            .mut_file(find_path)
            .ok_or(SquashfsError::FileNotFound)?;
        file.reader = SquashfsFileSource::UserDefined(RefCell::new(Box::new(reader)));
        Ok(())
    }

    /// Insert symlink `path` -> `link`
    pub fn push_symlink<P: Into<PathBuf>, S: Into<PathBuf>>(
        &mut self,
        link: S,
        path: P,
        header: NodeHeader,
    ) -> Result<(), SquashfsError> {
        let path = path.into();

        let new_symlink = InnerNode::Symlink(SquashfsSymlink {
            header,
            link: link.into(),
        });
        let node = Node::new(path, new_symlink);
        self.nodes.push(node);

        Ok(())
    }

    /// Insert empty `dir` at `path`
    pub fn push_dir<P: Into<PathBuf>>(
        &mut self,
        path: P,
        header: NodeHeader,
    ) -> Result<(), SquashfsError> {
        let path = path.into();

        let new_dir = InnerNode::Dir(SquashfsDir { header });
        let node = Node::new(path, new_dir);
        self.nodes.push(node);

        Ok(())
    }

    /// Insert character device with `device_number` at `path`
    pub fn push_char_device<P: Into<PathBuf>>(
        &mut self,
        device_number: u32,
        path: P,
        header: NodeHeader,
    ) -> Result<(), SquashfsError> {
        let path = path.into();

        let new_device = InnerNode::CharacterDevice(SquashfsCharacterDevice {
            header,
            device_number,
        });
        let node = Node::new(path, new_device);
        self.nodes.push(node);

        Ok(())
    }

    /// Insert block device with `device_number` at `path`
    pub fn push_block_device<P: Into<PathBuf>>(
        &mut self,
        device_number: u32,
        path: P,
        header: NodeHeader,
    ) -> Result<(), SquashfsError> {
        let path = path.into();

        let new_device = InnerNode::BlockDevice(SquashfsBlockDevice {
            header,
            device_number,
        });
        let node = Node::new(path, new_device);
        self.nodes.push(node);

        Ok(())
    }

    /// Generate the final squashfs file at the offset.
    #[instrument(skip_all)]
    pub fn write_with_offset<W: Write + Seek>(
        &self,
        w: &mut W,
        offset: u64,
    ) -> Result<(), SquashfsError> {
        let mut writer = WriterWithOffset { w, offset };
        self.write(&mut writer)
    }

    /// Generate the final squashfs file. This generates the Superblock with the
    /// correct fields from `Filesystem`, and the data after that contains the nodes.
    #[instrument(skip_all)]
    pub fn write<W: Write + Seek>(&self, w: &mut W) -> Result<(), SquashfsError> {
        let mut superblock = SuperBlock::new(self.compressor);

        trace!("{:#02x?}", self.nodes);
        info!("Creating Tree");
        let mut tree: TreeNode<R> = self.into();
        info!("Tree Created");

        // Empty Squashfs Superblock
        w.write_all(&[0x00; 96])?;
        let mut data_writer = DataWriter::new(self.compressor, None, self.block_size);
        let mut inode_writer = MetadataWriter::new(self.compressor, None, self.block_size);
        let mut dir_writer = MetadataWriter::new(self.compressor, None, self.block_size);

        info!("Creating Inodes and Dirs");
        //trace!("TREE: {:#02x?}", tree);
        info!("Writing Data");
        tree.write_data(self, w, &mut data_writer)?;
        info!("Writing Data Fragments");
        // Compress fragments and write
        data_writer.finalize(w)?;

        info!("Writing Other stuff");
        let (_, root_inode) = tree.write_inode_dir(&mut inode_writer, &mut dir_writer, 0)?;

        superblock.root_inode = root_inode;
        superblock.inode_count = self.nodes.len() as u32 + 1; // + 1 for the "/"
        superblock.block_size = self.block_size;
        superblock.block_log = self.block_log;
        superblock.mod_time = self.mod_time;

        info!("Writing Inodes");
        superblock.inode_table = w.stream_position()?;
        inode_writer.finalize(w)?;

        info!("Writing Dirs");
        superblock.dir_table = w.stream_position()?;
        dir_writer.finalize(w)?;

        info!("Writing Frag Lookup Table");
        Self::write_frag_table(w, data_writer.fragment_table, &mut superblock)?;

        info!("Writing Id Lookup Table");
        Self::write_id_table(w, &self.id_table, &mut superblock)?;

        info!("Finalize Superblock and End Bytes");
        Self::finalize(w, &mut superblock)?;

        info!("Superblock: {:#02x?}", superblock);
        info!("Success");
        Ok(())
    }

    fn finalize<W: Write + Seek>(
        w: &mut W,
        superblock: &mut SuperBlock,
    ) -> Result<(), SquashfsError> {
        // Pad out block_size
        info!("Writing Padding");
        superblock.bytes_used = w.stream_position()?;
        let blocks_used = superblock.bytes_used as u32 / 0x1000;
        let pad_len = (blocks_used + 1) * 0x1000;
        let pad_len = pad_len - superblock.bytes_used as u32;
        w.write_all(&vec![0x00; pad_len as usize])?;

        // Seek back the beginning and write the superblock
        info!("Writing Superblock");
        trace!("{:#02x?}", superblock);
        w.rewind()?;
        w.write_all(&superblock.to_bytes()?)?;

        info!("Writing Finished");

        Ok(())
    }

    fn write_id_table<W: Write + Seek>(
        w: &mut W,
        id_table: &Option<Vec<Id>>,
        write_superblock: &mut SuperBlock,
    ) -> Result<(), SquashfsError> {
        if let Some(id) = id_table {
            let id_table_dat = w.stream_position()?;
            let mut id_bytes = Vec::with_capacity(id.len() * ((u32::BITS / 8) as usize));
            for i in id {
                let bytes = i.to_bytes()?;
                id_bytes.write_all(&bytes)?;
            }
            let metadata_len = metadata::set_if_uncompressed(id_bytes.len() as u16).to_le_bytes();
            w.write_all(&metadata_len)?;
            w.write_all(&id_bytes)?;
            write_superblock.id_table = w.stream_position()?;
            write_superblock.id_count = id.len() as u16;
            w.write_all(&id_table_dat.to_le_bytes())?;
        }

        Ok(())
    }

    fn write_frag_table<W: Write + Seek>(
        w: &mut W,
        frag_table: Vec<Fragment>,
        write_superblock: &mut SuperBlock,
    ) -> Result<(), SquashfsError> {
        let frag_table_dat = w.stream_position()?;
        let mut frag_bytes = Vec::with_capacity(frag_table.len() * fragment::SIZE);
        for f in &frag_table {
            let bytes = f.to_bytes()?;
            frag_bytes.write_all(&bytes)?;
        }
        let metadata_len = metadata::set_if_uncompressed(frag_bytes.len() as u16).to_le_bytes();
        w.write_all(&metadata_len)?;
        w.write_all(&frag_bytes)?;
        write_superblock.frag_table = w.stream_position()?;
        write_superblock.frag_count = frag_table.len() as u32;
        w.write_all(&frag_table_dat.to_le_bytes())?;

        Ok(())
    }
}

/// File information for Node
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct NodeHeader {
    pub permissions: u16,
    pub uid: u16,
    pub gid: u16,
    pub mtime: u32,
}

impl From<InodeHeader> for NodeHeader {
    fn from(inode_header: InodeHeader) -> Self {
        Self {
            permissions: inode_header.permissions,
            uid: inode_header.uid,
            gid: inode_header.gid,
            mtime: inode_header.mtime,
        }
    }
}

/// Filesystem Node
#[derive(Debug, PartialEq, Eq)]
pub struct Node<T> {
    pub path: PathBuf,
    pub inner: InnerNode<T>,
}

impl<T> Node<T> {
    pub fn new(path: PathBuf, inner: InnerNode<T>) -> Self {
        Self { path, inner }
    }
}

/// Filesystem node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerNode<T> {
    /// Either [`SquashfsFileReader`] or [`SquashfsFileWriter`]
    File(T),
    Symlink(SquashfsSymlink),
    Dir(SquashfsDir),
    CharacterDevice(SquashfsCharacterDevice),
    BlockDevice(SquashfsBlockDevice),
}

/// Unread file for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsFileReader {
    pub header: NodeHeader,
    pub basic: BasicFile,
}

/// Read file
pub struct SquashfsFileWriter<'a, R: ReadSeek> {
    pub header: NodeHeader,
    pub reader: SquashfsFileSource<'a, R>,
}

impl<'a, R: ReadSeek> fmt::Debug for SquashfsFileWriter<'a, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileWriter")
            .field("header", &self.header)
            .finish()
    }
}
pub enum SquashfsFileSource<'a, R: ReadSeek> {
    UserDefined(RefCell<Box<dyn Read + 'a>>),
    SquashfsFile(FilesystemReaderFile<'a, R>),
}

/// Symlink for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsSymlink {
    pub header: NodeHeader,
    pub link: PathBuf,
}

/// Directory for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsDir {
    pub header: NodeHeader,
}

/// Character Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsCharacterDevice {
    pub header: NodeHeader,
    pub device_number: u32,
}

/// Block Device for filesystem
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsBlockDevice {
    pub header: NodeHeader,
    pub device_number: u32,
}

struct WriterWithOffset<W: WriteSeek> {
    w: W,
    offset: u64,
}
impl<W: WriteSeek> Write for WriterWithOffset<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.w.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.w.flush()
    }
}

impl<W: Write + Seek> Seek for WriterWithOffset<W> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(start) => self.w.seek(SeekFrom::Start(self.offset + start)),
            seek => self.w.seek(seek).map(|x| x - self.offset),
        }
    }
}
