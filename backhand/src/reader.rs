//! Reader traits

use std::collections::HashMap;
use std::io::{BufRead, Cursor, Read, Seek, SeekFrom, Write};

use deku::prelude::*;
use solana_nohash_hasher::IntMap;
use tracing::{error, trace};

use crate::error::BackhandError;
use crate::export::Export;
use crate::fragment::Fragment;
use crate::id::Id;
use crate::inode::Inode;
use crate::kinds::Kind;
use crate::metadata::METADATA_MAXSIZE;
use crate::squashfs::{SuperBlock, NOT_SET};
use crate::{fragment, metadata};

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

impl<R> BufRead for SquashfsReaderWithOffset<R>
where
    R: BufReadSeek,
{
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.io.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.io.consume(amt)
    }
}

impl<R> Read for SquashfsReaderWithOffset<R>
where
    R: BufReadSeek,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.io.read(buf)
    }
}

impl<R> Seek for SquashfsReaderWithOffset<R>
where
    R: BufReadSeek,
{
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
pub trait SquashFsReader: BufReadSeek + Sized {
    /// Cache Inode Table
    /// # Returns
    /// - `(RootInode, HashMap<inode_number, Inode>)`
    fn inodes(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<(Inode, IntMap<u32, Inode>), BackhandError> {
        let (map, bytes) = self.uncompress_metadatas(
            superblock.inode_table,
            superblock,
            superblock.dir_table,
            kind,
        )?;

        let mut inodes = IntMap::default();
        // Be nice the allocator, and only allocate a max of u16::MAX count of Indoes
        inodes.try_reserve(superblock.inode_count.min(u16::MAX as u32) as usize)?;

        let byte_len = bytes.len();
        let mut cursor = Cursor::new(bytes);
        let mut reader = Reader::new(&mut cursor);
        while reader.bits_read != byte_len * 8 {
            let inode = Inode::from_reader_with_ctx(
                &mut reader,
                (
                    superblock.bytes_used,
                    superblock.block_size,
                    superblock.block_log,
                    kind.inner.type_endian,
                ),
            )?;
            inodes.insert(inode.header.inode_number, inode);
        }

        if inodes.len() != superblock.inode_count as usize {
            error!("inodes {} != superblock.inode_count {}", inodes.len(), superblock.inode_count);
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        let root_inode_start = (superblock.root_inode >> 16) as usize;
        let root_inode_offset = (superblock.root_inode & 0xffff) as usize;

        let Some(root_offset) = map.get(&(root_inode_start as u64)) else {
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        };

        let mut cursor = reader.into_inner();
        cursor.seek(SeekFrom::Start(root_offset + root_inode_offset as u64))?;

        let mut reader = Reader::new(&mut cursor);
        let root_inode = Inode::from_reader_with_ctx(
            &mut reader,
            (
                superblock.bytes_used,
                superblock.block_size,
                superblock.block_log,
                kind.inner.type_endian,
            ),
        )?;

        Ok((root_inode, inodes))
    }

    /// Parse required number of `Metadata`s uncompressed blocks required for `Dir`s
    ///
    /// # Returns
    /// - `(HashMap<offset_from_seek, offset_from_bytes>, Bytes)`
    fn uncompress_metadatas(
        &mut self,
        seek: u64,
        superblock: &SuperBlock,
        end_ptr: u64,
        kind: &Kind,
    ) -> Result<(IntMap<u64, u64>, Vec<u8>), BackhandError> {
        self.seek(SeekFrom::Start(seek))?;
        let mut map = HashMap::default();
        let mut all_bytes = vec![];
        while self.stream_position()? != end_ptr {
            let metadata_start = self.stream_position()?;
            let mut bytes = metadata::read_block(self, superblock, kind)?;
            map.insert(metadata_start - seek, all_bytes.len() as u64);
            all_bytes.append(&mut bytes);
        }

        Ok((map, all_bytes))
    }

    /// Parse and Cache Fragment Table
    fn fragments(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<Option<(u64, Vec<Fragment>)>, BackhandError> {
        if superblock.frag_count == 0 || superblock.frag_table == NOT_SET {
            return Ok(None);
        }
        let (ptr, table) = self.lookup_table::<Fragment>(
            superblock,
            superblock.frag_table,
            u64::from(superblock.frag_count) * fragment::SIZE as u64,
            kind,
        )?;

        Ok(Some((ptr, table)))
    }

    /// Parse Export Table
    fn export(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<Option<(u64, Vec<Export>)>, BackhandError> {
        if superblock.nfs_export_table_exists() && superblock.export_table != NOT_SET {
            let ptr = superblock.export_table;
            let count = (superblock.inode_count as f32 / 1024_f32).ceil() as u64;
            let (ptr, table) = self.lookup_table::<Export>(superblock, ptr, count, kind)?;
            Ok(Some((ptr, table)))
        } else {
            Ok(None)
        }
    }

    /// Parse and Cache ID Table
    fn id(
        &mut self,
        superblock: &SuperBlock,
        kind: &Kind,
    ) -> Result<(u64, Vec<Id>), BackhandError> {
        let ptr = superblock.id_table;
        let count = superblock.id_count as u64;
        let (ptr, table) = self.lookup_table::<Id>(superblock, ptr, count, kind)?;
        Ok((ptr, table))
    }

    /// Parse Lookup Table
    fn lookup_table<T>(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        size: u64,
        kind: &Kind,
    ) -> Result<(u64, Vec<T>), BackhandError>
    where
        T: for<'a> DekuReader<'a, deku::ctx::Endian>,
    {
        // find the pointer at the initial offset
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;
        let buf: &mut [u8] = &mut [0u8; 8];
        self.read_exact(buf)?;
        trace!("{:02x?}", buf);

        let mut cursor = Cursor::new(buf);
        let mut deku_reader = Reader::new(&mut cursor);
        let ptr = u64::from_reader_with_ctx(&mut deku_reader, kind.inner.type_endian)?;

        let block_count = (size as f32 / METADATA_MAXSIZE as f32).ceil() as u64;

        trace!("ptr: {:02x?}", ptr);
        let table = self.metadata_with_count::<T>(superblock, ptr, block_count, kind)?;

        Ok((ptr, table))
    }

    /// Parse count of `Metadata` block at offset into `T`
    fn metadata_with_count<T>(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        count: u64,
        kind: &Kind,
    ) -> Result<Vec<T>, BackhandError>
    where
        T: for<'a> DekuReader<'a, deku::ctx::Endian>,
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
        while let Ok(t) = T::from_reader_with_ctx(&mut container, kind.inner.type_endian) {
            ret_vec.push(t);
        }

        Ok(ret_vec)
    }
}
