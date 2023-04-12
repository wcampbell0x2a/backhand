use std::cell::RefCell;
use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use deku::bitvec::BitVec;
use deku::DekuWrite;
use tracing::{error, info, instrument, trace};

use super::node::{InnerNode, Nodes};
use super::normalize_squashfs_path;
use crate::compressor::{CompressionOptions, Compressor};
use crate::data::DataWriter;
use crate::entry::Entry;
use crate::error::BackhandError;
use crate::filesystem::node::SquashfsSymlink;
use crate::fragment::Fragment;
use crate::kind::Kind;
use crate::metadata::{self, MetadataWriter};
use crate::reader::WriteSeek;
use crate::squashfs::{Id, SuperBlock};
//use crate::tree::TreeNode;
use crate::{
    fragment, FilesystemReader, Node, NodeHeader, SquashfsBlockDevice, SquashfsCharacterDevice,
    SquashfsDir, SquashfsFileWriter, DEFAULT_BLOCK_SIZE, DEFAULT_PAD_LEN, MAX_BLOCK_SIZE,
    MIN_BLOCK_SIZE,
};

/// Representation of SquashFS filesystem to be written back to an image
/// - Use [`Self::from_fs_reader`] to write with the data from a previous SquashFS image
/// - Use [`Self::default`] to create an empty SquashFS image without an original image. For example:
/// ```rust
/// # use std::time::SystemTime;
/// # use backhand::{NodeHeader, Id, FilesystemCompressor, FilesystemWriter, SquashfsDir, compression::Compressor, kind, DEFAULT_BLOCK_SIZE, ExtraXz, CompressionExtra};
/// // Add empty default FilesytemWriter
/// let mut fs = FilesystemWriter::default();
/// fs.set_current_time();
/// fs.set_block_size(DEFAULT_BLOCK_SIZE);
/// fs.set_only_root_id();
/// fs.set_kind(kind::LE_V4_0);
///
/// // set root image permissions
/// let header = NodeHeader {
///     permissions: 0o755,
///     ..NodeHeader::default()
/// };
/// fs.set_root_mode(0o777);
///
/// // set extra compression options
/// let mut xz_extra = ExtraXz::default();
/// xz_extra.level(9).unwrap();
/// let extra = CompressionExtra::Xz(xz_extra);
/// let mut compressor = FilesystemCompressor::new(Compressor::Xz, None).unwrap();
/// compressor.extra(extra).unwrap();
/// fs.set_compressor(compressor);
///
/// // push some dirs and a file
/// fs.push_dir("usr", header);
/// fs.push_dir("usr/bin", header);
/// fs.push_file(std::io::Cursor::new(vec![0x00, 0x01]), "usr/bin/file", header);
/// ```
#[derive(Debug)]
pub struct FilesystemWriter<'a> {
    pub(crate) kind: Kind,
    /// The size of a data block in bytes. Must be a power of two between 4096 (4k) and 1048576 (1 MiB).
    pub(crate) block_size: u32,
    /// Last modification time of the archive. Count seconds since 00:00, Jan 1st 1970 UTC (not counting leap seconds).
    /// This is unsigned, so it expires in the year 2106 (as opposed to 2038).
    pub(crate) mod_time: u32,
    /// 32 bit user and group IDs
    pub(crate) id_table: Vec<Id>,
    /// Compressor used when writing
    pub(crate) compressor: FilesystemCompressor,
    /// All files and directories in filesystem, including root
    pub(crate) root: Nodes<SquashfsFileWriter<'a>>,
    /// The log2 of the block size. If the two fields do not agree, the archive is considered corrupted.
    pub(crate) block_log: u16,
    pub(crate) pad_len: u32,
}

impl Default for FilesystemWriter<'_> {
    /// Create default FilesystemWriter
    ///
    /// block_size: [`DEFAULT_BLOCK_SIZE`], compressor: default XZ compression, no nodes,
    /// kind: [`Kind::default()`], and mod_time: `0`.
    fn default() -> Self {
        let block_size = DEFAULT_BLOCK_SIZE;
        Self {
            block_size,
            mod_time: 0,
            id_table: vec![],
            compressor: FilesystemCompressor::default(),
            kind: Kind::default(),
            root: Nodes::new_root(NodeHeader::default()),
            block_log: (block_size as f32).log2() as u16,
            pad_len: DEFAULT_PAD_LEN,
        }
    }
}

