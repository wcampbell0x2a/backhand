use solana_nohash_hasher::IntMap;

use super::filesystem::FilesystemReaderTrait;
use super::types::Compressor;
use crate::kinds::Kind;
use crate::v4::reader::BufReadSeek;

pub trait SquashfsVersion<'b> {
    type SuperBlock: Clone + std::fmt::Debug;
    type CompressionOptions: Clone + std::fmt::Debug;
    type Inode: Clone + std::fmt::Debug;
    type Dir: Clone + std::fmt::Debug;
    type Fragment: Clone + std::fmt::Debug;
    type Export: Clone + std::fmt::Debug;
    type Id: Clone + std::fmt::Debug;
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

pub struct GenericSquashfs<'b, V: SquashfsVersion<'b>> {
    pub kind: Kind,
    pub superblock: V::SuperBlock,
    pub compression_options: Option<V::CompressionOptions>,
    pub inodes: IntMap<u32, V::Inode>,
    pub root_inode: V::Inode,
    pub dir_blocks: (IntMap<u64, u64>, Vec<u8>),
    pub fragments: Option<Vec<V::Fragment>>,
    pub export: Option<Vec<V::Export>>,
    pub id: Vec<V::Id>,
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

pub fn create_squashfs_from_kind<'b>(
    reader: impl BufReadSeek + 'b,
    offset: u64,
    kind: Kind,
) -> Result<Box<dyn FilesystemReaderTrait + 'b>, crate::error::BackhandError> {
    let (major, minor) = (kind.version_major(), kind.version_minor());
    match (major, minor) {
        #[cfg(feature = "v3")]
        (3, 0) => {
            let squashfs = crate::v3::squashfs::Squashfs::from_reader_with_offset_and_kind(
                reader, offset, kind,
            )?;
            let filesystem = squashfs.into_filesystem_reader()?;
            Ok(Box::new(filesystem) as Box<dyn FilesystemReaderTrait + 'b>)
        }
        #[cfg(not(feature = "v3"))]
        (3, 0) => Err(crate::error::BackhandError::UnsupportedSquashfsVersion(3, 0)),
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
