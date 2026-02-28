//! Reader traits

use no_std_io2::io::{Read, Seek};
use std::collections::HashMap;
use std::io::{BufRead, Cursor, SeekFrom, Write};

use deku::prelude::*;
use solana_nohash_hasher::IntMap;
use tracing::{error, trace};

use super::export::Export;
use super::fragment::Fragment;
use super::inode::Inode;
use super::metadata::METADATA_MAXSIZE;
use super::squashfs::SuperBlock;
use super::{fragment, metadata};
use crate::error::BackhandError;
use crate::kinds::Kind;

/// Private struct containing logic to read the `Squashfs` section from a file
#[derive(Debug)]
pub(crate) struct SquashfsReaderWithOffset<R: BufReadSeek> {
    io: R,
    /// Offset from start of file to squashfs
    offset: u64,
}

impl<R: BufReadSeek> SquashfsReaderWithOffset<R> {
    pub fn new(mut io: R, offset: u64) -> std::io::Result<Self> {
        io.seek(SeekFrom::Start(offset))?;
        Ok(Self { io, offset })
    }
}

impl<R: BufReadSeek> BufRead for SquashfsReaderWithOffset<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.io.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.io.consume(amt)
    }
}

impl<R: BufReadSeek> Read for SquashfsReaderWithOffset<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.io.read(buf)
    }
}

impl<R: BufReadSeek> Seek for SquashfsReaderWithOffset<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let seek = match pos {
            SeekFrom::Start(start) => SeekFrom::Start(self.offset + start),
            seek => seek,
        };
        self.io.seek(seek).map(|x| x - self.offset)
    }
}

/// Pseudo-Trait for BufRead + Seek
pub trait BufReadSeek: BufRead + Seek + Send {}
impl<T: BufRead + Seek + Send> BufReadSeek for T {}

/// Pseudo-Trait for Write + Seek
pub trait WriteSeek: Write + Seek {}
impl<T: Write + Seek> WriteSeek for T {}

impl<T: BufReadSeek> SquashFsReader for T {}

