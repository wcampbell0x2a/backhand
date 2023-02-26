use std::cell::RefCell;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use deku::bitvec::BitVec;
use deku::DekuWrite;
use tracing::{error, info, instrument, trace};

use crate::compressor::{CompressionOptions, Compressor};
use crate::data::DataWriter;
use crate::error::SquashfsError;
use crate::filesystem::dummy::DummyReadSeek;
use crate::filesystem::node::SquashfsSymlink;
use crate::fragment::Fragment;
use crate::kind::Kind;
use crate::metadata::{self, MetadataWriter};
use crate::reader::{ReadSeek, WriteSeek};
use crate::squashfs::{Id, SuperBlock};
use crate::tree::TreeNode;
use crate::{
    fragment, FilesystemReader, InnerNode, Node, NodeHeader, SquashfsBlockDevice,
    SquashfsCharacterDevice, SquashfsDir, SquashfsFileSource, SquashfsFileWriter,
};

/// Representation of SquashFS filesystem to be written back to an image
#[derive(Debug)]
pub struct FilesystemWriter<'a, R: ReadSeek = DummyReadSeek> {
    pub kind: Kind,
    /// See [`SuperBlock`].`block_size`
    pub block_size: u32,
    /// See [`SuperBlock`].`block_log`
    pub block_log: u16,
    /// See [`SuperBlock`].`mod_time`
    pub mod_time: u32,
    /// See [`Squashfs`].`id`
    pub id_table: Option<Vec<Id>>,
    /// Information for the `/` node
    pub root_inode: SquashfsDir,
    /// All files and directories in filesystem, including root
    pub nodes: Vec<Node<SquashfsFileWriter<'a, R>>>,
    pub compressor: FilesystemCompressor,
}

