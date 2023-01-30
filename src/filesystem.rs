//! In-memory representation of SquashFS filesystem tree used for writing to image

use core::fmt;
use std::cell::RefCell;
use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::prelude::OsStrExt;
use std::path::{Path, PathBuf};

use deku::bitvec::{BitVec, Msb0};
use deku::{DekuContainerWrite, DekuWrite};
use tracing::{info, instrument, trace};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::data::{Added, DataWriter, DATA_STORED_UNCOMPRESSED};
use crate::entry::Entry;
use crate::error::SquashfsError;
use crate::fragment::Fragment;
use crate::inode::{
    BasicDeviceSpecialFile, BasicDirectory, BasicFile, BasicSymlink, Inode, InodeHeader, InodeId,
    InodeInner,
};
use crate::metadata::{self, MetadataWriter};
use crate::reader::{SquashFsReader, SquashfsReaderWithOffset};
use crate::squashfs::{Cache, Id, SuperBlock};
use crate::tree::TreeNode;
use crate::{fragment, Squashfs};

/// In-memory representation of a Squashfs image with extracted files and other information needed
/// to create an on-disk image.
#[derive(Debug)]
pub struct FilesystemReader<R: SquashFsReader> {
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

impl<R: SquashFsReader> FilesystemReader<R> {
    /// Call [`Squashfs::from_reader`], then [`Squashfs::into_filesystem_reader`]
    pub fn from_reader(reader: R) -> Result<Self, SquashfsError> {
        let squashfs = Squashfs::from_reader(reader)?;
        squashfs.into_filesystem_reader()
    }
}

impl<R: SquashFsReader> FilesystemReader<SquashfsReaderWithOffset<R>> {
    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before reading
    pub fn from_reader_with_offset(reader: R, offset: u64) -> Result<Self, SquashfsError> {
        let squashfs = Squashfs::from_reader_with_offset(reader, offset)?;
        squashfs.into_filesystem_reader()
    }
}

impl<R: SquashFsReader> FilesystemReader<R> {
    /// From file details, extract FileBytes
    pub fn file<'a>(&'a self, basic_file: &'a BasicFile) -> impl Read + 'a {
        FilesystemFileReader::new(self, basic_file)
    }

    /// Read and return all the bytes from the file
    pub fn read_file(&self, basic_file: &BasicFile) -> Result<Vec<u8>, SquashfsError> {
        let mut reader = FilesystemFileReader::new(self, basic_file);
        let mut bytes = Vec::with_capacity(basic_file.file_size as usize);
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Read from either Data blocks or Fragments blocks
    fn read_data(&self, size: usize) -> Result<Vec<u8>, SquashfsError> {
        let uncompressed = size & (DATA_STORED_UNCOMPRESSED as usize) != 0;
        let size = size & !(DATA_STORED_UNCOMPRESSED as usize);
        let mut buf = vec![0u8; size];
        self.reader.borrow_mut().read_exact(&mut buf)?;

        let bytes = if uncompressed {
            buf
        } else {
            compressor::decompress(buf, self.compressor)?
        };
        Ok(bytes)
    }
}

struct FilesystemFileReader<'a, R: SquashFsReader> {
    filesystem: &'a FilesystemReader<R>,
    file: &'a BasicFile,
    last_read: Vec<u8>,
    //current block, after all blocks maybe there is a fragment, None is finished
    current_block: Option<usize>,
    bytes_available: usize,
    pos: u64,
}
impl<'a, R: SquashFsReader> FilesystemFileReader<'a, R> {
    pub fn new(filesystem: &'a FilesystemReader<R>, file: &'a BasicFile) -> Self {
        Self {
            filesystem,
            file,
            last_read: vec![],
            current_block: Some(0),
            bytes_available: file.file_size as usize,
            pos: file.blocks_start.into(),
        }
    }

    pub fn read_block(&mut self, block: usize) -> Result<(), SquashfsError> {
        self.current_block = Some(block + 1);
        let block_size = self.file.block_sizes[block];
        self.filesystem
            .reader
            .borrow_mut()
            .seek(SeekFrom::Start(self.pos))?;
        self.last_read = self.filesystem.read_data(block_size as usize)?;
        self.pos = self.filesystem.reader.borrow_mut().stream_position()?;
        Ok(())
    }

