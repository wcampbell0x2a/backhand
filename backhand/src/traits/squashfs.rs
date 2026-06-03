use solana_nohash_hasher::IntMap;

use super::filesystem::FilesystemReaderTrait;
use super::types::Compressor;
use crate::kinds::Kind;
use crate::v4::reader::BufReadSeek;

/// Version-specific SquashFS implementation trait
pub trait SquashfsVersion<'b> {
    /// Superblock type for this version
    type SuperBlock: Clone + core::fmt::Debug;
    /// Compression options type
    type CompressionOptions: Clone + core::fmt::Debug;
    /// Inode type
    type Inode: Clone + core::fmt::Debug;
    /// Directory entry type
    type Dir: Clone + core::fmt::Debug;
    /// Fragment table entry type
    type Fragment: Clone + core::fmt::Debug;
    /// Export table entry type
    type Export: Clone + core::fmt::Debug;
    /// ID table entry type
    type Id: Clone + core::fmt::Debug;
    /// Filesystem reader type
    type FilesystemReader;

    /// Read superblock and compression options
    fn superblock_and_compression_options(
        reader: &mut Box<dyn BufReadSeek + 'b>,
        kind: &Kind,
    ) -> Result<(Self::SuperBlock, Option<Self::CompressionOptions>), crate::error::BackhandError>;

    /// Create from reader with offset and kind
    fn from_reader_with_offset_and_kind(
        reader: impl BufReadSeek + 'b,
        offset: u64,
        kind: Kind,
    ) -> Result<GenericSquashfs<'b, Self>, crate::error::BackhandError>
    where
        Self: Sized;

    /// Convert to filesystem reader
    fn into_filesystem_reader(
        squashfs: GenericSquashfs<'b, Self>,
    ) -> Result<Self::FilesystemReader, crate::error::BackhandError>
    where
        Self: Sized;

    /// Get compressor from superblock
    fn get_compressor(superblock: &Self::SuperBlock) -> Compressor;

    /// Get block size from superblock
    fn get_block_size(superblock: &Self::SuperBlock) -> u32;
}

/// Version-generic SquashFS image data
pub struct GenericSquashfs<'b, V: SquashfsVersion<'b>> {
    /// Image format kind
    pub kind: Kind,
    /// Parsed superblock
    pub superblock: V::SuperBlock,
    /// Compression options from the image
    pub compression_options: Option<V::CompressionOptions>,
    /// Inode cache keyed by inode number
    pub inodes: IntMap<u32, V::Inode>,
    /// Root directory inode
    pub root_inode: V::Inode,
    /// Directory table data
    pub dir_blocks: (IntMap<u64, u64>, Vec<u8>),
    /// Fragment lookup table
    pub fragments: Option<Vec<V::Fragment>>,
    /// NFS export lookup table
    pub export: Option<Vec<V::Export>>,
    /// ID lookup table
    pub id: Vec<V::Id>,
    /// Source reader
    pub file: Box<dyn BufReadSeek + 'b>,
}

impl<'b, V: SquashfsVersion<'b>> GenericSquashfs<'b, V> {
    /// Read Superblock and Compression Options at current `reader` offset without parsing inodes
    /// and dirs
    ///
    /// Used for unsquashfs (extraction and --stat)
    pub fn superblock_and_compression_options(
        reader: &mut Box<dyn BufReadSeek + 'b>,
        kind: &Kind,
    ) -> Result<(V::SuperBlock, Option<V::CompressionOptions>), crate::error::BackhandError> {
        V::superblock_and_compression_options(reader, kind)
    }

    /// Create `GenericSquashfs` from `Read`er
    pub fn from_reader(reader: impl BufReadSeek + 'b) -> Result<Self, crate::error::BackhandError> {
        Self::from_reader_with_offset(reader, 0)
    }

    /// Same as [`Self::from_reader`], but seek'ing to `offset` in `reader` before Reading
    pub fn from_reader_with_offset(
        reader: impl BufReadSeek + 'b,
        offset: u64,
    ) -> Result<Self, crate::error::BackhandError> {
        // Default to v4 kind for now
        use crate::kinds::LE_V4_0;
        use std::sync::Arc;
        let default_kind = Kind { inner: Arc::new(LE_V4_0) };
        V::from_reader_with_offset_and_kind(reader, offset, default_kind)
    }

    /// Same as [`Self::from_reader_with_offset`], but including custom `kind`
    pub fn from_reader_with_offset_and_kind(
        reader: impl BufReadSeek + 'b,
        offset: u64,
        kind: Kind,
    ) -> Result<Self, crate::error::BackhandError> {
        V::from_reader_with_offset_and_kind(reader, offset, kind)
    }

    /// Convert into [`FilesystemReader`] by extracting all file bytes and converting into a filesystem
    /// like structure in-memory
    pub fn into_filesystem_reader(
        self,
    ) -> Result<V::FilesystemReader, crate::error::BackhandError> {
        V::into_filesystem_reader(self)
    }

    /// Get the compressor for this squashfs
    pub fn compressor(&self) -> Compressor {
        V::get_compressor(&self.superblock)
    }

    /// Get the block size for this squashfs
    pub fn block_size(&self) -> u32 {
        V::get_block_size(&self.superblock)
    }
}

/// Create a version-appropriate SquashFS reader based on the kind's version
pub fn create_squashfs_from_kind<'b>(
    reader: impl BufReadSeek + 'b,
    offset: u64,
    kind: Kind,
) -> Result<Box<dyn FilesystemReaderTrait + 'b>, crate::error::BackhandError> {
    let (major, minor) = (kind.version_major(), kind.version_minor());
    match (major, minor) {
        #[cfg(feature = "v3")]
        (3, 0) | (3, 1) => {
            let squashfs = crate::v3::squashfs::Squashfs::from_reader_with_offset_and_kind(
                reader, offset, kind,
            )?;
            let filesystem = squashfs.into_filesystem_reader()?;
            Ok(Box::new(filesystem) as Box<dyn FilesystemReaderTrait + 'b>)
        }
        #[cfg(not(feature = "v3"))]
        (3, 0) | (3, 1) => Err(crate::error::BackhandError::UnsupportedSquashfsVersion(3, 0)),
        (4, 0) => {
            let squashfs = crate::v4::squashfs::Squashfs::from_reader_with_offset_and_kind(
                reader, offset, kind,
            )?;
            let filesystem = squashfs.into_filesystem_reader()?;
            Ok(Box::new(filesystem) as Box<dyn FilesystemReaderTrait + 'b>)
        }
        _ => Err(crate::error::BackhandError::UnsupportedSquashfsVersion(major, minor)),
    }
}