impl<'a, R: ReadSeek> FilesystemWriter<'a, R> {
    /// Use the same configuration as an existing `reader`
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
            kind: reader.kind,
            block_size: reader.block_size,
            block_log: reader.block_log,
            compressor: FilesystemCompressor::new(reader.compressor, reader.compression_options)?,
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
        mut header: NodeHeader,
    ) {
        // create uid and replace gid with index
        header.gid = self.lookup_add_id(header.gid as u32);
        // create uid and replace uid with index
        header.uid = self.lookup_add_id(header.uid as u32);
        let path = path.into();

        if path.parent().is_some() {
            let mut full_path = PathBuf::new();
            let components: Vec<_> = path.components().map(|comp| comp.as_os_str()).collect();
            'component: for dir in components.iter().take(components.len() - 1) {
                let dir = Path::new(dir);
                // add to path
                full_path.push(dir);

                // skip root directory
                if dir != Path::new("/") {
                    // check if exists
                    for node in &mut self.nodes {
                        if let InnerNode::Dir(_) = &node.inner {
                            let left = &node.path;
                            let right = &full_path;
                            if left == right {
                                continue 'component;
                            }
                        }
                    }

                    // not found, add to dir
                    let new_dir = InnerNode::Dir(SquashfsDir { header });
                    let node = Node::new(full_path.clone(), new_dir);
                    self.nodes.push(node);
                }
            }
        }

        let reader = RefCell::new(Box::new(reader));
        let new_file = InnerNode::File(SquashfsFileWriter {
            header,
            reader: SquashfsFileSource::UserDefined(reader),
        });
        let node = Node::new(path, new_file);
        self.nodes.push(node);
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
        mut header: NodeHeader,
    ) {
        // create uid and replace uid with index
        header.uid = self.lookup_add_id(header.uid as u32);
        // create uid and replace gid with index
        header.gid = self.lookup_add_id(header.gid as u32);
        let path = path.into();

        let new_symlink = InnerNode::Symlink(SquashfsSymlink {
            header,
            link: link.into(),
        });
        let node = Node::new(path, new_symlink);
        self.nodes.push(node);
    }

    /// Insert empty `dir` at `path`
    pub fn push_dir<P: Into<PathBuf>>(&mut self, path: P, mut header: NodeHeader) {
        // create uid and replace uid with index
        header.uid = self.lookup_add_id(header.uid as u32);
        // create uid and replace gid with index
        header.gid = self.lookup_add_id(header.gid as u32);
        let path = path.into();

        let new_dir = InnerNode::Dir(SquashfsDir { header });
        let node = Node::new(path, new_dir);
        self.nodes.push(node);
    }

    /// Insert character device with `device_number` at `path`
    pub fn push_char_device<P: Into<PathBuf>>(
        &mut self,
        device_number: u32,
        path: P,
        mut header: NodeHeader,
    ) {
        // create uid and replace uid with index
        header.uid = self.lookup_add_id(header.uid as u32);
        // create uid and replace gid with index
        header.gid = self.lookup_add_id(header.gid as u32);
        let path = path.into();

        let new_device = InnerNode::CharacterDevice(SquashfsCharacterDevice {
            header,
            device_number,
        });
        let node = Node::new(path, new_device);
        self.nodes.push(node);
    }

    /// Insert block device with `device_number` at `path`
    pub fn push_block_device<P: Into<PathBuf>>(
        &mut self,
        device_number: u32,
        path: P,
        mut header: NodeHeader,
    ) {
        // create uid and replace uid with index
        header.uid = self.lookup_add_id(header.uid as u32);
        // create uid and replace gid with index
        header.gid = self.lookup_add_id(header.gid as u32);
        let path = path.into();

        let new_device = InnerNode::BlockDevice(SquashfsBlockDevice {
            header,
            device_number,
        });
        let node = Node::new(path, new_device);
        self.nodes.push(node);
    }

    /// Generate the final squashfs file at the offset.
    #[instrument(skip_all)]
    pub fn write_with_offset<W: Write + Seek>(
        &self,
        w: &mut W,
        offset: u64,
    ) -> Result<(), SquashfsError> {
        let mut writer = WriterWithOffset::new(w, offset)?;
        self.write(&mut writer)
    }

    /// Generate the final squashfs file. This generates the Superblock with the
    /// correct fields from `Filesystem`, and the data after that contains the nodes.
    #[instrument(skip_all)]
    pub fn write<W: Write + Seek>(&self, w: &mut W) -> Result<(), SquashfsError> {
        let mut superblock = SuperBlock::new(self.compressor.id, self.kind);

        trace!("{:#02x?}", self.nodes);
        info!("Creating Tree");
        let mut tree: TreeNode<R> = self.into();
        info!("Tree Created");

        // Empty Squashfs Superblock
        w.write_all(&[0x00; 96])?;
        let mut data_writer = DataWriter::new(self.compressor, self.block_size);
        let mut inode_writer = MetadataWriter::new(self.compressor, self.block_size, self.kind);
        let mut dir_writer = MetadataWriter::new(self.compressor, self.block_size, self.kind);

        info!("Creating Inodes and Dirs");
        //trace!("TREE: {:#02x?}", tree);
        info!("Writing Data");
        tree.write_data(self, w, &mut data_writer)?;
        info!("Writing Data Fragments");
        // Compress fragments and write
        data_writer.finalize(w)?;

        info!("Writing Other stuff");
        let (_, root_inode) =
            tree.write_inode_dir(&mut inode_writer, &mut dir_writer, 0, superblock, self.kind)?;

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
        self.write_frag_table(w, data_writer.fragment_table, &mut superblock)?;

        info!("Writing Id Lookup Table");
        self.write_id_table(w, &self.id_table, &mut superblock)?;

        info!("Finalize Superblock and End Bytes");
        self.finalize(w, &mut superblock)?;

        info!("Superblock: {:#02x?}", superblock);
        info!("Success");
        Ok(())
    }

    fn finalize<W: Write + Seek>(
        &self,
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
        let mut bv = BitVec::new();
        superblock.write(&mut bv, self.kind)?;
        w.write_all(bv.as_raw_slice())?;

        info!("Writing Finished");

        Ok(())
    }

    fn write_id_table<W: Write + Seek>(
        &self,
        w: &mut W,
        id_table: &Option<Vec<Id>>,
        write_superblock: &mut SuperBlock,
    ) -> Result<(), SquashfsError> {
        if let Some(id) = id_table {
            let id_table_dat = w.stream_position()?;
            let mut id_bytes = Vec::with_capacity(id.len() * ((u32::BITS / 8) as usize));
            for i in id {
                let mut bv = BitVec::new();
                i.write(&mut bv, self.kind)?;
                id_bytes.write_all(bv.as_raw_slice())?;
            }
            // write metdata_length
            let mut bv = BitVec::new();
            metadata::set_if_uncompressed(id_bytes.len() as u16)
                .write(&mut bv, self.kind.data_endian)?;
            w.write_all(bv.as_raw_slice())?;
            w.write_all(&id_bytes)?;
            write_superblock.id_table = w.stream_position()?;
            write_superblock.id_count = id.len() as u16;

            let mut bv = BitVec::new();
            id_table_dat.write(&mut bv, self.kind.type_endian)?;
            w.write_all(bv.as_raw_slice())?;
        }

        Ok(())
    }

    fn write_frag_table<W: Write + Seek>(
        &self,
        w: &mut W,
        frag_table: Vec<Fragment>,
        write_superblock: &mut SuperBlock,
    ) -> Result<(), SquashfsError> {
        let frag_table_dat = w.stream_position()?;
        let mut frag_bytes = Vec::with_capacity(frag_table.len() * fragment::SIZE);
        for f in &frag_table {
            let mut bv = BitVec::new();
            f.write(&mut bv, self.kind)?;
            frag_bytes.write_all(bv.as_raw_slice())?;
        }
        // write metdata_length
        let mut bv = BitVec::new();
        metadata::set_if_uncompressed(frag_bytes.len() as u16)
            .write(&mut bv, self.kind.data_endian)?;
        w.write_all(bv.as_raw_slice())?;

        w.write_all(&frag_bytes)?;
        write_superblock.frag_table = w.stream_position()?;
        write_superblock.frag_count = frag_table.len() as u32;

        let mut bv = BitVec::new();
        frag_table_dat.write(&mut bv, self.kind.type_endian)?;
        w.write_all(bv.as_raw_slice())?;

        Ok(())
    }

    /// Return index of id, adding if required
    fn lookup_add_id(&mut self, id: u32) -> u16 {
        let found = self
            .id_table
            .as_ref()
            .unwrap()
            .iter()
            .position(|a| a.0 == id);

        match found {
            Some(found) => found as u16,
            None => {
                self.id_table.as_mut().unwrap().push(Id(id));
                self.id_table.as_ref().unwrap().len() as u16 - 1
            },
        }
    }
}

