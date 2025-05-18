use std::sync::{Mutex, RwLock};

use super::node::Nodes;
use crate::compressor::{CompressionOptions, Compressor};
use crate::data::DataSize;
use crate::error::BackhandError;
use crate::fragment::Fragment;
use crate::id::Id;
use crate::kinds::Kind;
use crate::reader::BufReadSeek;
use crate::squashfs::Cache;
use crate::{Node, Squashfs, SquashfsFileReader};

#[cfg(not(feature = "parallel"))]
use crate::filesystem::reader_no_parallel::{SquashfsRawData, SquashfsReadFile};
#[cfg(feature = "parallel")]
use crate::filesystem::reader_parallel::{SquashfsRawData, SquashfsReadFile};

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
///         InnerNode::NamedPipe => (),
///         InnerNode::Socket => (),
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
    /// File reader
    pub(crate) reader: Mutex<Box<dyn BufReadSeek + 'b>>,
    /// Cache used in the decompression
    pub(crate) cache: RwLock<Cache>,
    /// Superblock Flag to remove duplicate flags
    pub(crate) no_duplicate_files: bool,
}

impl<'b> FilesystemReader<'b> {
    /// Call [`Squashfs::from_reader`], then [`Squashfs::into_filesystem_reader`]
    ///
    /// With default kind: [`crate::kind::LE_V4_0`] and offset `0`.
    pub fn from_reader<R>(reader: R) -> Result<Self, BackhandError>
    where
        R: BufReadSeek + 'b,
    {
        let squashfs = Squashfs::from_reader_with_offset(reader, 0)?;
        squashfs.into_filesystem_reader()
    }

    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before reading
    pub fn from_reader_with_offset<R>(reader: R, offset: u64) -> Result<Self, BackhandError>
    where
        R: BufReadSeek + 'b,
    {
        let squashfs = Squashfs::from_reader_with_offset(reader, offset)?;
        squashfs.into_filesystem_reader()
    }

    /// Same as [`Self::from_reader_with_offset`], but setting custom `kind`
    pub fn from_reader_with_offset_and_kind<R>(
        reader: R,
        offset: u64,
        kind: Kind,
    ) -> Result<Self, BackhandError>
    where
        R: BufReadSeek + 'b,
    {
        let squashfs = Squashfs::from_reader_with_offset_and_kind(reader, offset, kind)?;
        squashfs.into_filesystem_reader()
    }

    /// Return a file handler for this file
    pub fn file<'a>(&'a self, file: &'a SquashfsFileReader) -> FilesystemReaderFile<'a, 'b> {
        FilesystemReaderFile::new(self, file)
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
    /// for node in filesystem.files() {
    ///     // extract
    ///     match &node.inner {
    ///         InnerNode::File(file) => {
    ///             let mut reader = filesystem
    ///                 .file(&file)
    ///                 .reader();
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
    pub(crate) file: &'a SquashfsFileReader,
}

impl<'a, 'b> FilesystemReaderFile<'a, 'b> {
    pub fn new(system: &'a FilesystemReader<'b>, file: &'a SquashfsFileReader) -> Self {
        Self { system, file }
    }

    /// Create [`SquashfsReadFile`] that impls [`std::io::Read`] from [`FilesystemReaderFile`].
    /// This can be used to then call functions from [`std::io::Read`]
    /// to de-compress and read the data from this file.
    ///
    /// [Read::read]: std::io::Read::read
    /// [Vec::clear]: Vec::clear
    pub fn reader(&self) -> SquashfsReadFile<'a, 'b> {
        self.raw_data_reader().into_reader()
    }

    pub fn fragment(&self) -> Option<&'a Fragment> {
        if self.file.frag_index() == 0xffffffff {
            None
        } else {
            self.system.fragments.as_ref().map(|fragments| &fragments[self.file.frag_index()])
        }
    }

    pub(crate) fn raw_data_reader(&self) -> SquashfsRawData<'a, 'b> {
        SquashfsRawData::new(Self { system: self.system, file: self.file })
    }
}

impl<'a> IntoIterator for FilesystemReaderFile<'a, '_> {
    type IntoIter = BlockIterator<'a>;
    type Item = <BlockIterator<'a> as Iterator>::Item;

    fn into_iter(self) -> Self::IntoIter {
        BlockIterator { blocks: self.file.block_sizes(), fragment: self.fragment() }
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
