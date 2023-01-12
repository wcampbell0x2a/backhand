//! In-memory representation of SquashFS filesystem tree used for writing to image

use core::fmt;
use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{Cursor, Seek, SeekFrom, Write};
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;

use deku::bitvec::{BitVec, Msb0};
use deku::{DekuContainerWrite, DekuWrite};
use tracing::{info, instrument, trace};

use crate::compressor::{CompressionOptions, Compressor};
use crate::data::{Added, DataWriter};
use crate::entry::Entry;
use crate::error::SquashfsError;
use crate::fragment::Fragment;
use crate::inode::{
    BasicDeviceSpecialFile, BasicDirectory, BasicFile, BasicSymlink, Inode, InodeHeader, InodeId,
    InodeInner,
};
use crate::metadata::{self, MetadataWriter};
use crate::reader::ReadSeek;
use crate::squashfs::{Id, SuperBlock};
use crate::tree::TreeNode;
use crate::Squashfs;

/// In-memory representation of a Squashfs image with extracted files and other information needed
/// to create an on-disk image
#[derive(Debug)]
pub struct Filesystem {
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
    pub root_inode: SquashfsPath,
    /// All files and directories in filesystem. This will be convert into a filesystem tree with [`Filesystem::to_bytes`]
    pub nodes: Vec<Node>,
}

