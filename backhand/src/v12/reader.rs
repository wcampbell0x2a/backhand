//! Reading the tables of a SquashFS v1 or v2 image

use deku::prelude::*;
use no_std_io2::io::{Cursor, SeekFrom};
use solana_nohash_hasher::IntMap;

use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::v4::reader::BufReadSeek;
use crate::v12::inode::Layout;
use crate::v12::squashfs::SuperBlock;
use crate::v12::{fragment, metadata};

use crate::v4::id::Id;
use crate::v12::fragment::Fragment;

/// One table, decompressed into a single buffer
///
/// `block_map` maps the offset of each compressed metadata block, relative to
/// the table start, to that block's offset in `data`. Inodes and directory
/// listings are addressed by compressed block, so the map is what turns an
/// on-disk reference into a position in `data`.
#[derive(Debug, Default)]
pub struct MetadataTable {
    pub block_map: IntMap<u64, u64>,
    pub data: Vec<u8>,
}

impl MetadataTable {
    /// Resolve an on-disk `(block, offset)` reference to a position in `data`
    pub fn position(&self, block: u32, offset: u16) -> Option<usize> {
        let start = self.block_map.get(&u64::from(block))?;
        usize::try_from(start + u64::from(offset)).ok()
    }
}

pub trait SquashFsReader: BufReadSeek {
    /// Read every metadata block between two offsets into one buffer
    fn metadata_table(
        &mut self,
        superblock: &SuperBlock,
        start: u32,
        end: u32,
        kind: &Kind,
    ) -> Result<MetadataTable, BackhandError> {
        if end < start {
            error!("table end({end:#x}) is before its start({start:#x})");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        self.seek(SeekFrom::Start(u64::from(start)))?;

        let mut table = MetadataTable::default();
        // A block that ends past `end` means the image is malformed, so stop on
        // "at or past" rather than on an exact match: one overshoot would
        // otherwise read until the reader itself fails.
        while self.stream_position()? < u64::from(end) {
            let metadata_start = self.stream_position()?;
            let bytes = metadata::read_block(self, superblock, kind)?;
            table.block_map.insert(metadata_start - u64::from(start), table.data.len() as u64);
            table.data.extend(bytes);
        }

        Ok(table)
    }

    /// Read the inode table, which runs up to the directory table
    fn inode_table(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<MetadataTable, BackhandError> {
        self.metadata_table(
            superblock,
            superblock.inode_table_start,
            superblock.directory_table_start,
            kind,
        )
    }

    /// Read the directory table, which runs up to the fragment or uid table
    fn dir_table(
        &mut self,
        superblock: &SuperBlock,
        layout: Layout,
        kind: &Kind,
    ) -> Result<MetadataTable, BackhandError> {
        self.metadata_table(
            superblock,
            superblock.directory_table_start,
            superblock.directory_table_end(layout),
            kind,
        )
    }

    /// Read the uid table
    ///
    /// Entries are 32 bits wide in every version.
    fn uid_table(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<Vec<Id>, BackhandError> {
        self.id_table(superblock.uid_start, superblock.no_uids, kind)
    }

    /// Read the guid table
    fn guid_table(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<Vec<Id>, BackhandError> {
        self.id_table(superblock.guid_start, superblock.no_guids, kind)
    }

    fn id_table(&mut self, start: u32, count: u8, kind: &Kind) -> Result<Vec<Id>, BackhandError> {
        let count = usize::from(count);
        if count == 0 {
            return Ok(vec![]);
        }
        self.seek(SeekFrom::Start(u64::from(start)))?;

        let mut buf = vec![0u8; count * Id::SIZE];
        self.read_exact(&mut buf)?;

        let mut cursor = Cursor::new(buf);
        let mut deku_reader = Reader::new(&mut cursor);
        let mut table = Vec::with_capacity(count);
        for _ in 0..count {
            table.push(Id::from_reader_with_ctx(&mut deku_reader, kind.inner.type_endian)?);
        }

        Ok(table)
    }

    /// Read the fragment table, when the image has one
    ///
    /// The lookup entries are 32 bits, against v3's 64. Each names a metadata
    /// block holding eight-byte fragment entries.
    fn fragment_table(
        &mut self,
        superblock: &SuperBlock,
        layout: Layout,
        kind: &Kind,
    ) -> Result<Option<Vec<Fragment>>, BackhandError> {
        let Some(start) = superblock.fragment_table_start(layout) else {
            return Ok(None);
        };
        let count = superblock.fragments(layout) as usize;
        if count == 0 {
            return Ok(None);
        }

        let bytes = count * fragment::SIZE;
        let indexes = bytes.div_ceil(metadata::METADATA_MAXSIZE);

        self.seek(SeekFrom::Start(u64::from(start)))?;
        let mut index_buf = vec![0u8; indexes * core::mem::size_of::<u32>()];
        self.read_exact(&mut index_buf)?;

        let mut cursor = Cursor::new(index_buf);
        let mut deku_reader = Reader::new(&mut cursor);
        let mut block_starts = Vec::with_capacity(indexes);
        for _ in 0..indexes {
            block_starts.push(u32::from_reader_with_ctx(&mut deku_reader, kind.inner.type_endian)?);
        }

        let mut table_bytes = Vec::with_capacity(bytes);
        for block_start in block_starts {
            self.seek(SeekFrom::Start(u64::from(block_start)))?;
            table_bytes.extend(metadata::read_block(self, superblock, kind)?);
        }

        if table_bytes.len() < bytes {
            error!("fragment table is shorter than the superblock says");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        let mut cursor = Cursor::new(table_bytes);
        let mut deku_reader = Reader::new(&mut cursor);
        let order = kind.inner.bit_order.unwrap_or(deku::ctx::Order::Lsb0);
        let mut table = Vec::with_capacity(count);
        for _ in 0..count {
            table.push(Fragment::from_reader_with_ctx(
                &mut deku_reader,
                (kind.inner.type_endian, order),
            )?);
        }

        trace!("{:02x?}", table);
        Ok(Some(table))
    }
}

impl<T: BufReadSeek> SquashFsReader for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_resolves_through_the_block_map() {
        let mut table = MetadataTable::default();
        table.block_map.insert(0, 0);
        table.block_map.insert(0x1f5, 8192);
        table.data = vec![0u8; 9000];

        assert_eq!(table.position(0, 203), Some(203));
        assert_eq!(table.position(0x1f5, 10), Some(8202));
        // A block the image never wrote has no position.
        assert_eq!(table.position(0x999, 0), None);
    }
}
