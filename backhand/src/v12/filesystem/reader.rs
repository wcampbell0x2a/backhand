//! Reading file data out of a SquashFS v1 or v2 image

use std::sync::{Mutex, RwLock};

use super::node::{Nodes, SquashfsFileReader};
use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::traits::block_reader::{self, BlockReaderVersion, Cache};
#[cfg(not(feature = "parallel"))]
use crate::traits::block_reader_no_parallel as block_reader_impl;
#[cfg(feature = "parallel")]
use crate::traits::block_reader_parallel as block_reader_impl;
use crate::traits::types::Compressor;
use crate::v4::id::Id;
use crate::v4::reader::{BufReadSeek, SquashfsReaderWithOffset};
use crate::v12::data::DataSize;
use crate::v12::fragment::Fragment;
use crate::v12::inode::NO_FRAGMENT;
use crate::v12::squashfs::Squashfs;

/// An opened v1 or v2 image, ready to read file data from
pub struct FilesystemReader<'b> {
    pub kind: Kind,
    pub block_size: u32,
    pub block_log: u16,
    /// Always `None`: neither version records a compressor id. The kind holds
    /// the decompressor instead.
    pub compressor: Option<Compressor>,
    pub mod_time: u32,
    pub uid_table: Vec<Id>,
    pub guid_table: Vec<Id>,
    pub fragments: Option<Vec<Fragment>>,
    pub root: Nodes<SquashfsFileReader>,
    pub(crate) reader: Mutex<Box<dyn BufReadSeek + 'b>>,
    pub(crate) cache: RwLock<Cache>,
}

impl<'b> FilesystemReader<'b> {
    /// Open an image with a kind, seeking to `offset` first
    pub fn from_reader_with_offset_and_kind<R>(
        reader: R,
        offset: u64,
        kind: Kind,
    ) -> Result<Self, BackhandError>
    where
        R: BufReadSeek + 'b,
    {
        let mut reader: Box<dyn BufReadSeek + 'b> = if offset == 0 {
            Box::new(reader)
        } else {
            Box::new(SquashfsReaderWithOffset::new(reader, offset)?)
        };

        let squashfs = Squashfs::from_reader(&mut *reader, kind)?;
        let root = squashfs.nodes()?;

        Ok(Self {
            block_size: squashfs.superblock.block_size(squashfs.layout),
            block_log: squashfs.superblock.block_log,
            compressor: None,
            mod_time: squashfs.superblock.mkfs_time,
            uid_table: squashfs.uid,
            guid_table: squashfs.guid,
            fragments: squashfs.fragments,
            kind: squashfs.kind,
            root,
            reader: Mutex::new(reader),
            cache: RwLock::new(Cache::default()),
        })
    }

    /// Iterate every node of the image
    pub fn files(
        &self,
    ) -> impl Iterator<Item = &crate::v12::filesystem::node::Node<SquashfsFileReader>> {
        self.root.nodes.iter()
    }

    /// A handle that reads one file's data
    pub fn file<'a>(&'a self, file: &'a SquashfsFileReader) -> FilesystemReaderFile<'a, 'b> {
        FilesystemReaderFile::new(self, file)
    }
}

/// Binds the shared block reader to the v1/v2 types
pub struct V12Blocks;

impl<'b> BlockReaderVersion<'b> for V12Blocks {
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
        u64::from(fragment.start)
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

    fn compressor(system: &Self::System) -> Option<Compressor> {
        system.compressor
    }

    fn reader(system: &Self::System) -> &Mutex<Box<dyn BufReadSeek + 'b>> {
        &system.reader
    }

    fn cache(system: &Self::System) -> &RwLock<Cache> {
        &system.cache
    }

    /// v1 has no fragment table at all, so `system.fragments` is `None` and
    /// every file falls through to "no fragment".
    fn fragment_of<'a>(
        system: &'a Self::System,
        file: &'a Self::File,
    ) -> Result<Option<&'a Self::Fragment>, BackhandError> {
        if file.frag_index() == NO_FRAGMENT as usize {
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

/// Handle to one file's data
pub type FilesystemReaderFile<'a, 'b> = block_reader::FilesystemReaderFile<'a, 'b, V12Blocks>;

/// The raw block stream of one file
pub type SquashfsRawData<'a, 'b> = block_reader_impl::SquashfsRawData<'a, 'b, V12Blocks>;
/// A [`std::io::Read`] over one file's decompressed data
pub type SquashfsReadFile<'a, 'b> = block_reader_impl::SquashfsReadFile<'a, 'b, V12Blocks>;

impl<'a, 'b> FilesystemReaderFile<'a, 'b> {
    /// A reader over this file's decompressed data
    pub fn reader(&self) -> SquashfsReadFile<'a, 'b> {
        SquashfsRawData::new(self.system, self.file)
            .unwrap_or_else(|_| SquashfsRawData::new_without_fragment(self.system, self.file))
            .into_reader()
    }

    /// Same as [`Self::reader`], but reporting a fragment index that is not in
    /// the table instead of treating the file as having no fragment.
    pub fn reader_checked(&self) -> Result<SquashfsReadFile<'a, 'b>, BackhandError> {
        Ok(SquashfsRawData::new(self.system, self.file)?.into_reader())
    }
}
