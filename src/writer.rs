use std::cmp::Ordering;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Cursor, Seek, Write};
use std::os::unix::prelude::OsStrExt;

use deku::bitvec::{BitVec, Msb0};
use deku::{DekuContainerWrite, DekuWrite};
use tracing::{info, instrument, trace};

use crate::compressor::{self, compress, CompressionOptions, Compressor};
use crate::dir::{Dir, DirEntry};
use crate::error::SquashfsError;
use crate::inode::{BasicDirectory, BasicFile, BasicSymlink, Inode, InodeHeader, InodeInner};
use crate::metadata;
use crate::metadata::METADATA_MAXSIZE;
use crate::squashfs::{Filesystem, Id, Node, SuperBlock};
use crate::tree::TreeNode;

#[derive(Debug)]
struct DataWriter {
    compressor: Compressor,
    compression_options: Option<CompressionOptions>,
    pub(crate) data_bytes: Vec<u8>,
}

impl DataWriter {
    #[instrument(skip_all)]
    pub fn new(compressor: Compressor, compression_options: Option<CompressionOptions>) -> Self {
        Self {
            compressor,
            compression_options,
            data_bytes: vec![],
        }
    }

    // TODO: support fragments
    pub(crate) fn add_bytes(&mut self, bytes: &[u8]) -> (u32, Vec<u32>) {
        // TODO: use const
        let chunks = bytes.chunks(0x20000);

        // only have one chunk, use fragment
        //if chunks.len() == 1 {
        //    todo!();
        //    self.fragment_bytes.append(&mut chunks[0].to_vec());
        //}

        let blocks_start = self.data_bytes.len();
        let mut block_sizes = vec![];
        for chunk in chunks {
            let cb = compress(chunk.to_vec(), self.compressor, &self.compression_options).unwrap();
            block_sizes.push(cb.len() as u32);
            self.data_bytes.write_all(&cb).unwrap();
        }

        (blocks_start as u32, block_sizes)
    }
}

// TODO: add the option of not compressing entires
// TODO: add docs
#[derive(Debug)]
struct MetadataWriter {
    compressor: Compressor,
    compression_options: Option<CompressionOptions>,
    /// Offset from the beginning of the metadata block last written
    pub(crate) metadata_start: u32,
    // All current bytes that are uncompressed
    pub(crate) uncompressed_bytes: Vec<u8>,
    // All current bytes that are compressed
    pub(crate) compressed_bytes: Vec<Vec<u8>>,
}

impl MetadataWriter {
    #[instrument(skip_all)]
    pub fn new(compressor: Compressor, compression_options: Option<CompressionOptions>) -> Self {
        Self {
            compressor,
            compression_options,
            metadata_start: 0,
            uncompressed_bytes: vec![],
            compressed_bytes: vec![],
        }
    }

    // TODO: add docs
    #[instrument(skip_all)]
    pub fn finalize(&mut self) -> Vec<u8> {
        let mut out = vec![];
        for cb in &self.compressed_bytes {
            trace!("len: {:02x?}", cb.len());
            trace!("total: {:02x?}", out.len());
            out.write_all(&(cb.len() as u16).to_le_bytes()).unwrap();
            out.write_all(cb).unwrap();
        }

        let b = compressor::compress(
            self.uncompressed_bytes.clone(),
            self.compressor,
            &self.compression_options,
        )
        .unwrap();

        trace!("len: {:02x?}", b.len());
        trace!("total: {:02x?}", out.len());
        out.write_all(&(b.len() as u16).to_le_bytes()).unwrap();
        out.write_all(&b).unwrap();

        out
    }
}

impl Write for MetadataWriter {
    // TODO: add docs
    #[instrument(skip_all)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // add all of buf into uncompressed
        self.uncompressed_bytes.write_all(buf)?;

        while self.uncompressed_bytes.len() >= METADATA_MAXSIZE {
            trace!("time to compress");
            // "Write" the to the saved metablock
            let b = compressor::compress(
                // TODO use split_at?
                self.uncompressed_bytes[..METADATA_MAXSIZE].to_vec(),
                self.compressor,
                &self.compression_options,
            )
            .unwrap();

            // Metadata len + bytes + last metadata_start
            self.metadata_start += 2 + b.len() as u32;
            trace!("new metadata start: {:#02x?}", self.metadata_start);
            self.uncompressed_bytes = self.uncompressed_bytes[METADATA_MAXSIZE..].to_vec();
            self.compressed_bytes.push(b);
        }
        trace!("LEN: {:02x?}", self.uncompressed_bytes.len());

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct Entry {
    pub start: u32,
    pub offset: u16,
    pub inode: u32,
    pub t: u16,
    pub name_size: u16,
    pub name: Vec<u8>,
}

impl Entry {
    pub fn name(&self) -> String {
        std::str::from_utf8(&self.name).unwrap().to_string()
    }

    pub fn value(v: &[u8]) -> u32 {
        v.iter().map(|a| *a as u32).sum()
    }

