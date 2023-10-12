use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use deku::bitvec::BitVec;
use deku::DekuWrite;
use tracing::{error, info, instrument, trace};

use super::node::{InnerNode, Nodes};
use super::normalize_squashfs_path;
use crate::bufread::WriteSeek;
use crate::compressor::{CompressionOptions, Compressor, FilesystemCompressor};
use crate::error::BackhandError;
use crate::flags::Flags;
use crate::kind::Kind;
use crate::kinds::LE_V4_0;
use crate::v4::data::DataWriter;
use crate::v4::entry::Entry;
use crate::v4::filesystem::node::SquashfsSymlink;
use crate::v4::id::Id;
use crate::v4::metadata::{self, MetadataWriter, METADATA_MAXSIZE};
use crate::v4::squashfs::SuperBlock;
use crate::v4::{
    fragment, FilesystemReader, Node, NodeHeader, SquashfsBlockDevice, SquashfsCharacterDevice,
    SquashfsDir, SquashfsFileWriter, DEFAULT_BLOCK_SIZE, DEFAULT_PAD_LEN, MAX_BLOCK_SIZE,
    MIN_BLOCK_SIZE,
};

/// Representation of SquashFS filesystem to be written back to an image
/// - Use [`Self::from_fs_reader`] to write with the data from a previous SquashFS image
/// - Use [`Self::default`] to create an empty SquashFS image without an original image. For example:
/// ```rust
/// # use std::time::SystemTime;
/// # use backhand::{NodeHeader, Id, FilesystemCompressor, FilesystemWriter, SquashfsDir, compression::Compressor, kind, DEFAULT_BLOCK_SIZE, ExtraXz, CompressionExtra, kind::Kind};
/// // Add empty default FilesytemWriter
/// let mut fs = FilesystemWriter::default();
/// fs.set_current_time();
/// fs.set_block_size(DEFAULT_BLOCK_SIZE);
/// fs.set_only_root_id();
/// fs.set_kind(Kind::from_const(kind::LE_V4_0).unwrap());
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
pub struct FilesystemWriter<'a, 'b> {
    pub(crate) kind: Kind,
    /// The size of a data block in bytes. Must be a power of two between 4096 (4k) and 1048576 (1 MiB).
    pub(crate) block_size: u32,
    /// Last modification time of the archive. Count seconds since 00:00, Jan 1st 1970 UTC (not counting leap seconds).
    /// This is unsigned, so it expires in the year 2106 (as opposed to 2038).
    pub(crate) mod_time: u32,
    /// 32 bit user and group IDs
    pub(crate) id_table: Vec<Id>,
    /// Compressor used when writing
    pub(crate) fs_compressor: FilesystemCompressor,
    /// All files and directories in filesystem, including root
    pub(crate) root: Nodes<SquashfsFileWriter<'a, 'b>>,
    /// The log2 of the block size. If the two fields do not agree, the archive is considered corrupted.
    pub(crate) block_log: u16,
    pub(crate) pad_len: u32,
}

impl<'a, 'b> Default for FilesystemWriter<'a, 'b> {
    /// Create default FilesystemWriter
    ///
    /// block_size: [`DEFAULT_BLOCK_SIZE`], compressor: default XZ compression, no nodes,
    /// kind: [`LE_V4_0`], and mod_time: `0`.
    fn default() -> Self {
        let block_size = DEFAULT_BLOCK_SIZE;
        Self {
            block_size,
            mod_time: 0,
            id_table: Id::root(),
            fs_compressor: FilesystemCompressor::default(),
            kind: Kind {
                inner: Arc::new(LE_V4_0),
            },
            root: Nodes::new_root(NodeHeader::default()),
            block_log: (block_size as f32).log2() as u16,
            pad_len: DEFAULT_PAD_LEN,
        }
    }
}

impl<'a, 'b> FilesystemWriter<'a, 'b> {
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
    /// # use backhand::{FilesystemWriter, kind::Kind, kind};
    /// let mut fs = FilesystemWriter::default();
    /// fs.set_kind(Kind::from_const(kind::LE_V4_0).unwrap());
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
    pub fn set_root_uid(&mut self, uid: u32) {
        self.root.root_mut().header.uid = uid;
    }

    /// Set root gid as `gid`
    pub fn set_root_gid(&mut self, gid: u32) {
        self.root.root_mut().header.gid = gid;
    }

    /// Set compressor as `compressor`
    ///
    ///```rust
    /// # use backhand::{FilesystemWriter, FilesystemCompressor, compression::Compressor};
    /// let mut compressor = FilesystemCompressor::new(Compressor::Xz, None).unwrap();
    /// ```
    pub fn set_compressor(&mut self, compressor: FilesystemCompressor) {
        self.fs_compressor = compressor;
    }