    pub fn read_fragment(&mut self) -> Result<(), SquashfsError> {
        self.current_block = None;
        if self.file.frag_index == 0xffffffff {
            return Ok(());
        }
        let fragments = match &self.filesystem.fragments {
            Some(fragments) => fragments,
            None => return Ok(()),
        };
        let frag = fragments[self.file.frag_index as usize];
        // use fragment cache if possible
        let cache = self.filesystem.cache.borrow();
        match cache.fragment_cache.get(&(frag.start)) {
            Some(cache_bytes) => {
                let bytes = &cache_bytes.clone();
                self.last_read = bytes[self.file.block_offset as usize..].to_vec();
            },
            None => {
                self.filesystem
                    .reader
                    .borrow_mut()
                    .seek(SeekFrom::Start(frag.start))?;
                let bytes = self.filesystem.read_data(frag.size as usize)?;
                drop(cache);
                self.filesystem
                    .cache
                    .borrow_mut()
                    .fragment_cache
                    .insert(frag.start, bytes.clone());
                self.last_read = bytes[self.file.block_offset as usize..].to_vec();
            },
        }
        Ok(())
    }

    pub fn read_available(&mut self, buf: &mut [u8]) -> usize {
        let read_len = buf
            .len()
            .min(self.last_read.len())
            .min(self.bytes_available);
        buf[..read_len].copy_from_slice(&self.last_read[..read_len]);
        self.last_read = self.last_read.split_off(read_len);
        self.bytes_available -= read_len;
        read_len
    }
}

impl<'a, R: SquashFsReader> Read for FilesystemFileReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        //if we already read the whole file, then we can return EoF
        if self.bytes_available == 0 {
            return Ok(0);
        }
        //if there is data available from the last read, consume it
        if !self.last_read.is_empty() {
            return Ok(self.read_available(buf));
        }
        //read a block/fragment
        match self.current_block {
            //no more blocks, try a fragment
            Some(block) if block == self.file.block_sizes.len() => self.read_fragment()?,
            //read the block
            Some(block) => self.read_block(block)?,
            //no more data to read, return EoF
            None => return Ok(0),
        }
        //return data from the read block/fragment
        Ok(self.read_available(buf))
    }
}

/// In-memory representation of a Squashfs image with extracted files and other information needed
/// to create an on-disk image. This can be used to create a Squashfs image using
/// [`FilesystemWriter::to_bytes`].
#[derive(Debug)]
pub struct FilesystemWriter<'a> {
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
    /// All files and directories in filesystem
    pub nodes: Vec<Node<SquashfsFileWriter<'a>>>,
}