    pub fn cmp(a: &[u8], b: &[u8]) -> Ordering {
        if a.len() == a.len() {
            if Self::value(a) > Self::value(b) {
                Ordering::Greater
            } else if Self::value(a) > Self::value(b) {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        } else if a.len() > b.len() {
            Ordering::Greater
        } else if a.len() < b.len() {
            Ordering::Less
        } else {
            panic!();
        }
    }
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("start", &self.start)
            .field("offset", &self.offset)
            .field("inode", &self.inode)
            .field("t", &self.t)
            .field("name_size", &self.name_size)
            .field("name", &self.name())
            .finish()
    }
}

impl Entry {
    fn create_dir(creating_dir: &Vec<&Entry>, start: u32) -> Dir {
        // find lowest inode number
        let mut lowest_inode = None;
        for e in creating_dir {
            if lowest_inode.is_none() {
                lowest_inode = Some(e.inode);
            }
            if let Some(low) = lowest_inode {
                if e.inode < low {
                    lowest_inode = Some(e.inode);
                }
            }
        }

        let mut dir = Dir::new(lowest_inode.unwrap());
        dir.count = creating_dir.len() as u32;
        if dir.count >= 256 {
            panic!("dir.count({}) >= 256:", dir.count);
        }
        dir.start = start;
        for e in creating_dir {
            let new_entry = DirEntry {
                offset: e.offset,
                inode_offset: (e.inode - lowest_inode.unwrap()) as i16,
                t: e.t,
                name_size: e.name_size,
                name: e.name.clone(),
            };
            dir.push(new_entry);
        }

        dir
    }

