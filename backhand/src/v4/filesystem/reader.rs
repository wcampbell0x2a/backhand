use std::sync::{Mutex, RwLock};

use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::traits::block_reader::{self, BlockReaderVersion, Cache};
use crate::v4::compressor::CompressionOptions;
use crate::v4::data::DataSize;
use crate::v4::filesystem::node::Nodes;
use crate::v4::fragment::Fragment;
use crate::v4::id::Id;
use crate::v4::reader::BufReadSeek;
use crate::{Node, Squashfs, SquashfsFileReader};

/// A file's `frag_index` when it has no tail stored in a fragment block
const NO_FRAGMENT: usize = 0xffffffff;

#[cfg(not(feature = "parallel"))]
use crate::traits::block_reader_no_parallel as block_reader_impl;
#[cfg(feature = "parallel")]
use crate::traits::block_reader_parallel as block_reader_impl;

/// Reads a v4 file's data blocks from the image, without decompressing them
pub type SquashfsRawData<'a, 'b> = block_reader_impl::SquashfsRawData<'a, 'b, V4Blocks>;

/// A [`std::io::Read`] + [`std::io::Seek`] handle over one v4 file's decompressed contents
pub type SquashfsReadFile<'a, 'b> = block_reader_impl::SquashfsReadFile<'a, 'b, V4Blocks>;

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
    ///
    /// `None` when the image records no compressor id. Kept version-neutral so the shared block
    /// reader can pass it straight to [`Kind::decompress`] without converting per block.
    pub compressor: Option<crate::traits::types::Compressor>,
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

/// Maps SquashFS v4 types onto the shared block reader
///
/// See [`BlockReaderVersion`] for what each method means.
pub struct V4Blocks;

impl<'b> BlockReaderVersion<'b> for V4Blocks {
    type DataSize = DataSize;
    type Fragment = Fragment;
    type File = SquashfsFileReader;
    type System = FilesystemReader<'b>;

    fn data_size(data_size: &Self::DataSize) -> u32 {
        data_size.size()
    }

    fn data_uncompressed(data_size: &Self::DataSize) -> bool {
        data_size.uncompressed()
    }

    fn fragment_start(fragment: &Self::Fragment) -> u64 {
        fragment.start
    }

    fn fragment_size(fragment: &Self::Fragment) -> Self::DataSize {
        fragment.size
    }

    fn file_len(file: &Self::File) -> usize {
        file.file_len()
    }

    fn block_sizes(file: &Self::File) -> &[Self::DataSize] {
        file.block_sizes()
    }

    fn blocks_start(file: &Self::File) -> u64 {
        file.blocks_start()
    }

    fn block_offset(file: &Self::File) -> u32 {
        file.block_offset()
    }

    fn kind(system: &Self::System) -> &Kind {
        &system.kind
    }

    fn block_size(system: &Self::System) -> u32 {
        system.block_size
    }

    fn compressor(system: &Self::System) -> Option<crate::traits::types::Compressor> {
        system.compressor
    }

    fn reader(system: &Self::System) -> &Mutex<Box<dyn BufReadSeek + 'b>> {
        &system.reader
    }

    fn cache(system: &Self::System) -> &RwLock<Cache> {
        &system.cache
    }

    fn fragment_of<'a>(
        system: &'a Self::System,
        file: &'a Self::File,
    ) -> Result<Option<&'a Self::Fragment>, BackhandError> {
        if file.frag_index() == NO_FRAGMENT {
            return Ok(None);
        }
        match system.fragments.as_ref() {
            None => Ok(None),
            Some(fragments) => fragments
                .get(file.frag_index())
                .map(Some)
                .ok_or(BackhandError::CorruptedOrInvalidSquashfs),
        }
    }
}

/// Filesystem handle for a v4 file
pub type FilesystemReaderFile<'a, 'b> = block_reader::FilesystemReaderFile<'a, 'b, V4Blocks>;

impl<'a, 'b> FilesystemReaderFile<'a, 'b> {
    /// Create [`SquashfsReadFile`] that impls [`std::io::Read`] from [`FilesystemReaderFile`].
    /// This can be used to then call functions from [`std::io::Read`]
    /// to de-compress and read the data from this file.
    ///
    /// [Read::read]: std::io::Read::read
    /// [Vec::clear]: Vec::clear
    pub fn reader(&self) -> SquashfsReadFile<'a, 'b> {
        self.raw_data_reader().into_reader()
    }

    /// Same as [`Self::reader`], but reporting a file whose fragment index is not in the fragment
    /// table instead of silently treating it as having no fragment.
    pub fn reader_checked(&self) -> Result<SquashfsReadFile<'a, 'b>, BackhandError> {
        Ok(self.raw_data_reader_checked()?.into_reader())
    }

    pub(crate) fn raw_data_reader(&self) -> SquashfsRawData<'a, 'b> {
        // A corrupt fragment index yields a reader over the file's whole blocks only; use
        // [`Self::raw_data_reader_checked`] to see the error instead.
        SquashfsRawData::new(self.system, self.file)
            .unwrap_or_else(|_| SquashfsRawData::new_without_fragment(self.system, self.file))
    }

    pub(crate) fn raw_data_reader_checked(&self) -> Result<SquashfsRawData<'a, 'b>, BackhandError> {
        SquashfsRawData::new(self.system, self.file)
    }
}
