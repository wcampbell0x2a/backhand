//! In-memory representation of SquashFS used for writing to image

use core::fmt;
use std::ffi::OsString;
use std::io::{Cursor, Seek, Write};
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;

use deku::bitvec::{BitVec, Msb0};
use deku::{DekuContainerWrite, DekuWrite};
use tracing::{info, instrument, trace};

use crate::compressor::{CompressionOptions, Compressor};
use crate::data::DataWriter;
use crate::error::SquashfsError;
use crate::inode::{BasicDirectory, BasicFile, BasicSymlink, Inode, InodeHeader, InodeInner};
use crate::metadata::{self, MetadataWriter};
use crate::squashfs::{Id, SuperBlock};
use crate::tree::TreeNode;
use crate::writer::Entry;

/// In-memory representation of a Squashfs Image
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Filesystem {
    pub compressor: Compressor,
    pub compression_options: Option<CompressionOptions>,
    pub id_table: Option<Vec<Id>>,
    /// "/" node
    pub root_inode: SquashfsPath,
    /// All other nodes
    pub nodes: Vec<Node>,
}

impl Filesystem {
    // Create SquashFS file system from each node of Tree
    //
    // This works my recursively creating Inodes and Dirs for each node in the tree. This also
    // keeps track of parent directories by calling this function on all nodes of a dir to get only
    // the nodes, but going into the child dirs in the case that it contains a child dir.
    #[instrument(skip_all)]
    fn write_node(
        tree: &TreeNode,
        child: &TreeNode,
        root_node: &SquashfsPath,
        inode: &mut u32,
        inode_writer: &mut MetadataWriter,
        dir_writer: &mut MetadataWriter,
        data_writer: &mut DataWriter,
        data_start: u32,
        dir_parent_inode: u32,
    ) -> (Vec<Entry>, Vec<(OsString, Node)>, u64) {
        let mut nodes = vec![];
        let mut ret_entries = vec![];
        let mut root_inode = 0;

        // If no children, just return this entry since it doesn't have anything recursive/new
        // directories
        if child.children.is_empty() {
            nodes.push((child.name(), child.node.as_ref().unwrap().clone()));
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
        for (_, child) in child.children.iter() {
            let (mut l_dir_entries, mut l_dir_nodes, _) = Self::write_node(
                tree,
                child,
                root_node,
                inode,
                inode_writer,
                dir_writer,
                data_writer,
                data_start,
                parent_inode,
            );
            child_dir_entries.append(&mut l_dir_entries);
            child_dir_nodes.append(&mut l_dir_nodes);
        }
        write_entries.append(&mut child_dir_entries);

        // write child inodes
        for (name, node) in child_dir_nodes {
            let entry = match node {
                Node::Path(path) => {
                    Self::path(name, path, inode, parent_inode, dir_writer, inode_writer)
                },
                Node::File(file) => Self::file(file, inode, data_writer, data_start, inode_writer),
                Node::Symlink(symlink) => Self::symlink(symlink, inode, inode_writer),
            };
            write_entries.push(entry);
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

        trace!("BEFORE: {:#02x?}", child);
        let offset = inode_writer.uncompressed_bytes.len() as u16;
        let start = inode_writer.metadata_start;
        let entry = Entry {
            start,
            offset,
            inode: parent_inode,
            t: 0x01,
            name_size: child.name().len() as u16 - 1,
            name: child.name().as_bytes().to_vec(),
        };
        trace!("ENTRY: {entry:#02x?}");
        ret_entries.push(entry);

        let path_node = if let Some(node) = tree.node.as_ref() {
            node.expect_path()
        } else {
            root_node
        };

        // write parent_inode
        let dir_inode = Inode {
            id: 0x1,
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

        trace!("[{:?}] entries: {ret_entries:#02x?}", child.name());
        trace!("[{:?}] nodes: {nodes:#02x?}", child.name());
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
            id: 0x1,
            header: InodeHeader {
                inode_number: *inode,
                ..path.header.into()
            },
            inner: InodeInner::BasicDirectory(BasicDirectory {
                block_index,
                link_count: 2,
                //TODO: assume this is empty and use 3?
                file_size: 3,
                block_offset,
                parent_inode,
            }),
        };
        *inode += 1;

        let mut v = BitVec::<u8, Msb0>::new();
        dir_inode.write(&mut v, (0, 0)).unwrap();
        let bytes = v.as_raw_slice().to_vec();
        let start = inode_writer.metadata_start;
        let offset = inode_writer.uncompressed_bytes.len() as u16;
        inode_writer.write_all(&bytes).unwrap();

        let entry = Entry {
            start,
            offset,
            inode: dir_inode.header.inode_number,
            t: 0x01,
            name_size: name.len() as u16 - 1,
            name: name.as_bytes().to_vec(),
        };

        entry
    }

    /// Write data and metadata for file node
    fn file(
        file: SquashfsFile,
        inode: &mut u32,
        data_writer: &mut DataWriter,
        data_start: u32,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let file_size = file.bytes.len() as u32;
        let (blocks_start, block_sizes) = data_writer.add_bytes(&file.bytes);
        let file_inode = Inode {
            id: 0x2,
            header: InodeHeader {
                inode_number: *inode,
                ..file.header.into()
            },
            inner: InodeInner::BasicFile(BasicFile {
                blocks_start: blocks_start + data_start,
                frag_index: 0xffffffff, // <- no fragment
                block_offset: 0x0,      // <- no fragment
                file_size,
                block_sizes,
            }),
        };
        *inode += 1;

        let mut v = BitVec::<u8, Msb0>::new();
        file_inode.write(&mut v, (0, 0)).unwrap();
        let bytes = v.as_raw_slice().to_vec();
        let start = inode_writer.metadata_start;
        let offset = inode_writer.uncompressed_bytes.len() as u16;
        inode_writer.write_all(&bytes).unwrap();

        let file_name = file.path.file_name().unwrap();
        let entry = Entry {
            start,
            offset,
            inode: file_inode.header.inode_number,
            t: 0x02,
            name_size: file_name.len() as u16 - 1,
            name: file_name.as_bytes().to_vec(),
        };

        entry
    }

    /// Write data and metadata for symlink node
    fn symlink(
        symlink: SquashfsSymlink,
        inode: &mut u32,
        inode_writer: &mut MetadataWriter,
    ) -> Entry {
        let link = symlink.link.as_bytes();
        let sym_inode = Inode {
            id: 0x3,
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
        *inode += 1;

        let mut v = BitVec::<u8, Msb0>::new();
        sym_inode.write(&mut v, (0, 0)).unwrap();
        let bytes = v.as_raw_slice().to_vec();
        let start = inode_writer.metadata_start;
        let offset = inode_writer.uncompressed_bytes.len() as u16;
        inode_writer.write_all(&bytes).unwrap();

        let entry = Entry {
            start,
            offset,
            inode: sym_inode.header.inode_number,
            t: 0x03,
            name_size: symlink.original.len() as u16 - 1,
            name: symlink.original.as_bytes().to_vec(),
        };

        entry
    }

    #[instrument(skip_all)]
    pub fn to_bytes(&self) -> Result<Vec<u8>, SquashfsError> {
        let mut superblock = SuperBlock::new(self.compressor);
        trace!("{:#02x?}", self.nodes);
        info!("Creating Tree");
        let tree = TreeNode::from(self);
        info!("Tree Created");

        let mut c = Cursor::new(vec![]);

        let mut data_writer = DataWriter::new(self.compressor, None);
        let mut inode_writer = MetadataWriter::new(self.compressor, None);
        let mut dir_writer = MetadataWriter::new(self.compressor, None);
        // TODO(#24): Add fragment support
        //let mut fragment_writer = MetadataWriter::new(self.compressor, None);
        //let mut fragment_table = vec![];

        // Empty Squashfs
        c.write_all(&[0x00; 96])?;

        info!("Creating Inodes and Dirs");
        let mut inode = 1;
        trace!("TREE: {:#02x?}", tree);
        let (_, _, root_inode) = Self::write_node(
            &tree,
            &tree,
            &self.root_inode,
            &mut inode,
            &mut inode_writer,
            &mut dir_writer,
            &mut data_writer,
            96,
            0,
        );

        superblock.root_inode = root_inode;
        superblock.inode_count = inode;

        info!("Writing Data");
        c.write_all(&data_writer.data_bytes)?;

        info!("Writing Inodes");
        superblock.inode_table = c.position();
        c.write_all(&inode_writer.finalize())?;

        info!("Writing Dirs");
        superblock.dir_table = c.position();
        c.write_all(&dir_writer.finalize())?;

        // TODO(#24): Add fragment support
        //
        // This is written to the position of the dir_table to support older versions of unsquashfs
        // that don't support the empty 0xffff_ffff_ffff_ffff
        superblock.frag_table = c.position();

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
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Node {
    File(SquashfsFile),
    Symlink(SquashfsSymlink),
    Path(SquashfsPath),
}

impl Node {
    pub fn expect_path(&self) -> &SquashfsPath {
        if let Self::Path(path) = self {
            path
        } else {
            panic!("not a path")
        }
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Node::File(_))
    }

    pub fn is_symlink(&self) -> bool {
        matches!(self, Node::Symlink(_))
    }

    pub fn is_path(&self) -> bool {
        matches!(self, Node::Path(_))
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct SquashfsFile {
    pub header: FilesystemHeader,
    pub path: PathBuf,
    // TODO: Maybe hold a reference to a Reader? so that something could be written to disk and read from
    // disk instead of loaded into memory
    pub bytes: Vec<u8>,
}

impl fmt::Debug for SquashfsFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirEntry")
            .field("header", &self.header)
            .field("path", &self.path)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsSymlink {
    pub header: FilesystemHeader,
    pub path: PathBuf,
    pub original: String,
    pub link: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsPath {
    pub header: FilesystemHeader,
    pub path: PathBuf,
}
