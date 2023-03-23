//! Reader traits

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};

use deku::bitvec::{BitView, Msb0};
use deku::prelude::*;
use rustc_hash::FxHashMap;
use tracing::{error, instrument, trace};

use crate::error::BackhandError;
use crate::fragment::Fragment;
use crate::inode::Inode;
use crate::squashfs::{Export, Id, Kind, SuperBlock};
use crate::{fragment, metadata};

/// Private struct containing logic to read the `Squashfs` section from a file
#[derive(Debug)]
pub struct SquashfsReaderWithOffset<R: ReadSeek> {
    io: R,
    /// Offset from start of file to squashfs
    offset: u64,
}

impl<R: ReadSeek> SquashfsReaderWithOffset<R> {
    pub fn new(mut io: R, offset: u64) -> std::io::Result<Self> {
        io.seek(SeekFrom::Start(offset))?;
        Ok(Self { io, offset })
    }
}

impl<R: ReadSeek> Read for SquashfsReaderWithOffset<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.io.read(buf)
    }
}

impl<R: ReadSeek> Seek for SquashfsReaderWithOffset<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let seek = match pos {
            SeekFrom::Start(start) => SeekFrom::Start(self.offset + start),
            seek => seek,
        };
        self.io.seek(seek).map(|x| x - self.offset)
    }
}

/// Similar to to Seek, but only require the `rewind` function
pub trait SeekRewind {
    /// Set the IO position back at the start
    fn rewind(&mut self) -> std::io::Result<()>;
}

impl<T: Seek> SeekRewind for T {
    fn rewind(&mut self) -> std::io::Result<()> {
        <Self as Seek>::rewind(self)
    }
}

/// Pseudo-Trait for Read + Seek
pub trait ReadRewind: Read + SeekRewind {}
impl<T: Read + SeekRewind> ReadRewind for T {}

/// Pseudo-Trait for Read + Seek
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

/// Pseudo-Trait for Write + Seek
pub trait WriteSeek: Write + Seek {}
impl<T: Write + Seek> WriteSeek for T {}

impl<T: ReadSeek> SquashFsReader for T {}