struct WriterWithOffset<W: WriteSeek> {
    w: W,
    offset: u64,
}
impl<W: WriteSeek> WriterWithOffset<W> {
    pub fn new(mut w: W, offset: u64) -> std::io::Result<Self> {
        w.seek(SeekFrom::Start(offset))?;
        Ok(Self { w, offset })
    }
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
        let seek = match pos {
            SeekFrom::Start(start) => SeekFrom::Start(self.offset + start),
            seek => seek,
        };
        self.w.seek(seek).map(|x| x - self.offset)
    }
}

/// All compression options for [`FilesystemWriter`]
#[derive(Debug, Copy, Clone)]
pub struct FilesystemCompressor {
    pub(crate) id: Compressor,
    pub(crate) options: Option<CompressionOptions>,
    pub(crate) extra: Option<CompressionExtra>,
}

impl FilesystemCompressor {
    pub fn new(id: Compressor, options: Option<CompressionOptions>) -> Result<Self, SquashfsError> {
        if matches!(id, Compressor::None) {
            let extra = None;
            return Ok(Self { id, options, extra });
        }

        if matches!(id, Compressor::Gzip)
            && (options.is_none() || matches!(options, Some(CompressionOptions::Gzip(_))))
        {
            let extra = None;
            return Ok(Self { id, options, extra });
        }

        if matches!(id, Compressor::Lzma)
            && (options.is_none() || matches!(options, Some(CompressionOptions::Lzma)))
        {
            let extra = None;
            return Ok(Self { id, options, extra });
        }

        if matches!(id, Compressor::Lzo)
            && (options.is_none() || matches!(options, Some(CompressionOptions::Lzo(_))))
        {
            let extra = None;
            return Ok(Self { id, options, extra });
        }

        if matches!(id, Compressor::Xz)
            && (options.is_none() || matches!(options, Some(CompressionOptions::Xz(_))))
        {
            let extra = None;
            return Ok(Self { id, options, extra });
        }

        if matches!(id, Compressor::Lz4)
            && (options.is_none() || matches!(options, Some(CompressionOptions::Lz4(_))))
        {
            let extra = None;
            return Ok(Self { id, options, extra });
        }

        if matches!(id, Compressor::Zstd)
            && (options.is_none() || matches!(options, Some(CompressionOptions::Zstd(_))))
        {
            let extra = None;
            return Ok(Self { id, options, extra });
        }

        error!("invalid compression settings");
        Err(SquashfsError::InvalidCompressionOption)
    }

    pub fn extra(&mut self, extra: CompressionExtra) -> Result<(), SquashfsError> {
        if matches!(extra, CompressionExtra::Xz(_)) && matches!(self.id, Compressor::Xz) {
            self.extra = Some(extra);
            return Ok(());
        }

        error!("invalid extra compression settings");
        Err(SquashfsError::InvalidCompressionOption)
    }
}

/// Compression options only for [`FilesystemWriter`]
#[derive(Debug, Copy, Clone)]
pub enum CompressionExtra {
    Xz(ExtraXz),
}

/// Xz compression option for [`FilesystemWriter`]
#[derive(Debug, Copy, Clone, Default)]
pub struct ExtraXz {
    pub(crate) level: Option<u32>,
}

impl ExtraXz {
    /// Set compress preset level. Must be in range `0..=9`
    pub fn level(&mut self, level: u32) -> Result<(), SquashfsError> {
        if level > 9 {
            return Err(SquashfsError::InvalidCompressionOption);
        }
        self.level = Some(level);

        Ok(())
    }
}