impl<'a> FilesystemWriter<'a> {
    /// Set block size
    ///
    /// # Panics
    /// If invalid, must be [`MIN_BLOCK_SIZE`] `> block_size <` [`MAX_BLOCK_SIZE`]
    pub fn set_block_size(&mut self, block_size: u32) {
        if !(MIN_BLOCK_SIZE..=MAX_BLOCK_SIZE).contains(&block_size) {
            panic!("invalid block_size");
        }
        self.block_size = block_size;
        self.block_log = (block_size as f32).log2() as u16;
    }

    /// Set time of image as `mod_time`
    ///
    /// # Example: Set to `Wed Oct 19 01:26:15 2022`
    /// ```rust
    /// # use backhand::{FilesystemWriter, kind};
    /// let mut fs = FilesystemWriter::default();
    /// fs.set_time(0x634f_5237);
    /// ```
    pub fn set_time(&mut self, mod_time: u32) {
        self.mod_time = mod_time;
    }

    /// Set time of image as current time
    pub fn set_current_time(&mut self) {
        self.mod_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
    }

    /// Set kind as `kind`
    ///
    /// # Example: Set kind to default V4.0
    /// ```rust
    /// # use backhand::{FilesystemWriter, kind};
    /// let mut fs = FilesystemWriter::default();
    /// fs.set_kind(kind::LE_V4_0);
    /// ```
    pub fn set_kind(&mut self, kind: Kind) {
        self.kind = kind;
    }

    /// Set root mode as `mode`
    ///
    /// # Example
    ///```rust
    /// # use backhand::FilesystemWriter;
    /// let mut fs = FilesystemWriter::default();
    /// fs.set_root_mode(0o777);
    /// ```
    pub fn set_root_mode(&mut self, mode: u16) {
        self.root.root_mut().header.permissions = mode;
    }

    /// Set root uid as `uid`
    pub fn set_root_uid(&mut self, uid: u16) {
        self.root.root_mut().header.uid = uid;
    }

    /// Set root gid as `gid`
    pub fn set_root_gid(&mut self, gid: u16) {
        self.root.root_mut().header.gid = gid;
    }

    /// Set compressor as `compressor`
    ///
    ///```rust
    /// # use backhand::{FilesystemWriter, FilesystemCompressor, compression::Compressor};
    /// let mut compressor = FilesystemCompressor::new(Compressor::Xz, None).unwrap();
    /// ```
    pub fn set_compressor(&mut self, compressor: FilesystemCompressor) {
        self.compressor = compressor;
    }

    /// Set id_table to `Id::root()`, removing old entries
    pub fn set_only_root_id(&mut self) {
        self.id_table = Id::root();
    }

    /// Set padding(zero bytes) added to the end of the image after calling [`write`].
    ///
    /// For example, if given `pad_kib` of 8; a 8K padding will be added to the end of the image.
    ///
    /// Default: [`DEFAULT_PAD_LEN`]
    pub fn set_kib_padding(&mut self, pad_kib: u32) {
        self.pad_len = pad_kib * 1024;
    }

    /// Set *no* padding(zero bytes) added to the end of the image after calling [`write`].
    pub fn set_no_padding(&mut self) {
        self.pad_len = 0;
    }

    /// Inherit filesystem structure and properties from `reader`
    pub fn from_fs_reader(reader: &'a FilesystemReader) -> Result<Self, BackhandError> {
        let mut root: Vec<Node<_>> = reader
            .root
            .nodes
            .iter()
            .map(|node| {
                let inner = match &node.inner {
                    InnerNode::File(file) => {
                        let reader = reader.file(&file.basic);
                        InnerNode::File(SquashfsFileWriter::SquashfsFile(reader))
                    },
                    InnerNode::Symlink(x) => InnerNode::Symlink(x.clone()),
                    InnerNode::Dir(x) => InnerNode::Dir(*x),
                    InnerNode::CharacterDevice(x) => InnerNode::CharacterDevice(*x),
                    InnerNode::BlockDevice(x) => InnerNode::BlockDevice(*x),
                };
                Node {
                    fullpath: node.fullpath.clone(),
                    header: node.header,
                    inner,
                }
            })
            .collect();
        root.sort();
        Ok(Self {
            kind: reader.kind,
            block_size: reader.block_size,
            block_log: reader.block_log,
            compressor: FilesystemCompressor::new(reader.compressor, reader.compression_options)?,
            mod_time: reader.mod_time,
            id_table: reader.id_table.clone(),
            root: Nodes { nodes: root },
            pad_len: DEFAULT_PAD_LEN,
        })
    }