/// Squashfs data extraction methods implemented over [`Read`] and [`Seek`]
pub trait SquashFsReader: ReadSeek {
    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    #[instrument(skip_all)]
    fn inodes(
        &mut self,
        superblock: &SuperBlock,
        kind: Kind,
    ) -> Result<FxHashMap<u32, Inode>, BackhandError> {
        self.seek(SeekFrom::Start(superblock.inode_table))?;

        // The directory inodes store the total, uncompressed size of the entire listing, including headers.
        // Using this size, a SquashFS reader can determine if another header with further entries
        // should be following once it reaches the end of a run.

        let mut ret_bytes = vec![];

        //let og_len = buf.len();
        let mut metadata_offsets = vec![];
        //let mut rest = buf;
        let start = self.stream_position()?;
        while self.stream_position()? < superblock.dir_table {
            trace!("offset: {:02x?}", self.stream_position());
            metadata_offsets.push(self.stream_position()? - start);
            // parse into metadata
            let mut bytes = metadata::read_block(self, superblock, kind)?;
            ret_bytes.append(&mut bytes);
        }
        //tracing::trace!("TRACE: TOTAL BYTES: {02x?}", ret_bytes.len());

        let mut ret_vec = HashMap::default();
        while !ret_bytes.is_empty() {
            let input_bits = ret_bytes.view_bits::<deku::bitvec::Msb0>();
            match Inode::read(
                input_bits,
                (
                    superblock.bytes_used,
                    superblock.block_size,
                    superblock.block_log,
                    kind,
                ),
            ) {
                Ok((rest, inode)) => {
                    // Push the new Inode to the return, with the position this was read from
                    ret_vec.insert(inode.header.inode_number, inode);
                    ret_bytes = rest.domain().region().unwrap().1.to_vec();
                },
                Err(e) => {
                    error!("corrupted or invalid squashfs {e}");
                    return Err(BackhandError::CorruptedOrInvalidSquashfs);
                },
            }
        }

        Ok(ret_vec)
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    #[instrument(skip_all)]
    fn root_inode(&mut self, superblock: &SuperBlock, kind: Kind) -> Result<Inode, BackhandError> {
        let root_inode_start = (superblock.root_inode >> 16) as usize;
        let root_inode_offset = (superblock.root_inode & 0xffff) as usize;
        trace!("root_inode_start:  0x{root_inode_start:02x?}");
        trace!("root_inode_offset: 0x{root_inode_offset:02x?}");
        if (root_inode_start as u64) > superblock.bytes_used {
            error!("root_inode_offset > bytes_used");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Assumptions are made here that the root inode fits within two metadatas
        let seek = superblock.inode_table + root_inode_start as u64;
        self.seek(SeekFrom::Start(seek))?;
        let mut bytes_01 = metadata::read_block(self, superblock, kind)?;
        let bytes_02 = metadata::read_block(self, superblock, kind)?;
        bytes_01.write_all(&bytes_02)?;
        if root_inode_offset > bytes_01.len() {
            error!("root_inode_offset > bytes.len()");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        let new_bytes = &bytes_01[root_inode_offset..];

        let input_bits = new_bytes.view_bits::<::deku::bitvec::Msb0>();
        match Inode::read(
            input_bits,
            (
                superblock.bytes_used,
                superblock.block_size,
                superblock.block_log,
                kind,
            ),
        ) {
            Ok((_, inode)) => Ok(inode),
            Err(e) => Err(e.into()),
        }
    }

    /// Parse required number of `Metadata`s uncompressed blocks required for `Dir`s
    #[instrument(skip_all)]
    fn dir_blocks(
        &mut self,
        superblock: &SuperBlock,
        end_ptr: u64,
        kind: Kind,
    ) -> Result<Vec<(u64, Vec<u8>)>, BackhandError> {
        let seek = superblock.dir_table;
        self.seek(SeekFrom::Start(seek))?;
        let mut all_bytes = vec![];
        while self.stream_position()? != end_ptr {
            let metadata_start = self.stream_position()?;
            let bytes = metadata::read_block(self, superblock, kind)?;
            all_bytes.push((metadata_start - seek, bytes));
        }

        Ok(all_bytes)
    }

    /// Parse Fragment Table
    #[instrument(skip_all)]
    fn fragments(
        &mut self,
        superblock: &SuperBlock,
        kind: Kind,
    ) -> Result<Option<(u64, Vec<Fragment>)>, BackhandError> {
        if superblock.frag_count == 0 || superblock.frag_table == SuperBlock::NOT_SET {
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
    #[instrument(skip_all)]
    fn export(
        &mut self,
        superblock: &SuperBlock,
        kind: Kind,
    ) -> Result<Option<(u64, Vec<Export>)>, BackhandError> {
        if superblock.nfs_export_table_exists() && superblock.export_table != SuperBlock::NOT_SET {
            let ptr = superblock.export_table;
            let count = (superblock.inode_count as f32 / 1024_f32).ceil() as u64;
            let (ptr, table) = self.lookup_table::<Export>(superblock, ptr, count, kind)?;
            Ok(Some((ptr, table)))
        } else {
            Ok(None)
        }
    }

    /// Parse ID Table
    #[instrument(skip_all)]
    fn id(&mut self, superblock: &SuperBlock, kind: Kind) -> Result<(u64, Vec<Id>), BackhandError> {
        let ptr = superblock.id_table;
        let count = superblock.id_count as u64;
        let (ptr, table) = self.lookup_table::<Id>(superblock, ptr, count, kind)?;
        Ok((ptr, table))
    }

    /// Parse Lookup Table
    #[instrument(skip_all)]
    fn lookup_table<T: for<'a> DekuRead<'a, Kind>>(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        size: u64,
        kind: Kind,
    ) -> Result<(u64, Vec<T>), BackhandError> {
        // find the pointer at the initial offset
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        trace!("{:02x?}", buf);

        let bv = buf.view_bits::<deku::bitvec::Msb0>();
        let (_, ptr) = u64::read(bv, kind.type_endian)?;

        let block_count = (size as f32 / 8192_f32).ceil() as u64;

        let ptr = ptr;
        trace!("ptr: {:02x?}", ptr);
        let table = self.metadata_with_count::<T>(superblock, ptr, block_count, kind)?;

        Ok((ptr, table))
    }

    /// Parse count of `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadata_with_count<T: for<'a> DekuRead<'a, Kind>>(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        count: u64,
        kind: Kind,
    ) -> Result<Vec<T>, BackhandError> {
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;

        let mut all_bytes = vec![];
        for _ in 0..count {
            let mut bytes = metadata::read_block(self, superblock, kind)?;
            all_bytes.append(&mut bytes);
        }

        let mut ret_vec = vec![];
        let mut all_bytes = all_bytes.view_bits::<Msb0>();
        // Read until we fail to turn bytes into `T`
        while let Ok((rest, t)) = T::read(all_bytes, kind) {
            ret_vec.push(t);
            all_bytes = rest;
        }

        Ok(ret_vec)
    }
}