impl Filesystem {
    /// First call `Squashfs::from_reader(..)`, then call `Squashfs::into_filesystem(..)`
    pub fn from_reader<R: ReadSeek + 'static>(reader: R) -> Result<Self, SquashfsError> {
        let squashfs = Squashfs::from_reader(reader)?;
        squashfs.into_filesystem()
    }

    /// Same as `from_reader`, but with a starting `offset` to the image in the `reader`
    pub fn from_reader_with_offset<R: ReadSeek + 'static>(
        reader: R,
        offset: u64,
    ) -> Result<Self, SquashfsError> {
        let squashfs = Squashfs::from_reader_with_offset(reader, offset)?;
        squashfs.into_filesystem()
    }

    /// Insert file `bytes` into `path` with metadata `header`.
    ///
    /// This will create dirs for every directory in the path that doesn't exist in `nodes` with
    /// the metadata of `header`.
    pub fn push_file<B: Into<Vec<u8>>, P: Into<PathBuf>>(
        &mut self,
        bytes: B,
        path: P,
        header: FilesystemHeader,
    ) {
        let path = path.into();
        if path.parent().is_some() {
            let mut full_path = "".to_string();
            let components: Vec<_> = path.components().map(|comp| comp.as_os_str()).collect();
            'component: for dir in components.iter().take(components.len() - 1) {
                // add to path
                full_path.push('/');
                full_path.push_str(dir.to_str().unwrap());

                // check if exists
                for node in &mut self.nodes {
                    if let InnerNode::Path(_) = &node.inner {
                        if node.path.as_os_str().to_str() == Some(dir.to_str().unwrap()) {
                            break 'component;
                        }
                    }
                }

                // not found, add to dir
                let new_dir = InnerNode::Path(SquashfsPath { header });
                let node = Node::new(PathBuf::from(full_path.clone()), new_dir);
                self.nodes.push(node);
            }
        }

        let new_file = InnerNode::File(SquashfsFile {
            header,
            reader: RefCell::new(Box::new(Cursor::new(bytes.into()))),
        });
        let node = Node::new(path, new_file);
        self.nodes.push(node);
    }

    /// Take a mutable reference to existing file at `find_path`
    pub fn mut_file<S: Into<PathBuf>>(&mut self, find_path: S) -> Option<&mut SquashfsFile> {
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

    /// Create SquashFS file system from each node of Tree
    ///
    /// This works my recursively creating Inodes and Dirs for each node in the tree. This also
    /// keeps track of parent directories by calling this function on all nodes of a dir to get only
    /// the nodes, but going into the child dirs in the case that it contains a child dir.
    #[instrument(skip_all)]
    fn write_node<'a>(
        tree: &'a TreeNode,
        inode: &mut u32,
        inode_writer: &mut MetadataWriter,
        dir_writer: &mut MetadataWriter,
        data_writer: &mut DataWriter,
        dir_parent_inode: u32,
    ) -> (Vec<Entry>, Vec<(OsString, &'a InnerNode)>, u64) {
        let mut nodes = vec![];
        let mut ret_entries = vec![];
        let mut root_inode = 0;

        // If no children, just return this entry since it doesn't have anything recursive/new
        // directories
        if tree.children.is_empty() {
            nodes.push((tree.name(), tree.node.unwrap()));
            return (ret_entries, nodes, root_inode);
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
            );
            child_dir_entries.append(&mut l_dir_entries);
            child_dir_nodes.append(&mut l_dir_nodes);
        }
        write_entries.append(&mut child_dir_entries);

        // write child inodes
        for (name, node) in child_dir_nodes {
            let node_path = PathBuf::from(name.clone());
            let entry = match node {
                InnerNode::Path(path) => Self::path(
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
                InnerNode::Symlink(symlink) => Self::symlink(symlink, inode, inode_writer),
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
            let bytes = dir.to_bytes().unwrap();
            total_size += bytes.len() as u16;
            dir_writer.write_all(&bytes).unwrap();
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

        let path_node = if let Some(InnerNode::Path(node)) = &tree.node {
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
        dir_inode.write(&mut v, (0, 0)).unwrap();
        let bytes = v.as_raw_slice().to_vec();
        inode_writer.write_all(&bytes).unwrap();
        root_inode = ((start as u64) << 16) | ((offset as u64) & 0xffff);

        trace!("[{:?}] entries: {ret_entries:#02x?}", tree.name());
        trace!("[{:?}] nodes: {nodes:#02x?}", tree.name());
        (ret_entries, nodes, root_inode)
    }

    /// Write data and metadata for path node
    fn path(
        name: OsString,
        path: SquashfsPath,
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
        file: &SquashfsFile,
        inode: &mut u32,
        data_writer: &mut DataWriter,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let mut reader = file.reader.borrow_mut();
        let file_size = reader.seek(SeekFrom::End(0)).unwrap() as u32;
        let added = data_writer.add_bytes(&mut *reader);

        let basic_file = match added {
            Added::Data {
                blocks_start,
                block_sizes,
            } => {
                BasicFile {
                    blocks_start,
                    frag_index: 0xffffffff, // <- no fragment
                    block_offset: 0x0,      // <- no fragment
                    file_size,
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
                file_size,
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
        symlink: &SquashfsSymlink,
        inode: &mut u32,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let link = symlink.link.as_bytes();
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

        sym_inode.to_bytes(symlink.original.as_bytes(), inode_writer)
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

    /// Convert into bytes that can be stored on disk and used as a completed and correct read-only
    /// filesystem. This generates the Superblock with the correct fields from `Filesystem`, and
    /// the data after that contains the nodes.
    #[instrument(skip_all)]
    pub fn to_bytes(&self) -> Result<Vec<u8>, SquashfsError> {
        let mut superblock = SuperBlock::new(self.compressor);

        trace!("{:#02x?}", self.nodes);
        info!("Creating Tree");
        let mut tree = TreeNode::from(self);
        info!("Tree Created");

        let mut c = Cursor::new(vec![]);
        let data_start = 96;

        let mut data_writer = DataWriter::new(self.compressor, None, data_start, self.block_size);
        let mut inode_writer = MetadataWriter::new(self.compressor, None);
        let mut dir_writer = MetadataWriter::new(self.compressor, None);

        // Empty Squashfs
        c.write_all(&vec![0x00; data_start as usize])?;

        info!("Creating Inodes and Dirs");
        let mut inode = 1;

        // Add the "/" entry
        let inner = InnerNode::Path(self.root_inode.clone());
        tree.node = Some(&inner);

        //trace!("TREE: {:#02x?}", tree);
        let (_, _, root_inode) = Self::write_node(
            &tree,
            &mut inode,
            &mut inode_writer,
            &mut dir_writer,
            &mut data_writer,
            0,
        );

        // Compress everything
        data_writer.finalize();

        superblock.root_inode = root_inode;
        superblock.inode_count = inode;
        superblock.block_size = self.block_size;
        superblock.block_log = self.block_log;
        superblock.mod_time = self.mod_time;

        info!("Writing Data");
        c.write_all(&data_writer.data_bytes)?;

        info!("Writing Inodes");
        superblock.inode_table = c.position();
        c.write_all(&inode_writer.finalize())?;

        info!("Writing Dirs");
        superblock.dir_table = c.position();
        c.write_all(&dir_writer.finalize())?;

        info!("Writing Frag Lookup Table");
        Self::write_frag_table(&mut c, data_writer.fragment_table, &mut superblock)?;

        info!("Writing Id Lookup Table");
        Self::write_id_table(&mut c, &self.id_table, &mut superblock)?;

        info!("Finalize Superblock and End Bytes");
        Self::finalize(&mut c, &mut superblock)?;

        info!("Superblock: {:#02x?}", superblock);
        info!("Success");
        Ok(c.into_inner())
    }

    fn finalize(w: &mut Cursor<Vec<u8>>, superblock: &mut SuperBlock) -> Result<(), SquashfsError> {
        // Pad out block_size
        info!("Writing Padding");
        superblock.bytes_used = w.position();
        let blocks_used = superblock.bytes_used as u32 / 0x1000;
        let pad_len = (blocks_used + 1) * 0x1000;
        let pad_len = pad_len - superblock.bytes_used as u32;
        w.write_all(&vec![0x00; pad_len as usize])?;

        // Seek back the beginning and write the superblock
        info!("Writing Superblock");
        trace!("{:#02x?}", superblock);
        w.rewind()?;
        w.write_all(&superblock.to_bytes().unwrap())?;

        info!("Writing Finished");

        Ok(())
    }

    fn write_id_table(
        w: &mut Cursor<Vec<u8>>,
        id_table: &Option<Vec<Id>>,
        write_superblock: &mut SuperBlock,
    ) -> Result<(), SquashfsError> {
        if let Some(id) = id_table {
            let id_table_dat = w.position();
            let bytes: Vec<u8> = id.iter().flat_map(|a| a.to_bytes().unwrap()).collect();
            let metadata_len = metadata::set_if_uncompressed(bytes.len() as u16).to_le_bytes();
            w.write_all(&metadata_len)?;
            w.write_all(&bytes)?;
            write_superblock.id_table = w.position();
            write_superblock.id_count = id.len() as u16;
            w.write_all(&id_table_dat.to_le_bytes())?;
        }

        Ok(())
    }

    fn write_frag_table(
        w: &mut Cursor<Vec<u8>>,
        frag_table: Vec<Fragment>,
        write_superblock: &mut SuperBlock,
    ) -> Result<(), SquashfsError> {
        let frag_table_dat = w.position();
        let bytes: Vec<u8> = frag_table
            .iter()
            .flat_map(|a| a.to_bytes().unwrap())
            .collect();
        let metadata_len = metadata::set_if_uncompressed(bytes.len() as u16).to_le_bytes();
        w.write_all(&metadata_len)?;
        w.write_all(&bytes)?;
        write_superblock.frag_table = w.position();
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

/// Nodes that are converted into filesystem tree during writing to bytes
#[derive(Debug)]
pub struct Node {
    pub path: PathBuf,
    pub inner: InnerNode,
}

impl Node {
    pub fn new(path: PathBuf, inner: InnerNode) -> Self {
        Node { path, inner }
    }
}

#[derive(Debug)]
pub enum InnerNode {
    File(SquashfsFile),
    Symlink(SquashfsSymlink),
    Path(SquashfsPath),
    CharacterDevice(SquashfsCharacterDevice),
    BlockDevice(SquashfsBlockDevice),
}

//#[derive(PartialEq, Eq)]
pub struct SquashfsFile {
    pub header: FilesystemHeader,
    pub reader: RefCell<Box<dyn ReadSeek>>,
}

impl fmt::Debug for SquashfsFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirEntry")
            .field("header", &self.header)
            .field(
                "bytes",
                &self.reader.borrow_mut().seek(SeekFrom::End(0)).unwrap(),
            )
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsSymlink {
    pub header: FilesystemHeader,
    pub original: String,
    pub link: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsPath {
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
