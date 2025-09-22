//! SquashFS v3 implementation

use solana_nohash_hasher::IntMap;

use crate::kinds::Kind;
use crate::traits::{GenericSquashfs, SquashfsVersion};
use crate::v4::reader::BufReadSeek;

pub mod compressor;
pub mod data;
pub mod dir;
pub mod entry;
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

        // Convert v3 dir_blocks from Vec<(u64, Vec<u8>)> to (IntMap<u64, u64>, Vec<u8>)
        let mut dir_block_map = IntMap::default();
        let mut dir_data = Vec::new();
        let mut current_offset = 0;
        for (block_offset, block_data) in v3_squashfs.dir_blocks {
            dir_block_map.insert(block_offset, current_offset);
            current_offset += block_data.len() as u64;
            dir_data.extend(block_data);
        }

        Ok(GenericSquashfs {
            kind: v3_squashfs.kind,
            superblock: v3_squashfs.superblock,
            compression_options: v3_squashfs.compression_options,
            inodes: v3_squashfs.inodes,
            root_inode: v3_squashfs.root_inode,
            dir_blocks: (dir_block_map, dir_data),
            fragments: v3_squashfs.fragments,
            export: v3_squashfs.export,
            id: v3_squashfs.id.unwrap_or_default(),
            file: v3_squashfs.file,
        })
    }

    fn into_filesystem_reader(
        squashfs: GenericSquashfs<'b, Self>,
    ) -> Result<Self::FilesystemReader, crate::BackhandError> {
        // Convert dir_blocks from (IntMap<u64, u64>, Vec<u8>) back to Vec<(u64, Vec<u8>)>
        let (dir_block_map, dir_data) = squashfs.dir_blocks;
        let mut dir_blocks = Vec::new();
        let mut sorted_blocks: Vec<_> = dir_block_map.into_iter().collect();
        sorted_blocks.sort_by_key(|(_, offset)| *offset);

        for i in 0..sorted_blocks.len() {
            let (block_offset, data_offset) = sorted_blocks[i];
            let next_offset = if i + 1 < sorted_blocks.len() {
                sorted_blocks[i + 1].1
            } else {
                dir_data.len() as u64
            };
            let block_data = dir_data[data_offset as usize..next_offset as usize].to_vec();
            dir_blocks.push((block_offset, block_data));
        }

        let v3_squashfs = squashfs::Squashfs {
            kind: squashfs.kind,
            superblock: squashfs.superblock,
            compression_options: squashfs.compression_options,
            inodes: squashfs.inodes,
            root_inode: squashfs.root_inode,
            dir_blocks,
            fragments: squashfs.fragments,
            export: squashfs.export,
            id: Some(squashfs.id),
            uid: None,  // v3 compatibility: not used in GenericSquashfs
            guid: None, // v3 compatibility: not used in GenericSquashfs
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
