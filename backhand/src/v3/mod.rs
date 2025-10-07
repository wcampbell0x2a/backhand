//! SquashFS v3 implementation

use crate::kinds::Kind;
use crate::traits::{GenericSquashfs, SquashfsVersion};
use crate::v4::reader::BufReadSeek;

pub mod compressor;
pub mod data;
pub mod dir;
pub mod export;
pub mod filesystem;
pub mod fragment;
pub mod id;
pub mod inode;
pub mod metadata;
pub mod reader;
pub mod squashfs;
pub mod unix_string;

/// V3 implementation of SquashfsVersion trait
pub struct V3;

impl<'b> SquashfsVersion<'b> for V3 {
    type SuperBlock = squashfs::SuperBlock;
    // v3 doesn't have compression options
    type CompressionOptions = ();
    type Inode = inode::Inode;
    type Dir = dir::Dir;
    type Fragment = fragment::Fragment;
    type Export = export::Export;
    type Id = id::Id;
    type FilesystemReader = filesystem::reader::FilesystemReader<'b>;

    fn superblock_and_compression_options(
        reader: &mut Box<dyn BufReadSeek + 'b>,
        kind: &Kind,
    ) -> Result<(Self::SuperBlock, Option<Self::CompressionOptions>), crate::BackhandError> {
        squashfs::Squashfs::superblock_and_compression_options(reader, kind)
    }

    fn from_reader_with_offset_and_kind(
        reader: impl BufReadSeek + 'b,
        offset: u64,
        kind: Kind,
    ) -> Result<GenericSquashfs<'b, Self>, crate::BackhandError> {
        let v3_squashfs =
            squashfs::Squashfs::from_reader_with_offset_and_kind(reader, offset, kind)?;

        Ok(GenericSquashfs {
            kind: v3_squashfs.kind,
            superblock: v3_squashfs.superblock,
            compression_options: None, // v3 doesn't have compression options
            inodes: v3_squashfs.inodes,
            root_inode: v3_squashfs.root_inode,
            dir_blocks: v3_squashfs.dir_blocks,
            fragments: v3_squashfs.fragments,
            export: v3_squashfs.export,
            id: v3_squashfs.id.unwrap_or_default(),
            file: v3_squashfs.file,
        })
    }

    fn into_filesystem_reader(
        squashfs: GenericSquashfs<'b, Self>,
    ) -> Result<Self::FilesystemReader, crate::BackhandError> {
        let v3_squashfs = squashfs::Squashfs {
            kind: squashfs.kind,
            superblock: squashfs.superblock,
            inodes: squashfs.inodes,
            root_inode: squashfs.root_inode,
            dir_blocks: squashfs.dir_blocks,
            fragments: squashfs.fragments,
            export: squashfs.export,
            id: Some(squashfs.id),
            uid: None,  // v3 compatibility: not used
            guid: None, // v3 compatibility: not used
            file: squashfs.file,
        };

        v3_squashfs.into_filesystem_reader()
    }

    fn get_compressor(_superblock: &Self::SuperBlock) -> crate::traits::types::Compressor {
        // v3 only supports gzip compression
        crate::traits::types::Compressor::Gzip
    }

    fn get_block_size(superblock: &Self::SuperBlock) -> u32 {
        superblock.block_size
    }
}