    /// Create alphabetically sorted entries
    #[instrument(skip_all)]
    fn into_dir(entries: &mut [Entry]) -> Vec<Dir> {
        entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

        let mut dirs = vec![];
        let mut creating_dir = vec![];
        let mut creating_start = entries[0].start;
        let mut iter = entries.iter().peekable();

        while let Some(e) = iter.next() {
            creating_dir.push(e);

            // last entry
            if let Some(next) = &iter.peek() {
                if next.start != creating_start {
                    let dir = Self::create_dir(&creating_dir, creating_start);
                    dirs.push(dir);
                    creating_dir = vec![];
                    creating_start = next.start;
                }
            }
            // last entry
            if iter.peek().is_none() {
                let dir = Self::create_dir(&creating_dir, creating_start);
                dirs.push(dir);
            }
        }

        trace!("DIIIIIIIIIIR: {:#02x?}", dirs);
        dirs
    }
}

impl Filesystem {
    #[instrument(skip_all)]
    fn write_node(
        tree: &TreeNode,
        child: &TreeNode,
        inode: &mut u32,
        inode_writer: &mut MetadataWriter,
        dir_writer: &mut MetadataWriter,
        data_writer: &mut DataWriter,
        data_start: u32,
        dir_parent_inode: u32,
        // TODO: make unowned
    ) -> (Vec<Entry>, Vec<(OsString, Node)>, u64) {
        let mut nodes = vec![];
        let mut ret_entries = vec![];
        let mut root_inode = 0;

        // tree is node
        if let Some(node) = &child.node {
            trace!("node");
            match node {
                Node::Path(path) => {
                    trace!("{path:#?}");
                },
                Node::File(file) => {
                    trace!("{file:#?}");
                },
                Node::Symlink(symlink) => {
                    trace!("{symlink:#?}");
                },
            }
            nodes.push((child.name().clone(), node.clone()));
        } else {
            let mut write_entries = vec![];
            let mut child_dir_entries = vec![];
            let mut child_dir_nodes = vec![];

            // store parent inode
            let parent_inode = *inode;
            *inode += 1;

            // tree has children, this is a Dir
            trace!("children");
            for (_child_name, child) in child.children.iter() {
                let (mut l_dir_entries, mut l_dir_nodes, _) = Self::write_node(
                    tree,
                    child,
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
                match node {
                    Node::Path(path) => {
                        trace!("EMPTY: {path:?}");
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
                        println!("INODE: {dir_inode:#02x?}");
                        *inode += 1;

                        let mut v = BitVec::<Msb0, u8>::new();
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
                        println!("ENTRY: {entry:#02x?}");
                        write_entries.push(entry);
                    },
                    Node::File(file) => {
                        let file_size = file.bytes.len() as u32;
                        // TODO: I guess blocks_start includes the squashfs metadata +
                        // compression_options, so we need to add those
                        trace!("add bytes done yo: {:02x?}", file.bytes.len());
                        let (blocks_start, block_sizes) = data_writer.add_bytes(&file.bytes);
                        trace!("add bytes done yo");
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
                        println!("INODE: {file_inode:#02x?}");
                        *inode += 1;

                        let mut v = BitVec::<Msb0, u8>::new();
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
                        println!("ENTRY: {entry:#02x?}");
                        write_entries.push(entry);
                    },
                    Node::Symlink(symlink) => {
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
                        println!("INODE: {sym_inode:#02x?}");
                        *inode += 1;

                        let mut v = BitVec::<Msb0, u8>::new();
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
                        println!("ENTRY: {entry:#02x?}");
                        write_entries.push(entry);
                    },
                }
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
            // TODO: do i skip this step when making root dir entry? (== "/") ?
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

            // write parent_inode
            info!(inode);
            info!(parent_inode);
            let dir_inode = Inode {
                id: 0x1,
                header: InodeHeader {
                    permissions: 0x1ed,
                    uid: 0x0,
                    gid: 0x0,
                    mtime: 0x634761bb,
                    inode_number: parent_inode,
                },
                inner: InodeInner::BasicDirectory(BasicDirectory {
                    block_index,
                    link_count: 2,
                    file_size: total_size,
                    block_offset,
                    parent_inode: dir_parent_inode,
                }),
            };
            println!("INODE: {dir_inode:#02x?}");

            let mut v = BitVec::<Msb0, u8>::new();
            dir_inode.write(&mut v, (0, 0)).unwrap();
            let bytes = v.as_raw_slice().to_vec();
            inode_writer.write_all(&bytes).unwrap();
            root_inode = ((start as u64) << 16) | ((offset as u64) & 0xffff);
        }
        trace!("[{:?}] RRRRRR entries: {ret_entries:#02x?}", child.name());
        trace!("[{:?}] RRRRRR nodes: {nodes:#02x?}", child.name());
        (ret_entries, nodes, root_inode)
    }

    #[instrument(skip_all)]
    pub fn to_bytes(
        &self,
        compressor: Compressor,
        id_table: Option<Vec<Id>>,
    ) -> Result<Vec<u8>, SquashfsError> {
        let mut superblock = SuperBlock::new(compressor);
        info!("Creating Tree");
        let tree = TreeNode::from(self);
        info!("Tree Created");

        let mut c = Cursor::new(vec![]);

        let mut data_writer = DataWriter::new(compressor, None);
        let mut inode_writer = MetadataWriter::new(compressor, None);
        let mut dir_writer = MetadataWriter::new(compressor, None);
        //let mut fragment_writer = MetadataWriter::new(compressor, None);
        //let mut fragment_table = vec![];

        // Empty Squashfs
        c.write_all(&[0x00; 96])?;

        info!("Creating Inodes and Dirs");
        let mut inode = 1;
        trace!("TREE: {:#02x?}", tree);
        let (_, _, root_inode) = Self::write_node(
            &tree,
            &tree,
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

        info!("Writing Id Lookup Table");
        Self::write_id_table(&mut c, id_table, &mut superblock)?;

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
        id_table: Option<Vec<Id>>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mwriter() {
        let bytes = [0xffu8; METADATA_MAXSIZE - 3];

        let mut mwriter = MetadataWriter::new(Compressor::Xz, None);

        mwriter.write_all(&bytes).unwrap();
        assert_eq!(0, mwriter.metadata_start);
        assert_eq!(bytes, &*mwriter.uncompressed_bytes);
        assert!(mwriter.compressed_bytes.is_empty());

        let bytes = [0x11u8; 6];

        mwriter.write_all(&bytes).unwrap();
        assert_eq!(0x6e, mwriter.metadata_start);
        assert_eq!(bytes[3..], mwriter.uncompressed_bytes);
        assert_eq!(mwriter.compressed_bytes[0].len(), 0x6c);
    }

    #[test]
    fn test_entry() {
        let mut entries = vec![
            Entry {
                start: 0,
                offset: 0x100,
                inode: 1,
                t: 1,
                name_size: 0x01,
                name: [b'a', b'a'].to_vec(),
            },
            Entry {
                start: 1,
                offset: 0x200,
                inode: 6,
                t: 1,
                name_size: 0x01,
                name: [b'z', b'z'].to_vec(),
            },
            Entry {
                start: 1,
                offset: 0x300,
                inode: 5,
                t: 1,
                name_size: 0x01,
                name: [b'b', b'b'].to_vec(),
            },
        ];

        let dir = Entry::into_dir(&mut entries);
        assert_eq!(
            vec![
                Dir {
                    count: 0x0,
                    start: 0x0,
                    inode_num: 0x1,
                    dir_entries: vec![DirEntry {
                        offset: 0x100,
                        inode_offset: 0x0,
                        t: 0x1,
                        name_size: 0x1,
                        name: b"aa".to_vec(),
                    },],
                },
                Dir {
                    count: 0x1,
                    start: 0x1,
                    inode_num: 0x5,
                    dir_entries: vec![
                        DirEntry {
                            offset: 0x300,
                            inode_offset: 0x0,
                            t: 0x1,
                            name_size: 0x1,
                            name: b"bb".to_vec(),
                        },
                        DirEntry {
                            offset: 0x200,
                            inode_offset: 0x1,
                            t: 0x1,
                            name_size: 0x1,
                            name: b"zz".to_vec(),
                        },
                    ],
                },
            ],
            dir
        );
    }
}