impl<'a> FilesystemWriter<'a> {
    /// use the same configuration then an existing SquashFsFile
    pub fn from_fs_reader<R: SquashFsReader>(
        reader: &'a FilesystemReader<R>,
    ) -> Result<Self, SquashfsError> {
        let nodes = reader
            .nodes
            .iter()
            .map(|x| {
                let inner = match &x.inner {
                    InnerNode::File(file) => {
                        let reader = reader.file(&file.basic);
                        InnerNode::File(SquashfsFileWriter {
                            header: file.header,
                            reader: RefCell::new(Box::new(reader)),
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
        header: FilesystemHeader,
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
        let new_file = InnerNode::File(SquashfsFileWriter { header, reader });
        let node = Node::new(path, new_file);
        self.nodes.push(node);

        Ok(())
    }

    /// Take a mutable reference to existing file at `find_path`
    pub fn mut_file<S: Into<PathBuf>>(
        &mut self,
        find_path: S,
    ) -> Option<&mut SquashfsFileWriter<'a>> {
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
        file.reader = RefCell::new(Box::new(reader));
        Ok(())
    }

    /// Insert symlink `path` -> `link`
    pub fn push_symlink<P: Into<PathBuf>, S: Into<PathBuf>>(
        &mut self,
        link: S,
        path: P,
        header: FilesystemHeader,
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
        header: FilesystemHeader,
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
        header: FilesystemHeader,
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
        header: FilesystemHeader,
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

    /// Create SquashFS file system from each node of Tree
    ///
    /// This works my recursively creating Inodes and Dirs for each node in the tree. This also
    /// keeps track of parent directories by calling this function on all nodes of a dir to get only
    /// the nodes, but going into the child dirs in the case that it contains a child dir.
    #[instrument(skip_all)]
    #[allow(clippy::type_complexity)]
    fn write_node<'b>(
        tree: &'b TreeNode<'a, 'b>,
        inode: &'_ mut u32,
        inode_writer: &'_ mut MetadataWriter,
        dir_writer: &'_ mut MetadataWriter,
        data_writer: &'_ mut DataWriter,
        dir_parent_inode: u32,
    ) -> Result<
        (
            Vec<Entry>,
            Vec<(&'b OsStr, &'b InnerNode<SquashfsFileWriter<'a>>)>,
            u64,
        ),
        SquashfsError,
    > {
        let mut nodes = vec![];
        let mut ret_entries = vec![];
        let mut root_inode = 0;

        // If no children, just return this entry since it doesn't have anything recursive/new
        // directories
        if tree.children.is_empty() {
            nodes.push((tree.name(), tree.node.unwrap()));
            return Ok((ret_entries, nodes, root_inode));
        }

        // ladies and gentlemen, we have a directory
        let mut write_entries = vec![];
        let mut child_dir_entries = vec![];
        let mut child_dir_nodes = vec![];

        // store parent Inode, this is used for child Dirs, as they will need this to reference
        // back to this
        let parent_inode = *inode;
        *inode += 1;

        // tree has children, this is a Dir, get information of every child node
        for (_, child) in tree.children.iter() {
            let (mut l_dir_entries, mut l_dir_nodes, _) = Self::write_node(
                child,
                inode,
                inode_writer,
                dir_writer,
                data_writer,
                parent_inode,
            )?;
            child_dir_entries.append(&mut l_dir_entries);
            child_dir_nodes.append(&mut l_dir_nodes);
        }
        write_entries.append(&mut child_dir_entries);

        // write child inodes
        for (name, node) in &child_dir_nodes {
            let node_path = PathBuf::from(name);
            let entry = match node {
                InnerNode::Dir(path) => Self::path(
                    name,
                    path.clone(),
                    inode,
                    parent_inode,
                    dir_writer,
                    inode_writer,
                ),
                InnerNode::File(file) => {
                    Self::file(node_path, file, inode, data_writer, inode_writer)
                },
                InnerNode::Symlink(symlink) => {
                    Self::symlink(&node_path, symlink, inode, inode_writer)
                },
                InnerNode::CharacterDevice(char) => {
                    Self::char(node_path, char, inode, inode_writer)
                },
                InnerNode::BlockDevice(block) => {
                    Self::block_device(node_path, block, inode, inode_writer)
                },
            };
            write_entries.push(entry);
            *inode += 1;
        }

        // write dir
        let block_index = dir_writer.metadata_start;
        let block_offset = dir_writer.uncompressed_bytes.len() as u16;
        trace!("WRITING DIR: {block_offset:#02x?}");
        let mut total_size = 3;
        for dir in Entry::into_dir(&mut write_entries) {
            trace!("WRITING DIR: {dir:#02x?}");
            let bytes = dir.to_bytes()?;
            total_size += bytes.len() as u16;
            dir_writer.write_all(&bytes)?;
        }

        //trace!("BEFORE: {:#02x?}", child);
        let offset = inode_writer.uncompressed_bytes.len() as u16;
        let start = inode_writer.metadata_start;
        let entry = Entry {
            start,
            offset,
            inode: parent_inode,
            t: InodeId::BasicDirectory,
            name_size: tree.name().len() as u16 - 1,
            name: tree.name().as_bytes().to_vec(),
        };
        trace!("ENTRY: {entry:#02x?}");
        ret_entries.push(entry);

        let path_node = if let Some(InnerNode::Dir(node)) = &tree.node {
            node.clone()
        } else {
            panic!();
        };

        // write parent_inode
        let dir_inode = Inode {
            id: InodeId::BasicDirectory,
            header: InodeHeader {
                permissions: path_node.header.permissions,
                uid: path_node.header.uid,
                gid: path_node.header.gid,
                mtime: path_node.header.mtime,
                inode_number: parent_inode,
            },
            inner: InodeInner::BasicDirectory(BasicDirectory {
                block_index,
                link_count: 2, // <- TODO: set this
                file_size: total_size,
                block_offset,
                parent_inode: dir_parent_inode,
            }),
        };

        let mut v = BitVec::<u8, Msb0>::new();
        dir_inode.write(&mut v, (0, 0))?;
        let bytes = v.as_raw_slice().to_vec();
        inode_writer.write_all(&bytes)?;
        root_inode = ((start as u64) << 16) | ((offset as u64) & 0xffff);

        trace!("[{:?}] entries: {ret_entries:#02x?}", tree.name());
        trace!("[{:?}] nodes: {nodes:#02x?}", tree.name());
        Ok((ret_entries, nodes, root_inode))
    }

    /// Write data and metadata for path node
    fn path(
        name: &OsStr,
        path: SquashfsDir,
        inode: &mut u32,
        parent_inode: u32,
        dir_writer: &MetadataWriter,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let block_offset = dir_writer.uncompressed_bytes.len() as u16;
        let block_index = dir_writer.metadata_start;
        let dir_inode = Inode {
            id: InodeId::BasicDirectory,
            header: InodeHeader {
                inode_number: *inode,
                ..path.header.into()
            },
            inner: InodeInner::BasicDirectory(BasicDirectory {
                block_index,
                link_count: 2,
                // Empty path
                file_size: 3,
                block_offset,
                parent_inode,
            }),
        };

        dir_inode.to_bytes(name.as_bytes(), inode_writer)
    }

    /// Write data and metadata for file node
    fn file(
        node_path: PathBuf,
        file: &SquashfsFileWriter<'a>,
        inode: &mut u32,
        data_writer: &mut DataWriter,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let (file_size, added) = data_writer.add_bytes(file.reader.borrow_mut().as_mut());

        let basic_file = match added {
            Added::Data {
                blocks_start,
                block_sizes,
            } => {
                BasicFile {
                    blocks_start,
                    frag_index: 0xffffffff, // <- no fragment
                    block_offset: 0x0,      // <- no fragment
                    file_size: file_size.try_into().unwrap(),
                    block_sizes,
                }
            },
            Added::Fragment {
                frag_index,
                block_offset,
            } => BasicFile {
                blocks_start: 0,
                frag_index,
                block_offset,
                file_size: file_size.try_into().unwrap(),
                block_sizes: vec![],
            },
        };

        let file_inode = Inode {
            id: InodeId::BasicFile,
            header: InodeHeader {
                inode_number: *inode,
                ..file.header.into()
            },
            inner: InodeInner::BasicFile(basic_file),
        };

        let file_name = node_path.file_name().unwrap();
        file_inode.to_bytes(file_name.as_bytes(), inode_writer)
    }

    /// Write data and metadata for symlink node
    fn symlink(
        path: &Path,
        symlink: &SquashfsSymlink,
        inode: &mut u32,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let link = symlink.link.as_os_str().as_bytes();
        let sym_inode = Inode {
            id: InodeId::BasicSymlink,
            header: InodeHeader {
                inode_number: *inode,
                ..symlink.header.into()
            },
            inner: InodeInner::BasicSymlink(BasicSymlink {
                link_count: 0x1,
                target_size: link.len() as u32,
                target_path: link.to_vec(),
            }),
        };

        let name = path.file_name().unwrap().to_str().unwrap();
        sym_inode.to_bytes(name.as_bytes(), inode_writer)
    }

    /// Write data and metadata for char device node
    fn char(
        node_path: PathBuf,
        char_device: &SquashfsCharacterDevice,
        inode: &mut u32,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let char_inode = Inode {
            id: InodeId::BasicCharacterDevice,
            header: InodeHeader {
                inode_number: *inode,
                ..char_device.header.into()
            },
            inner: InodeInner::BasicCharacterDevice(BasicDeviceSpecialFile {
                link_count: 0x1,
                device_number: char_device.device_number,
            }),
        };

        let name = node_path.file_name().unwrap().to_str().unwrap();
        char_inode.to_bytes(name.as_bytes(), inode_writer)
    }

    /// Write data and metadata for block device node
    fn block_device(
        node_path: PathBuf,
        block_device: &SquashfsBlockDevice,
        inode: &mut u32,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let block_inode = Inode {
            id: InodeId::BasicBlockDevice,
            header: InodeHeader {
                inode_number: *inode,
                ..block_device.header.into()
            },
            inner: InodeInner::BasicBlockDevice(BasicDeviceSpecialFile {
                link_count: 0x1,
                device_number: block_device.device_number,
            }),
        };

        let name = node_path.file_name().unwrap().to_str().unwrap();
        block_inode.to_bytes(name.as_bytes(), inode_writer)
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
        let mut tree = TreeNode::from(self);
        info!("Tree Created");

        let data_start = 96;

        let mut data_writer = DataWriter::new(self.compressor, None, data_start, self.block_size);
        let mut inode_writer = MetadataWriter::new(self.compressor, None, self.block_size);
        let mut dir_writer = MetadataWriter::new(self.compressor, None, self.block_size);

        // Empty Squashfs
        w.write_all(&vec![0x00; data_start as usize])?;

        info!("Creating Inodes and Dirs");
        let mut inode = 1;

        // Add the "/" entry
        let inner = InnerNode::Dir(self.root_inode.clone());
        tree.node = Some(&inner);

        //trace!("TREE: {:#02x?}", tree);
        let (_, _, root_inode) = Self::write_node(
            &tree,
            &mut inode,
            &mut inode_writer,
            &mut dir_writer,
            &mut data_writer,
            0,
        )?;

        // Compress everything
        data_writer.finalize();

        superblock.root_inode = root_inode;
        superblock.inode_count = inode - 1;
        superblock.block_size = self.block_size;
        superblock.block_log = self.block_log;
        superblock.mod_time = self.mod_time;

        info!("Writing Data");
        w.write_all(&data_writer.data_bytes)?;

        info!("Writing Inodes");
        superblock.inode_table = w.stream_position()?;
        w.write_all(&inode_writer.finalize())?;

        info!("Writing Dirs");
        superblock.dir_table = w.stream_position()?;
        w.write_all(&dir_writer.finalize())?;

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

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct FilesystemHeader {
    pub permissions: u16,
    pub uid: u16,
    pub gid: u16,
    pub mtime: u32,
}

impl From<InodeHeader> for FilesystemHeader {
    fn from(inode_header: InodeHeader) -> Self {
        Self {
            permissions: inode_header.permissions,
            uid: inode_header.uid,
            gid: inode_header.gid,
            mtime: inode_header.mtime,
        }
    }
}

/// Nodes from an existing file that are converted into filesystem tree during writing to bytes
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerNode<T> {
    File(T),
    Symlink(SquashfsSymlink),
    Dir(SquashfsDir),
    CharacterDevice(SquashfsCharacterDevice),
    BlockDevice(SquashfsBlockDevice),
}

/// Unread file
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsFileReader {
    pub header: FilesystemHeader,
    pub basic: BasicFile,
}

/// Read file
pub struct SquashfsFileWriter<'a> {
    pub header: FilesystemHeader,
    pub reader: RefCell<Box<dyn Read + 'a>>,
}

impl<'a> fmt::Debug for SquashfsFileWriter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirEntry")
            .field("header", &self.header)
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsSymlink {
    pub header: FilesystemHeader,
    pub link: PathBuf,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsDir {
    pub header: FilesystemHeader,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsCharacterDevice {
    pub header: FilesystemHeader,
    pub device_number: u32,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsBlockDevice {
    pub header: FilesystemHeader,
    pub device_number: u32,
}

struct WriterWithOffset<W: Write + Seek> {
    w: W,
    offset: u64,
}
impl<W: Write + Seek> Write for WriterWithOffset<W> {
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