    //find the node relative to this path and return a mutable reference
    fn mut_node<S: AsRef<Path>>(
        &mut self,
        find_path: S,
    ) -> Option<&mut Node<SquashfsFileWriter<'a>>> {
        //the search path root prefix is optional, so remove it if present to
        //not affect the search
        let find_path = normalize_squashfs_path(find_path.as_ref()).ok()?;
        self.root.node_mut(find_path)
    }

    fn insert_node<P: AsRef<Path>>(
        &mut self,
        path: P,
        mut header: NodeHeader,
        node: InnerNode<SquashfsFileWriter<'a>>,
    ) -> Result<(), BackhandError> {
        // create uid and replace gid with index
        header.gid = self.lookup_add_id(header.gid as u32);
        // create uid and replace uid with index
        header.uid = self.lookup_add_id(header.uid as u32);

        let path = normalize_squashfs_path(path.as_ref())?;
        let node = Node::new(path, header, node);
        self.root.insert(node)
    }

    /// Insert `reader` into filesystem with `path` and metadata `header`.
    ///
    /// The `uid` and `guid` in `header` are added to FilesystemWriters id's
    pub fn push_file<P: AsRef<Path>>(
        &mut self,
        reader: impl Read + 'a,
        path: P,
        header: NodeHeader,
    ) -> Result<(), BackhandError> {
        let reader = RefCell::new(Box::new(reader));
        let new_file = InnerNode::File(SquashfsFileWriter::UserDefined(reader));
        self.insert_node(path, header, new_file)?;
        Ok(())
    }

    /// Take a mutable reference to existing file at `find_path`
    pub fn mut_file<S: AsRef<Path>>(
        &mut self,
        find_path: S,
    ) -> Option<&mut SquashfsFileWriter<'a>> {
        self.mut_node(find_path).and_then(|node| {
            if let InnerNode::File(file) = &mut node.inner {
                Some(file)
            } else {
                None
            }
        })
    }

    /// Replace an existing file
    pub fn replace_file<S: AsRef<Path>>(
        &mut self,
        find_path: S,
        reader: impl Read + 'a,
    ) -> Result<(), BackhandError> {
        let file = self
            .mut_file(find_path)
            .ok_or(BackhandError::FileNotFound)?;
        *file = SquashfsFileWriter::UserDefined(RefCell::new(Box::new(reader)));
        Ok(())
    }

    /// Insert symlink `path` -> `link`
    ///
    /// The `uid` and `guid` in `header` are added to FilesystemWriters id's
    pub fn push_symlink<P: AsRef<Path>, S: Into<PathBuf>>(
        &mut self,
        link: S,
        path: P,
        header: NodeHeader,
    ) -> Result<(), BackhandError> {
        let new_symlink = InnerNode::Symlink(SquashfsSymlink { link: link.into() });
        self.insert_node(path, header, new_symlink)?;
        Ok(())
    }

    /// Insert empty `dir` at `path`
    ///
    /// The `uid` and `guid` in `header` are added to FilesystemWriters id's
    pub fn push_dir<P: AsRef<Path>>(
        &mut self,
        path: P,
        header: NodeHeader,
    ) -> Result<(), BackhandError> {
        let new_dir = InnerNode::Dir(SquashfsDir::default());
        self.insert_node(path, header, new_dir)?;
        Ok(())
    }

    /// Recursively create an empty directory and all of its parent components
    /// if they are missing.
    ///
    /// The `uid` and `guid` in `header` are added to FilesystemWriters id's
    pub fn push_dir_all<P: AsRef<Path>>(
        &mut self,
        path: P,
        header: NodeHeader,
    ) -> Result<(), BackhandError> {
        //the search path root prefix is optional, so remove it if present to
        //not affect the search
        let path = normalize_squashfs_path(path.as_ref())?;
        //TODO this is not elegant, find a better solution
        let ancestors: Vec<&Path> = path.ancestors().collect();

        for file in ancestors.iter().rev() {
            match self
                .root
                .nodes
                .binary_search_by(|node| node.fullpath.as_path().cmp(file))
            {
                Ok(index) => {
                    //if exists, but is not a directory, return an error
                    let node = &self.root.nodes[index];
                    if !matches!(&node.inner, InnerNode::Dir(_)) {
                        return Err(BackhandError::InvalidFilePath);
                    }
                },
                //if the dir don't exists, create it
                Err(index) => {
                    self.root.nodes.insert(
                        index,
                        Node::new(
                            file.to_path_buf(),
                            header,
                            InnerNode::Dir(SquashfsDir::default()),
                        ),
                    );
                },
            }
        }
        Ok(())
    }

    /// Insert character device with `device_number` at `path`
    ///
    /// The `uid` and `guid` in `header` are added to FilesystemWriters id's
    pub fn push_char_device<P: AsRef<Path>>(
        &mut self,
        device_number: u32,
        path: P,
        header: NodeHeader,
    ) -> Result<(), BackhandError> {
        let new_device = InnerNode::CharacterDevice(SquashfsCharacterDevice { device_number });
        self.insert_node(path, header, new_device)?;
        Ok(())
    }

    /// Insert block device with `device_number` at `path`
    ///
    /// The `uid` and `guid` in `header` are added to FilesystemWriters id's
    pub fn push_block_device<P: AsRef<Path>>(
        &mut self,
        device_number: u32,
        path: P,
        header: NodeHeader,
    ) -> Result<(), BackhandError> {
        let new_device = InnerNode::BlockDevice(SquashfsBlockDevice { device_number });
        self.insert_node(path, header, new_device)?;
        Ok(())
    }

    /// Same as [`Self::write`], but seek'ing to `offset` in `w` before reading. This offset
    /// is treated as the base image offset.
    #[instrument(skip_all)]
    pub fn write_with_offset<W: Write + Seek>(
        &mut self,
        w: &mut W,
        offset: u64,
    ) -> Result<(SuperBlock, u64), BackhandError> {
        let mut writer = WriterWithOffset::new(w, offset)?;
        self.write(&mut writer)
    }

    fn write_data<W: WriteSeek>(
        &mut self,
        compressor: FilesystemCompressor,
        block_size: u32,
        writer: &mut W,
        data_writer: &mut DataWriter,
    ) -> Result<(), BackhandError> {
        let files = self
            .root
            .nodes
            .iter_mut()
            .filter_map(|node| match &mut node.inner {
                InnerNode::File(file) => Some(file),
                _ => None,
            });
        for file in files {
            let (filesize, added) = match &file {
                SquashfsFileWriter::UserDefined(file) => {
                    data_writer.add_bytes(file.borrow_mut().as_mut(), writer)?
                },
                SquashfsFileWriter::SquashfsFile(file) => {
                    // if the source file and the destination files are both
                    // squashfs files and use the same compressor and block_size
                    // just copy the data, don't compress->decompress
                    if file.system.compressor == compressor.id
                        && file.system.compression_options == compressor.options
                        && file.system.block_size == block_size
                    {
                        data_writer.just_copy_it(file.raw_data_reader(), writer)?
                    } else {
                        let mut buf_read = Vec::with_capacity(file.system.block_size as usize);
                        let mut buf_decompress =
                            Vec::with_capacity(file.system.block_size as usize);
                        data_writer
                            .add_bytes(file.reader(&mut buf_read, &mut buf_decompress), writer)?
                    }
                },
                SquashfsFileWriter::Consumed(_, _) => unreachable!(),
            };
            *file = SquashfsFileWriter::Consumed(filesize, added);
        }
        Ok(())
    }

    /// Create SquashFS file system from each node of Tree
    ///
    /// This works my recursively creating Inodes and Dirs for each node in the tree. This also
    /// keeps track of parent directories by calling this function on all nodes of a dir to get only
    /// the nodes, but going into the child dirs in the case that it contains a child dir.
    fn write_inode_dir<'b>(
        &'b self,
        inode_writer: &'_ mut MetadataWriter,
        dir_writer: &'_ mut MetadataWriter,
        parent_node_id: u32,
        node_id: NonZeroUsize,
        superblock: &SuperBlock,
        kind: Kind,
    ) -> Result<Entry<'b>, BackhandError> {
        let node = &self.root.node(node_id).unwrap();
        let filename = node.fullpath.file_name().unwrap_or(OsStr::new("/"));
        //if not a dir, return the entry
        match &node.inner {
            InnerNode::File(SquashfsFileWriter::Consumed(filesize, added)) => {
                return Ok(Entry::file(
                    filename,
                    node.header,
                    node_id.get().try_into().unwrap(),
                    inode_writer,
                    *filesize,
                    added,
                    superblock,
                    kind,
                ))
            },
            InnerNode::File(_) => unreachable!(),
            InnerNode::Symlink(symlink) => {
                return Ok(Entry::symlink(
                    filename,
                    node.header,
                    symlink,
                    node_id.get().try_into().unwrap(),
                    inode_writer,
                    superblock,
                    kind,
                ))
            },
            InnerNode::CharacterDevice(char) => {
                return Ok(Entry::char(
                    filename,
                    node.header,
                    char,
                    node_id.get().try_into().unwrap(),
                    inode_writer,
                    superblock,
                    kind,
                ))
            },
            InnerNode::BlockDevice(block) => {
                return Ok(Entry::block_device(
                    filename,
                    node.header,
                    block,
                    node_id.get().try_into().unwrap(),
                    inode_writer,
                    superblock,
                    kind,
                ))
            },
            InnerNode::Dir(_) => {},
        };

        // ladies and gentlemen, we have a directory
        let entries: Vec<_> = self
            .root
            .children_of(node_id)
            //only direct children
            .filter(|(_child_id, child)| {
                child
                    .fullpath
                    .parent()
                    .map(|child| child == node.fullpath)
                    .unwrap_or(false)
            })
            .map(|(child_id, _child)| {
                self.write_inode_dir(
                    inode_writer,
                    dir_writer,
                    node_id.get().try_into().unwrap(),
                    child_id,
                    superblock,
                    kind,
                )
            })
            .collect::<Result<_, _>>()?;

        // write dir
        let block_index = dir_writer.metadata_start;
        let block_offset = dir_writer.uncompressed_bytes.len() as u16;
        trace!("WRITING DIR: {block_offset:#02x?}");
        let mut total_size: usize = 3;
        for dir in Entry::into_dir(entries) {
            trace!("WRITING DIR: {dir:#02x?}");

            let mut bv = BitVec::new();
            dir.write(&mut bv, kind)?;
            let bytes = bv.as_raw_slice();
            dir_writer.write_all(bv.as_raw_slice())?;

            total_size += bytes.len();
        }
        let entry = Entry::path(
            filename,
            node.header,
            node_id.get().try_into().unwrap(),
            parent_node_id,
            inode_writer,
            total_size.try_into().unwrap(),
            block_offset,
            block_index,
            superblock,
            kind,
        );
        trace!("[{:?}] entries: {:#02x?}", filename, &entry);
        Ok(entry)
    }

    /// Generate and write the resulting squashfs image to `w`
    ///
    /// # Returns
    /// (written populated [`SuperBlock`], total amount of bytes written including padding)
    #[instrument(skip_all)]
    pub fn write<W: Write + Seek>(
        &mut self,
        w: &mut W,
    ) -> Result<(SuperBlock, u64), BackhandError> {
        let mut superblock = SuperBlock::new(self.compressor.id, self.kind);

        trace!("{:#02x?}", self.root);

        // Empty Squashfs Superblock
        w.write_all(&[0x00; 96])?;
        let mut data_writer = DataWriter::new(self.compressor, self.block_size);
        let mut inode_writer = MetadataWriter::new(self.compressor, self.block_size, self.kind);
        let mut dir_writer = MetadataWriter::new(self.compressor, self.block_size, self.kind);

        info!("Creating Inodes and Dirs");
        //trace!("TREE: {:#02x?}", &self.root);
        info!("Writing Data");
        self.write_data(self.compressor, self.block_size, w, &mut data_writer)?;
        info!("Writing Data Fragments");
        // Compress fragments and write
        data_writer.finalize(w)?;

        info!("Writing Other stuff");
        let root = self.write_inode_dir(
            &mut inode_writer,
            &mut dir_writer,
            0,
            1.try_into().unwrap(),
            &superblock,
            self.kind,
        )?;

        superblock.root_inode = ((root.start as u64) << 16) | ((root.offset as u64) & 0xffff);
        superblock.inode_count = self.root.nodes.len() as u32;
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
        let bytes_written = self.finalize(w, &mut superblock)?;

        info!("Superblock: {:#02x?}", superblock);
        info!("Success");
        Ok((superblock, bytes_written))
    }

    fn finalize<W: Write + Seek>(
        &self,
        w: &mut W,
        superblock: &mut SuperBlock,
    ) -> Result<u64, BackhandError> {
        superblock.bytes_used = w.stream_position()?;

        // pad bytes if required
        let mut pad_len = 0;
        if self.pad_len != 0 {
            // Pad out block_size to 4K
            info!("Writing Padding");
            let blocks_used = superblock.bytes_used as u32 / self.pad_len;
            let total_pad_len = (blocks_used + 1) * self.pad_len;
            pad_len = total_pad_len - superblock.bytes_used as u32;

            // Write 1K at a time
            let mut total_written = 0;
            while w.stream_position()? < (superblock.bytes_used + pad_len as u64) {
                let arr = &[0x00; 1024];

                // check if last block to write
                let len = if (pad_len - total_written) < 1024 {
                    (pad_len - total_written) % 1024
                } else {
                    // else, full 1K
                    1024
                };

                w.write_all(&arr[..len as usize])?;
                total_written += len;
            }
        }

        // Seek back the beginning and write the superblock
        info!("Writing Superblock");
        trace!("{:#02x?}", superblock);
        w.rewind()?;
        let mut bv = BitVec::new();
        superblock.write(&mut bv, self.kind)?;
        w.write_all(bv.as_raw_slice())?;

        info!("Writing Finished");

        Ok(superblock.bytes_used + pad_len as u64)
    }

    fn write_id_table<W: Write + Seek>(
        &self,
        w: &mut W,
        id_table: &Vec<Id>,
        write_superblock: &mut SuperBlock,
    ) -> Result<(), BackhandError> {
        let id_table_dat = w.stream_position()?;
        let mut id_bytes = Vec::with_capacity(id_table.len() * ((u32::BITS / 8) as usize));
        for i in &self.id_table {
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
        write_superblock.id_count = id_table.len() as u16;

        let mut bv = BitVec::new();
        id_table_dat.write(&mut bv, self.kind.type_endian)?;
        w.write_all(bv.as_raw_slice())?;

        Ok(())
    }

    fn write_frag_table<W: Write + Seek>(
        &self,
        w: &mut W,
        frag_table: Vec<Fragment>,
        write_superblock: &mut SuperBlock,
    ) -> Result<(), BackhandError> {
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
        let found = self.id_table.iter().position(|a| a.0 == id);

        match found {
            Some(found) => found as u16,
            None => {
                self.id_table.push(Id(id));
                self.id_table.len() as u16 - 1
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
#[derive(Debug, Copy, Clone, Default)]
pub struct FilesystemCompressor {
    pub(crate) id: Compressor,
    pub(crate) options: Option<CompressionOptions>,
    pub(crate) extra: Option<CompressionExtra>,
}

impl FilesystemCompressor {
    pub fn new(id: Compressor, options: Option<CompressionOptions>) -> Result<Self, BackhandError> {
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
        Err(BackhandError::InvalidCompressionOption)
    }

    /// Set options that are originally derived from the image if from a [`FilesystemReader`].
    /// These options will be written to the image when
    /// <https://github.com/wcampbell0x2a/backhand/issues/53> is fixed.
    pub fn options(&mut self, options: CompressionOptions) -> Result<(), BackhandError> {
        self.options = Some(options);
        Ok(())
    }

    /// Extra options that are *only* using during compression and are *not* stored in the
    /// resulting image
    pub fn extra(&mut self, extra: CompressionExtra) -> Result<(), BackhandError> {
        if matches!(extra, CompressionExtra::Xz(_)) && matches!(self.id, Compressor::Xz) {
            self.extra = Some(extra);
            return Ok(());
        }

        error!("invalid extra compression settings");
        Err(BackhandError::InvalidCompressionOption)
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
    pub fn level(&mut self, level: u32) -> Result<(), BackhandError> {
        if level > 9 {
            return Err(BackhandError::InvalidCompressionOption);
        }
        self.level = Some(level);

        Ok(())
    }
}