/// Squashfs data extraction methods implemented over [`Read`] and [`Seek`]
pub trait SquashFsReader: BufReadSeek {
    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    fn inodes(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<IntMap<u32, Inode>, BackhandError> {
        self.seek(SeekFrom::Start(superblock.inode_table_start))?;

        // The directory inodes store the total, uncompressed size of the entire listing, including headers.
        // Using this size, a SquashFS reader can determine if another header with further entries
        // should be following once it reaches the end of a run.

        let mut next = vec![];

        let mut metadata_offsets = vec![];
        let mut ret_vec = HashMap::default();
        let start = self.stream_position()?;

        while self.stream_position()? < superblock.directory_table_start {
            metadata_offsets.push(self.stream_position()? - start);
            // parse into metadata
            let mut bytes = metadata::read_block(self, superblock, kind)?;

            // parse as many inodes as you can
            let mut inode_bytes = next;
            inode_bytes.append(&mut bytes);
            let mut c_inode_bytes = Cursor::new(inode_bytes.clone());
            let mut container = Reader::new(&mut c_inode_bytes);

            // store last successful read position
            let mut container_bits_read = container.bits_read;
            loop {
                match Inode::from_reader_with_ctx(
                    &mut container,
                    (
                        superblock.bytes_used,
                        superblock.block_size,
                        superblock.block_log,
                        kind.inner.type_endian,
                        kind.inner.bit_order.unwrap(),
                    ),
                ) {
                    Ok(inode) => {
                        ret_vec.insert(inode.header.inode_number, inode);
                        container_bits_read = container.bits_read;
                    }
                    Err(e) => {
                        if matches!(e, DekuError::Incomplete(_)) {
                            // try next block, inodes can span multiple blocks!
                            next = inode_bytes.clone()[(container_bits_read / 8)..].to_vec();
                            break;
                        } else {
                            error!("Fatal error parsing inode: {:?}", e);
                            return Err(BackhandError::Deku(e));
                        }
                    }
                }
            }
        }

        if ret_vec.len() != superblock.inode_count.try_into().unwrap() {
            error!("Parsed {} inodes, expected {}", ret_vec.len(), superblock.inode_count);
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        Ok(ret_vec)
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    fn root_inode(&mut self, superblock: &SuperBlock, kind: &Kind) -> Result<Inode, BackhandError> {
        let root_inode_start = (superblock.root_inode >> 16) as usize;
        let root_inode_offset = (superblock.root_inode & 0xffff) as usize;
        tracing::info!("root_inode_start:  0x{root_inode_start:02x?}");
        tracing::info!("root_inode_offset: 0x{root_inode_offset:02x?}");
        if (root_inode_start as u64) > superblock.bytes_used {
            error!("root_inode_offset > bytes_used");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Assumptions are made here that the root inode fits within two metadatas
        let seek = superblock.inode_table_start + root_inode_start as u64;
        self.seek(SeekFrom::Start(seek))?;
        let mut bytes_01 = metadata::read_block(self, superblock, kind)?;

        // try reading just one metdata block
        if root_inode_offset > bytes_01.len() {
            error!("root_inode_offset > bytes.len()");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        let mut cursor = Cursor::new(&bytes_01[root_inode_offset..]);
        let mut new_bytes = Reader::new(&mut cursor);
        if let Ok(inode) = Inode::from_reader_with_ctx(
            &mut new_bytes,
            (
                superblock.bytes_used,
                superblock.block_size,
                superblock.block_log,
                kind.inner.type_endian,
                kind.inner.bit_order.unwrap(),
            ),
        ) {
            tracing::info!("ROOT: {:?}", inode);
            return Ok(inode);
        }

        // if that doesn't work, we need another block
        let bytes_02 = metadata::read_block(self, superblock, kind)?;
        bytes_01.write_all(&bytes_02)?;
        if root_inode_offset > bytes_01.len() {
            error!("root_inode_offset > bytes.len()");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        let mut cursor = Cursor::new(&bytes_01[root_inode_offset..]);
        let mut new_bytes = Reader::new(&mut cursor);
        match Inode::from_reader_with_ctx(
            &mut new_bytes,
            (
                superblock.bytes_used,
                superblock.block_size,
                superblock.block_log,
                kind.inner.type_endian,
                kind.inner.bit_order.unwrap(),
            ),
        ) {
            Ok(inode) => Ok(inode),
            Err(e) => Err(e.into()),
        }
    }

    /// Parse required number of `Metadata`s uncompressed blocks required for `Dir`s
    fn dir_blocks(
        &mut self,
        superblock: &SuperBlock,
        end_ptr: u64,
        kind: &Kind,
    ) -> Result<(IntMap<u64, u64>, Vec<u8>), BackhandError> {
        let seek = superblock.directory_table_start;
        self.seek(SeekFrom::Start(seek))?;
        let mut block_map = IntMap::default();
        let mut dir_data = Vec::new();

        while self.stream_position()? != end_ptr {
            let metadata_start = self.stream_position()?;
            let bytes = metadata::read_block(self, superblock, kind)?;
            let compressed_offset = metadata_start - seek;
            let decompressed_offset = dir_data.len() as u64;
            block_map.insert(compressed_offset, decompressed_offset);
            dir_data.extend(bytes);
        }

        Ok((block_map, dir_data))
    }

    /// Parse Fragment Table
    fn fragments(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<Option<(u64, Vec<Fragment>)>, BackhandError> {
        let (ptr, table) = self.fragment_lookup_table(
            superblock,
            superblock.fragment_table_start,
            u64::from(superblock.fragments) * fragment::SIZE as u64,
            kind,
        )?;
        trace!("{:02x?}", table);
        Ok(Some((ptr, table)))
    }

    /// Parse Export Table
    fn export(
        &mut self,
        _superblock: &SuperBlock,
        _kind: &Kind,
    ) -> Result<Option<(u64, Vec<Export>)>, BackhandError> {
        Ok(None)
    }

    /// Parse UID Table
    fn uid(&mut self, superblock: &SuperBlock, kind: &Kind) -> Result<Vec<u16>, BackhandError> {
        let ptr = superblock.uid_start;
        let count = superblock.no_uids as u64;
        self.seek(SeekFrom::Start(ptr))?;

        // I wish self was Read here, but this works
        let mut buf = vec![0u8; count as usize * core::mem::size_of::<u16>()];
        self.read_exact(&mut buf)?;

        let mut cursor = Cursor::new(buf);
        let mut deku_reader = Reader::new(&mut cursor);
        let mut table = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let v = u16::from_reader_with_ctx(
                &mut deku_reader,
                (kind.inner.type_endian, kind.inner.bit_order.unwrap()),
            )?;
            table.push(v);
        }

        Ok(table)
    }

    /// Parse GUID Table
    fn guid(&mut self, superblock: &SuperBlock, kind: &Kind) -> Result<Vec<u16>, BackhandError> {
        let ptr = superblock.guid_start;
        let count = superblock.no_guids as u64;
        self.seek(SeekFrom::Start(ptr))?;

        // I wish self was Read here, but this works
        let mut buf = vec![0u8; count as usize * core::mem::size_of::<u16>()];
        self.read_exact(&mut buf)?;

        let mut cursor = Cursor::new(buf);
        let mut deku_reader = Reader::new(&mut cursor);
        let mut table = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let v = u16::from_reader_with_ctx(
                &mut deku_reader,
                (kind.inner.type_endian, kind.inner.bit_order.unwrap()),
            )?;
            table.push(v);
        }

        Ok(table)
    }

    /// Parse Fragment Lookup Table (specialized for Fragment context)
    fn fragment_lookup_table(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        size: u64,
        kind: &Kind,
    ) -> Result<(u64, Vec<Fragment>), BackhandError> {
        trace!(
            "fragment_lookup_table: seek=0x{:x}, size={}, fragments={}",
            seek, size, superblock.fragments
        );

        // V3 fragment table parsing follows the same pattern as v4:
        // 1. Read index table that points to metadata blocks
        // 2. Read metadata blocks to get fragment entries

        // Calculate number of metadata blocks needed
        let fragment_count = superblock.fragments as u64;
        let fragment_bytes = fragment_count * fragment::SIZE as u64;
        let metadata_block_count = fragment_bytes.div_ceil(METADATA_MAXSIZE as u64);

        trace!(
            "fragment_lookup_table: {} fragments need {} metadata blocks",
            fragment_count, metadata_block_count
        );

        // Read the index table (pointers to metadata blocks)
        self.seek(SeekFrom::Start(seek))?;
        let index_size = metadata_block_count * core::mem::size_of::<u64>() as u64;
        let mut index_buf = vec![0u8; index_size as usize];
        self.read_exact(&mut index_buf)?;

        // Parse the index table
        let mut index_ptrs = vec![];
        let mut cursor = Cursor::new(&index_buf);
        let mut reader = Reader::new(&mut cursor);

        for i in 0..metadata_block_count {
            let ptr = u64::from_reader_with_ctx(
                &mut reader,
                (kind.inner.type_endian, kind.inner.bit_order.unwrap()),
            )?;
            trace!("Fragment metadata block {}: pointer 0x{:x}", i, ptr);
            index_ptrs.push(ptr);
        }

        // Read fragments from metadata blocks
        let mut ret_vec = vec![];
        let mut fragments_read = 0;

        for (i, &ptr) in index_ptrs.iter().enumerate() {
            if fragments_read >= fragment_count {
                break;
            }

            let fragments_in_this_block = core::cmp::min(
                fragment_count - fragments_read,
                METADATA_MAXSIZE as u64 / fragment::SIZE as u64,
            );

            trace!(
                "Reading {} fragments from metadata block {} at 0x{:x}",
                fragments_in_this_block, i, ptr
            );

            self.seek(SeekFrom::Start(ptr))?;
            let block_fragments = self.fragment_metadata_with_count(superblock, ptr, 1, kind)?;

            // Only take the fragments we need
            let take_count =
                core::cmp::min(block_fragments.len(), fragments_in_this_block as usize);
            ret_vec.extend_from_slice(&block_fragments[..take_count]);
            fragments_read += take_count as u64;
        }

        trace!("fragment_lookup_table: successfully read {} fragments", ret_vec.len());
        Ok((seek, ret_vec))
    }

    /// Parse count of Fragment `Metadata` blocks
    fn fragment_metadata_with_count(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        count: u64,
        kind: &Kind,
    ) -> Result<Vec<Fragment>, BackhandError> {
        trace!("fragment_metadata_with_count: seek=0x{:02x}, count={}", seek, count);
        self.seek(SeekFrom::Start(seek))?;

        let mut all_bytes = vec![];
        for i in 0..count {
            let pos_before = self.stream_position()?;
            let mut bytes = metadata::read_block(self, superblock, kind)?;
            let pos_after = self.stream_position()?;
            trace!(
                "fragment metadata block {}: pos 0x{:x} -> 0x{:x}, read {} decompressed bytes, first 20: {:02x?}",
                i,
                pos_before,
                pos_after,
                bytes.len(),
                &bytes[..core::cmp::min(20, bytes.len())]
            );
            all_bytes.append(&mut bytes);
        }

        trace!(
            "fragment_metadata_with_count: total decompressed bytes: {}, content: {:02x?}",
            all_bytes.len(),
            &all_bytes[..core::cmp::min(50, all_bytes.len())]
        );

        let mut ret_vec = vec![];
        // Read until we fail to turn bytes into Fragment
        let mut cursor = Cursor::new(&all_bytes);
        let mut container = Reader::new(&mut cursor);
        loop {
            match Fragment::from_reader_with_ctx(
                &mut container,
                (kind.inner.type_endian, kind.inner.bit_order.unwrap()),
            ) {
                Ok(t) => {
                    trace!("Parsed fragment: {:?}", t);
                    ret_vec.push(t);
                }
                Err(e) => {
                    trace!("Failed to parse more fragments: {:?}", e);
                    break;
                }
            }
        }

        Ok(ret_vec)
    }
}