    /// Set id_table to [`Id::root`], removing old entries
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
    pub fn from_fs_reader(reader: &'a FilesystemReader<'b>) -> Result<Self, BackhandError> {
        let mut root: Vec<Node<_>> = reader
            .root
            .nodes
            .iter()
            .map(|node| {
                let inner = match &node.inner {
                    InnerNode::File(file) => {
                        let reader = reader.file(&file.basic);
                        InnerNode::File(SquashfsFileWriter::SquashfsFile(reader))
                    }
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
            kind: Kind {
                inner: reader.kind.inner.clone(),
            },
            block_size: reader.block_size,
            block_log: reader.block_log,
            fs_compressor: FilesystemCompressor::new(
                reader.compressor,
                reader.compression_options,
            )?,
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
    ) -> Option<&mut Node<SquashfsFileWriter<'a, 'b>>> {
        //the search path root prefix is optional, so remove it if present to
        //not affect the search
        let find_path = normalize_squashfs_path(find_path.as_ref()).ok()?;
        self.root.node_mut(find_path)
    }

    fn insert_node<P: AsRef<Path>>(
        &mut self,
        path: P,
        header: NodeHeader,
        node: InnerNode<SquashfsFileWriter<'a, 'b>>,
    ) -> Result<(), BackhandError> {
        // create gid id
        self.lookup_add_id(header.gid);
        // create uid id
        self.lookup_add_id(header.uid);

        let path = normalize_squashfs_path(path.as_ref())?;
        let node = Node::new(path, header, node);
        self.root.insert(node)
    }

    /// Insert `reader` into filesystem with `path` and metadata `header`.
    ///
    /// The `uid` and `gid` in `header` are added to FilesystemWriters id's
    pub fn push_file<P: AsRef<Path>>(
        &mut self,
        reader: impl Read + 'b,
        path: P,
        header: NodeHeader,
    ) -> Result<(), BackhandError> {
        let reader = Arc::new(Mutex::new(reader));
        let new_file = InnerNode::File(SquashfsFileWriter::UserDefined(reader));
        self.insert_node(path, header, new_file)?;
        Ok(())
    }

    /// Take a mutable reference to existing file at `find_path`
    pub fn mut_file<S: AsRef<Path>>(
        &mut self,
        find_path: S,
    ) -> Option<&mut SquashfsFileWriter<'a, 'b>> {
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
        reader: impl Read + 'b,
    ) -> Result<(), BackhandError> {
        let file = self
            .mut_file(find_path)
            .ok_or(BackhandError::FileNotFound)?;
        let reader = Arc::new(Mutex::new(reader));
        *file = SquashfsFileWriter::UserDefined(reader);
        Ok(())
    }

    /// Insert symlink `path` -> `link`
    ///
    /// The `uid` and `gid` in `header` are added to FilesystemWriters id's
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
    /// The `uid` and `gid` in `header` are added to FilesystemWriters id's
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
    /// The `uid` and `gid` in `header` are added to FilesystemWriters id's
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
                }
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
                }
            }
        }
        Ok(())
    }

    /// Insert character device with `device_number` at `path`
    ///
    /// The `uid` and `gid` in `header` are added to FilesystemWriters id's
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
    /// The `uid` and `gid` in `header` are added to FilesystemWriters id's
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
        data_writer: &mut DataWriter<'b>,
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
            let (filesize, added) = match file {
                SquashfsFileWriter::UserDefined(file) => {
                    let file_ptr = Arc::clone(file);
                    let mut file_lock = file_ptr.lock().unwrap();
                    data_writer.add_bytes(&mut *file_lock, writer)?
                }
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
                        let mut buf_decompress = vec![];
                        data_writer
                            .add_bytes(file.reader(&mut buf_read, &mut buf_decompress), writer)?
                    }
                }
                SquashfsFileWriter::Consumed(_, _) => unreachable!(),
            };
            *file = SquashfsFileWriter::Consumed(filesize, added);
        }
        Ok(())
    }

    /// Create SquashFS file system from each node of Tree
    ///
    /// This works by recursively creating Inodes and Dirs for each node in the tree. This also
    /// keeps track of parent directories by calling this function on all nodes of a dir to get only
    /// the nodes, but going into the child dirs in the case that it contains a child dir.
    #[allow(clippy::too_many_arguments)]
    fn write_inode_dir<'c>(
        &'c self,
        inode_writer: &'_ mut MetadataWriter,
        dir_writer: &'_ mut MetadataWriter,
        parent_node_id: u32,
        node_id: NonZeroUsize,
        superblock: &SuperBlock,
        kind: &Kind,
        id_table: &Vec<Id>,
    ) -> Result<Entry<'c>, BackhandError> {
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
                    id_table,
                ))
            }
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
                    id_table,
                ))
            }
            InnerNode::CharacterDevice(char) => {
                return Ok(Entry::char(
                    filename,
                    node.header,
                    char,
                    node_id.get().try_into().unwrap(),
                    inode_writer,
                    superblock,
                    kind,
                    id_table,
                ))
            }
            InnerNode::BlockDevice(block) => {
                return Ok(Entry::block_device(
                    filename,
                    node.header,
                    block,
                    node_id.get().try_into().unwrap(),
                    inode_writer,
                    superblock,
                    kind,
                    id_table,
                ))
            }
            // if dir, fall through
            InnerNode::Dir(_) => (),
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
                    id_table,
                )
            })
            .collect::<Result<_, _>>()?;
        let children_num = entries.len();

        // write dir
        let block_index = dir_writer.metadata_start;
        let block_offset = dir_writer.uncompressed_bytes.len() as u16;
        trace!("WRITING DIR: {block_offset:#02x?}");
        let mut total_size: usize = 3;
        for dir in Entry::into_dir(entries) {
            let mut bv = BitVec::new();
            dir.write(&mut bv, kind.inner.type_endian)?;
            let bytes = bv.as_raw_slice();
            dir_writer.write_all(bv.as_raw_slice())?;

            total_size += bytes.len();
        }
        let entry = Entry::path(
            filename,
            node.header,
            node_id.get().try_into().unwrap(),
            children_num,
            parent_node_id,
            inode_writer,
            total_size,
            block_offset,
            block_index,
            superblock,
            kind,
            id_table,
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
        let mut superblock = SuperBlock::new(
            self.fs_compressor.id,
            Kind {
                inner: self.kind.inner.clone(),
            },
        );

        trace!("{:#02x?}", self.root);

        // Empty Squashfs Superblock
        w.write_all(&[0x00; 96])?;

        // Write compression options, if any
        if let Some(options) = &self.fs_compressor.options {
            superblock.flags |= Flags::CompressorOptionsArePresent as u16;
            let mut buf = BitVec::new();
            match options {
                CompressionOptions::Gzip(gzip) => {
                    gzip.write(&mut buf, self.kind.inner.type_endian)?
                }
                CompressionOptions::Lz4(lz4) => lz4.write(&mut buf, self.kind.inner.type_endian)?,
                CompressionOptions::Zstd(zstd) => {
                    zstd.write(&mut buf, self.kind.inner.type_endian)?
                }
                CompressionOptions::Xz(xz) => xz.write(&mut buf, self.kind.inner.type_endian)?,
                CompressionOptions::Lzo(lzo) => lzo.write(&mut buf, self.kind.inner.type_endian)?,
                CompressionOptions::Lzma => {}
            }
            let mut metadata = MetadataWriter::new(
                self.fs_compressor,
                self.block_size,
                Kind {
                    inner: self.kind.inner.clone(),
                },
            );
            metadata.write_all(buf.as_raw_slice())?;
            metadata.finalize(w)?;
        }

        let mut data_writer = DataWriter::new(
            self.kind.inner.compressor,
            self.fs_compressor,
            self.block_size,
        );
        let mut inode_writer = MetadataWriter::new(
            self.fs_compressor,
            self.block_size,
            Kind {
                inner: self.kind.inner.clone(),
            },
        );
        let mut dir_writer = MetadataWriter::new(
            self.fs_compressor,
            self.block_size,
            Kind {
                inner: self.kind.inner.clone(),
            },
        );

        info!("Creating Inodes and Dirs");
        //trace!("TREE: {:#02x?}", &self.root);
        info!("Writing Data");
        self.write_data(self.fs_compressor, self.block_size, w, &mut data_writer)?;
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
            &self.kind,
            &self.id_table,
        )?;

        superblock.root_inode = ((root.start as u64) << 16) | ((root.offset as u64) & 0xffff);
        superblock.inode_count = self.root.nodes.len().try_into().unwrap();
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
        let (table_position, count) =
            self.write_lookup_table(w, &data_writer.fragment_table, fragment::SIZE)?;
        superblock.frag_table = table_position;
        superblock.frag_count = count;

        info!("Writing Id Lookup Table");
        let (table_position, count) = self.write_lookup_table(w, &self.id_table, Id::SIZE)?;
        superblock.id_table = table_position;
        superblock.id_count = count.try_into().unwrap();

        info!("Finalize Superblock and End Bytes");
        let bytes_written = self.finalize(w, &mut superblock)?;

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
            let blocks_used: u32 = u32::try_from(superblock.bytes_used).unwrap() / self.pad_len;
            let total_pad_len = (blocks_used + 1) * self.pad_len;
            pad_len = total_pad_len - u32::try_from(superblock.bytes_used).unwrap();

            // Write 1K at a time
            let mut total_written = 0;
            while w.stream_position()? < (superblock.bytes_used + u64::try_from(pad_len).unwrap()) {
                let arr = &[0x00; 1024];

                // check if last block to write
                let len = if (pad_len - total_written) < 1024 {
                    (pad_len - total_written) % 1024
                } else {
                    // else, full 1K
                    1024
                };

                w.write_all(&arr[..len.try_into().unwrap()])?;
                total_written += len;
            }
        }

        // Seek back the beginning and write the superblock
        info!("Writing Superblock");
        w.rewind()?;
        let mut bv = BitVec::new();
        superblock.write(
            &mut bv,
            (
                self.kind.inner.magic,
                self.kind.inner.version_major,
                self.kind.inner.version_minor,
                self.kind.inner.type_endian,
            ),
        )?;
        w.write_all(bv.as_raw_slice())?;

        info!("Writing Finished");

        //clean any cache, make sure the output is on disk
        w.flush()?;
        Ok(superblock.bytes_used + u64::try_from(pad_len).unwrap())
    }

    /// For example, writing a fragment table:
    /// ```text
    ///  ┌──────────────────────────────┐
    ///  │Metadata                      │◄───┐
    ///  │┌────────────────────────────┐│    │
    ///  ││pointer to fragment block   ││    │
    ///  │├────────────────────────────┤│    │
    ///  ││pointer to fragment block   ││    │
    ///  │└────────────────────────────┘│    │
    ///  └──────────────────────────────┘    │
    ///  ┌──────────────────────────────┐    │
    ///  │Metadata                      │◄─┐ │
    ///  │┌────────────────────────────┐│  │ │
    ///  ││pointer to fragment block   ││  │ │
    ///  │├────────────────────────────┤│  │ │
    ///  ││pointer to fragment block   ││  │ │
    ///  │└────────────────────────────┘│  │ │
    ///  └──────────────────────────────┘  │ │
    ///  ┌──────────────────────────────┐──│─│───►superblock.frag_table
    ///  │Frag Table                    │  │ │
    ///  │┌────────────────────────────┐│  │ │
    ///  ││fragment0(u64)         ─────────│─┘
    ///  │├────────────────────────────┤│  │
    ///  ││fragment1(u64)         ─────────┘
    ///  │└────────────────────────────┘│
    ///  └──────────────────────────────┘
    ///  ```
    fn write_lookup_table<D: DekuWrite<deku::ctx::Endian>, W: Write + Seek>(
        &self,
        w: &mut W,
        table: &Vec<D>,
        element_size: usize,
    ) -> Result<(u64, u32), BackhandError> {
        let mut ptrs: Vec<u64> = vec![];
        let mut table_bytes = Vec::with_capacity(table.len() * element_size);
        let mut iter = table.iter().peekable();
        while let Some(t) = iter.next() {
            // convert fragment ptr to bytes
            let mut bv = BitVec::new();
            t.write(&mut bv, self.kind.inner.type_endian)?;
            table_bytes.write_all(bv.as_raw_slice())?;

            // once table_bytes + next is over the maximum size of a metadata block, write
            if ((table_bytes.len() + element_size) > METADATA_MAXSIZE) || iter.peek().is_none() {
                ptrs.push(w.stream_position()?);

                let mut bv = BitVec::new();
                // write metadata len
                let len = metadata::set_if_uncompressed(table_bytes.len() as u16);
                len.write(&mut bv, self.kind.inner.data_endian)?;
                w.write_all(bv.as_raw_slice())?;
                // write metadata bytes
                w.write_all(&table_bytes)?;

                table_bytes.clear();
            }
        }

        let table_position = w.stream_position()?;
        let count = table.len() as u32;

        // write ptr
        for ptr in ptrs {
            let mut bv = BitVec::new();
            ptr.write(&mut bv, self.kind.inner.type_endian)?;
            w.write_all(bv.as_raw_slice())?;
        }

        Ok((table_position, count))
    }

    /// Return index of id, adding if required
    fn lookup_add_id(&mut self, id: u32) -> u32 {
        let found = self.id_table.iter().position(|a| a.num == id);

        match found {
            Some(found) => found as u32,
            None => {
                self.id_table.push(Id::new(id));
                self.id_table.len() as u32 - 1
            }
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
