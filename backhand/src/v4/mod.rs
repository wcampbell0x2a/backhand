//! SquashFS v4 implementation

use crate::BackhandError;
use crate::kinds::Kind;
use crate::traits::{GenericSquashfs, SquashfsVersion};
use crate::v4::reader::BufReadSeek;

/// Compression algorithm types and implementation
pub mod compressor;
/// Data block reading and writing
pub mod data;
/// Directory table types
pub mod dir;
/// Directory entry types (internal)
pub mod entry;
/// NFS export table
pub mod export;
/// In-memory filesystem representation
pub mod filesystem;
/// Fragment table types
pub mod fragment;
/// UID/GID table
pub mod id;
/// Inode types
pub mod inode;
/// Metadata block reading and writing
pub mod metadata;
/// Reader traits and utilities
pub mod reader;
/// Superblock and on-disk image types
pub mod squashfs;
/// Platform-independent OsStr/OsString extensions
pub mod unix_string;

/// V4 implementation of SquashfsVersion trait
pub struct V4;

impl<'b> SquashfsVersion<'b> for V4 {
    type SuperBlock = squashfs::SuperBlock;
    type CompressionOptions = compressor::CompressionOptions;
    type Inode = inode::Inode;
    type Dir = dir::Dir;
    type Fragment = fragment::Fragment;
    type Export = export::Export;
    type Id = id::Id;
    type FilesystemReader = filesystem::reader::FilesystemReader<'b>;

    fn superblock_and_compression_options(
        reader: &mut Box<dyn BufReadSeek + 'b>,
        kind: &Kind,
    ) -> Result<(Self::SuperBlock, Option<Self::CompressionOptions>), BackhandError> {
        squashfs::Squashfs::superblock_and_compression_options(reader, kind)
    }

    fn from_reader_with_offset_and_kind(
        reader: impl BufReadSeek + 'b,
        offset: u64,
        kind: Kind,
    ) -> Result<GenericSquashfs<'b, Self>, BackhandError> {
        let v4_squashfs =
            squashfs::Squashfs::from_reader_with_offset_and_kind(reader, offset, kind)?;

        Ok(GenericSquashfs {
            kind: v4_squashfs.kind,
            superblock: v4_squashfs.superblock,
            compression_options: v4_squashfs.compression_options,
            inodes: v4_squashfs.inodes,
            root_inode: v4_squashfs.root_inode,
            dir_blocks: v4_squashfs.dir_blocks,
            fragments: v4_squashfs.fragments,
            export: v4_squashfs.export,
            id: v4_squashfs.id,
            file: v4_squashfs.file,
        })
    }

    fn into_filesystem_reader(
        squashfs: GenericSquashfs<'b, Self>,
    ) -> Result<Self::FilesystemReader, BackhandError> {
        let v4_squashfs = squashfs::Squashfs {
            kind: squashfs.kind,
            superblock: squashfs.superblock,
            compression_options: squashfs.compression_options,
            inodes: squashfs.inodes,
            root_inode: squashfs.root_inode,
            dir_blocks: squashfs.dir_blocks,
            fragments: squashfs.fragments,
            export: squashfs.export,
            id: squashfs.id,
            file: squashfs.file,
        };

        v4_squashfs.into_filesystem_reader()
    }

    fn get_compressor(superblock: &Self::SuperBlock) -> crate::traits::types::Compressor {
        superblock.compressor.into()
    }

    fn get_block_size(superblock: &Self::SuperBlock) -> u32 {
        superblock.block_size
    }
}
