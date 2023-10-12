//! Reader traits

use std::collections::HashMap;
use std::io::{BufRead, Cursor, Read, Seek, SeekFrom, Write};

use deku::prelude::*;
use rustc_hash::FxHashMap;
use tracing::{error, instrument, trace};

use crate::bufread::BufReadSeek;
use crate::error::BackhandError;
use crate::kinds::Kind;
use crate::v3::export::Export;
use crate::v3::fragment::Fragment;
use crate::v3::id::Id;
use crate::v3::inode::Inode;
use crate::v3::metadata::METADATA_MAXSIZE;
use crate::v3::squashfs::{SuperBlock, NOT_SET};
use crate::v3::{fragment, metadata};

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

impl<T: BufReadSeek> SquashFsReader for T {}

/// Squashfs data extraction methods implemented over [`Read`] and [`Seek`]
pub trait SquashFsReader: BufReadSeek {
    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    fn inodes(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<FxHashMap<u32, Inode>, BackhandError> {
        self.seek(SeekFrom::Start(u64::from(superblock.inode_table_start)))?;

        // The directory inodes store the total, uncompressed size of the entire listing, including headers.
        // Using this size, a SquashFS reader can determine if another header with further entries
        // should be following once it reaches the end of a run.

        let mut next = vec![];

        let mut metadata_offsets = vec![];
        let mut ret_vec = HashMap::default();
        let start = self.stream_position()?;

        while self.stream_position()? < u64::from(superblock.directory_table_start) {
            metadata_offsets.push(self.stream_position()? - start);
            // parse into metadata
            let mut bytes = metadata::read_block(self, superblock, kind)?;

            // parse as many inodes as you can
            let mut inode_bytes = next;
            inode_bytes.append(&mut bytes);
            let mut c_inode_bytes = Cursor::new(inode_bytes.clone());
            trace!("{:02x?}", &c_inode_bytes);
            let mut container = Reader::new(&mut c_inode_bytes);

            // store last successful read position
            let mut container_bits_read = container.bits_read;
            loop {
                match Inode::from_reader_with_ctx(
                    &mut container,
                    (
                        superblock.bytes_used,
                        u32::from(superblock.block_size_1),
                        superblock.block_log,
                        kind.inner.type_endian,
                        kind.inner.bit_order.unwrap(),
                    ),
                ) {
                    Ok(inode) => {
                        // Push the new Inode to the return, with the position this was read from
                        trace!("new: {inode:02x?}");
                        ret_vec.insert(inode.header.inode_number, inode);
                        container_bits_read = container.bits_read;
                    }
                    Err(e) => {
                        if matches!(e, DekuError::Incomplete(_)) {
                            // try next block, inodes can span multiple blocks!
                            next = inode_bytes.clone()[(container_bits_read / 8)..].to_vec();
                            break;
                        } else {
                            panic!("{:?}", e);
                        }
                    }
                }
            }
        }

        Ok(ret_vec)
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    #[instrument(skip_all)]
    fn root_inode(&mut self, superblock: &SuperBlock, kind: &Kind) -> Result<Inode, BackhandError> {
        let root_inode_start = (superblock.root_inode >> 16) as usize;
        let root_inode_offset = (superblock.root_inode & 0xffff) as usize;
        trace!("root_inode_start:  0x{root_inode_start:02x?}");
        trace!("root_inode_offset: 0x{root_inode_offset:02x?}");
        if (root_inode_start as u64) > superblock.bytes_used {
            error!("root_inode_offset > bytes_used");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Assumptions are made here that the root inode fits within two metadatas
        let seek = u64::from(superblock.inode_table_start) + root_inode_start as u64;
        self.seek(SeekFrom::Start(u64::from(seek)))?;
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
    #[instrument(skip_all)]
    fn dir_blocks(
        &mut self,
        superblock: &SuperBlock,
        end_ptr: u64,
        kind: &Kind,
    ) -> Result<Vec<(u64, Vec<u8>)>, BackhandError> {
        let seek = superblock.directory_table_start;
        self.seek(SeekFrom::Start(u64::from(seek)))?;
        let mut all_bytes = vec![];
        while self.stream_position()? != end_ptr {
            let metadata_start = self.stream_position()?;
            let bytes = metadata::read_block(self, superblock, kind)?;
            all_bytes.push((metadata_start - u64::from(seek), bytes));
        }

        Ok(all_bytes)
    }

    /// Parse Fragment Table
    #[instrument(skip_all)]
    fn fragments(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<Option<(u64, Vec<Fragment>)>, BackhandError> {
        // if superblock.fragments == 0 || superblock.fragment_table_start == NOT_SET {
        //     return Ok(None);
        // }
        let (ptr, table) = self.lookup_table::<Fragment>(
            superblock,
            u64::from(superblock.fragment_table_start),
            u64::from(superblock.fragments) * fragment::SIZE as u64,
            kind,
        )?;
        trace!("{:02x?}", table);
        Ok(Some((ptr, table)))
    }

    /// Parse Export Table
    #[instrument(skip_all)]
    fn export(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<Option<(u64, Vec<Export>)>, BackhandError> {
        Ok(None)
        // if superblock.nfs_export_table_exists() && superblock.export_table != NOT_SET {
        //     let ptr = superblock.export_table;
        //     let count = (superblock.inode_count as f32 / 1024_f32).ceil() as u64;
        //     let (ptr, table) = self.lookup_table::<Export>(superblock, ptr, count, kind)?;
        //     Ok(Some((ptr, table)))
        // } else {
        //     Ok(None)
        // }
    }

    /// Parse UID Table
    #[instrument(skip_all)]
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

    /// Parse Lookup Table
    #[instrument(skip_all)]
    fn lookup_table<T>(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        size: u64,
        kind: &Kind,
    ) -> Result<(u64, Vec<T>), BackhandError>
    where
        T: for<'a> DekuReader<'a, (deku::ctx::Endian, deku::ctx::Order)>,
    {
        // find the pointer at the initial offset
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;
        let buf: &mut [u8] = &mut [0u8; 8];
        self.read_exact(buf)?;
        trace!("{:02x?}", buf);

        let mut cursor = Cursor::new(buf);
        let mut deku_reader = Reader::new(&mut cursor);
        let ptr = u64::from_reader_with_ctx(
            &mut deku_reader,
            (kind.inner.type_endian, kind.inner.bit_order.unwrap()),
        )?;

        let block_count = (size as f32 / METADATA_MAXSIZE as f32).ceil() as u64;

        trace!("ptr: {:02x?}", ptr);
        let table = self.metadata_with_count::<T>(superblock, ptr, block_count, kind)?;

        Ok((ptr, table))
    }

    /// Parse count of `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadata_with_count<T>(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        count: u64,
        kind: &Kind,
    ) -> Result<Vec<T>, BackhandError>
    where
        T: for<'a> DekuReader<'a, (deku::ctx::Endian, deku::ctx::Order)>,
    {
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;

        let mut all_bytes = vec![];
        for _ in 0..count {
            let mut bytes = metadata::read_block(self, superblock, kind)?;
            all_bytes.append(&mut bytes);
        }

        let mut ret_vec = vec![];
        // Read until we fail to turn bytes into `T`
        let mut cursor = Cursor::new(all_bytes);
        let mut container = Reader::new(&mut cursor);
        while let Ok(t) = T::from_reader_with_ctx(
            &mut container,
            (kind.inner.type_endian, kind.inner.bit_order.unwrap()),
        ) {
            ret_vec.push(t);
        }

        Ok(ret_vec)
    }
}
