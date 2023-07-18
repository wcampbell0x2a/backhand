use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};

use deku::bitvec::{BitView, Msb0};
use deku::prelude::*;
use rustc_hash::FxHashMap;
use tracing::{error, info, trace};

use crate::compression::Compressor;
use crate::kind::Kind;
use crate::metadata::{self, METADATA_MAXSIZE};
use crate::reader::WriteSeek;
use crate::squashfs::{SuperBlock_V3_0, SuperBlock_V4_0, NOT_SET};
use crate::{fragment, BackhandError, BufReadSeek, Export, Fragment, Id, Inode, Squashfs};

/// Squashfs data extraction methods implemented over [`Read`] and [`Seek`]
pub trait SquashFsReaderV4: BufReadSeek {
    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    fn inodes(
        &mut self,
        superblock: &SuperBlock_V4_0,
        kind: &Kind,
    ) -> Result<FxHashMap<u32, Inode>, BackhandError> {
        self.seek(SeekFrom::Start(superblock.inode_table))?;

        // The directory inodes store the total, uncompressed size of the entire listing, including headers.
        // Using this size, a SquashFS reader can determine if another header with further entries
        // should be following once it reaches the end of a run.

        let mut ret_bytes = Vec::with_capacity(METADATA_MAXSIZE);

        let mut metadata_offsets = vec![];
        let mut ret_vec = HashMap::default();
        let start = self.stream_position()?;

        while self.stream_position()? < superblock.dir_table {
            metadata_offsets.push(self.stream_position()? - start);
            // parse into metadata
            let mut bytes = metadata::read_block(self, superblock.compressor, kind)?;

            // parse as many inodes as you can
            ret_bytes.append(&mut bytes);

            let mut input_bits = ret_bytes.view_bits::<deku::bitvec::Msb0>();
            while !input_bits.is_empty() {
                match Inode::read(
                    input_bits,
                    (
                        superblock.bytes_used,
                        superblock.block_size,
                        superblock.block_log,
                        kind.inner.type_endian,
                    ),
                ) {
                    Ok((rest, inode)) => {
                        // Push the new Inode to the return, with the position this was read from
                        ret_vec.insert(inode.header.inode_number, inode);
                        input_bits = rest;
                    },
                    Err(_) => {
                        // try next block, inodes can span multiple blocks!
                        break;
                    },
                }
            }

            // save leftover bits to new bits to leave for the next metadata block
            // this is safe, input_bits is always byte aligned
            ret_bytes = ret_bytes[(ret_bytes.len() - (input_bits.len() / 8))..].to_vec();
        }

        Ok(ret_vec)
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    fn root_inode(
        &mut self,
        superblock: &SuperBlock_V4_0,
        kind: &Kind,
    ) -> Result<Inode, BackhandError> {
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
        let mut bytes_01 = metadata::read_block(self, superblock.compressor, kind)?;

        // try reading just one metdata block
        if root_inode_offset > bytes_01.len() {
            error!("root_inode_offset > bytes.len()");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        let new_bytes = &bytes_01[root_inode_offset..];
        let input_bits = new_bytes.view_bits::<::deku::bitvec::Msb0>();
        if let Ok((_, inode)) = Inode::read(
            input_bits,
            (
                superblock.bytes_used,
                superblock.block_size,
                superblock.block_log,
                kind.inner.type_endian,
            ),
        ) {
            return Ok(inode);
        }

        // if that doesn't work, we need another block
        let bytes_02 = metadata::read_block(self, superblock.compressor, kind)?;
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
                kind.inner.type_endian,
            ),
        ) {
            Ok((_, inode)) => Ok(inode),
            Err(e) => Err(e.into()),
        }
    }

    /// Parse required number of `Metadata`s uncompressed blocks required for `Dir`s
    fn dir_blocks(
        &mut self,
        superblock: &SuperBlock_V4_0,
        end_ptr: u64,
        kind: &Kind,
    ) -> Result<Vec<(u64, Vec<u8>)>, BackhandError> {
        let seek = superblock.dir_table;
        self.seek(SeekFrom::Start(seek))?;
        let mut all_bytes = vec![];
        while self.stream_position()? != end_ptr {
            let metadata_start = self.stream_position()?;
            let bytes = metadata::read_block(self, superblock.compressor, kind)?;
            all_bytes.push((metadata_start - seek, bytes));
        }

        Ok(all_bytes)
    }

    /// Parse Fragment Table
    fn fragments(
        &mut self,
        superblock: &SuperBlock_V4_0,
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
        superblock: &SuperBlock_V4_0,
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

    /// Parse ID Table
    fn id(
        &mut self,
        superblock: &SuperBlock_V4_0,
        kind: &Kind,
    ) -> Result<(u64, Vec<Id>), BackhandError> {
        let ptr = superblock.id_table;
        let count = superblock.id_count as u64;
        let (ptr, table) = self.lookup_table::<Id>(superblock, ptr, count, kind)?;
        Ok((ptr, table))
    }

    /// Parse Lookup Table
    fn lookup_table<T: for<'a> DekuRead<'a, deku::ctx::Endian>>(
        &mut self,
        superblock: &SuperBlock_V4_0,
        seek: u64,
        size: u64,
        kind: &Kind,
    ) -> Result<(u64, Vec<T>), BackhandError> {
        // find the pointer at the initial offset
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        trace!("{:02x?}", buf);

        let bv = buf.view_bits::<deku::bitvec::Msb0>();
        let (_, ptr) = u64::read(bv, kind.inner.type_endian)?;

        let block_count = (size as f32 / METADATA_MAXSIZE as f32).ceil() as u64;

        let ptr = ptr;
        trace!("ptr: {:02x?}", ptr);
        let table = self.metadata_with_count::<T>(superblock, ptr, block_count, kind)?;

        Ok((ptr, table))
    }

    /// Parse count of `Metadata` block at offset into `T`
    fn metadata_with_count<T: for<'a> DekuRead<'a, deku::ctx::Endian>>(
        &mut self,
        superblock: &SuperBlock_V4_0,
        seek: u64,
        count: u64,
        kind: &Kind,
    ) -> Result<Vec<T>, BackhandError> {
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;

        let mut all_bytes = vec![];
        for _ in 0..count {
            let mut bytes = metadata::read_block(self, superblock.compressor, kind)?;
            all_bytes.append(&mut bytes);
        }

        let mut ret_vec = vec![];
        let mut all_bytes = all_bytes.view_bits::<Msb0>();
        // Read until we fail to turn bytes into `T`
        while let Ok((rest, t)) = T::read(all_bytes, kind.inner.type_endian) {
            ret_vec.push(t);
            all_bytes = rest;
        }

        Ok(ret_vec)
    }
}
impl<T: BufReadSeek> SquashFsReaderV4 for T {}

pub trait SquashFsReaderV3: BufReadSeek {
    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    fn inodes(
        &mut self,
        superblock: &SuperBlock_V3_0,
        kind: &Kind,
    ) -> Result<FxHashMap<u32, Inode>, BackhandError> {
        self.seek(SeekFrom::Start(superblock.inode_table_start))?;

        // The directory inodes store the total, uncompressed size of the entire listing, including headers.
        // Using this size, a SquashFS reader can determine if another header with further entries
        // should be following once it reaches the end of a run.

        let mut ret_bytes = Vec::with_capacity(METADATA_MAXSIZE);

        let mut metadata_offsets = vec![];
        let mut ret_vec = HashMap::default();
        let start = self.stream_position()?;

        while self.stream_position()? < superblock.directory_table_start {
            metadata_offsets.push(self.stream_position()? - start);
            // parse into metadata
            let mut bytes = metadata::read_block(self, Compressor::Gzip, kind)?;

            // parse as many inodes as you can
            ret_bytes.append(&mut bytes);

            let mut input_bits = ret_bytes.view_bits::<deku::bitvec::Msb0>();
            while !input_bits.is_empty() {
                trace!("{:?}", input_bits.len()/8);
                let bytes: &[u8] = input_bits.domain().region().unwrap().1;
                tracing::debug!("{:02x?}", &bytes);
                match Inode::read(
                    input_bits,
                    (
                        superblock.bytes_used,
                        superblock.block_size,
                        superblock.block_log,
                        kind.inner.type_endian,
                    ),
                ) {
                    Ok((rest, inode)) => {
                        // Push the new Inode to the return, with the position this was read from
                        trace!("{:02x?}", inode);
                        ret_vec.insert(inode.header.inode_number, inode);
                        input_bits = rest;
                    },
                    Err(e) => {
                        error!("e: {e}");
                        // try next block, inodes can span multiple blocks!
                        break;
                    },
                }
            }

            // save leftover bits to new bits to leave for the next metadata block
            // this is safe, input_bits is always byte aligned
            ret_bytes = ret_bytes[(ret_bytes.len() - (input_bits.len() / 8))..].to_vec();
        }

        Ok(ret_vec)
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    fn root_inode(
        &mut self,
        superblock: &SuperBlock_V3_0,
        kind: &Kind,
    ) -> Result<Inode, BackhandError> {
        let root_inode_start = (superblock.root_inode >> 16) as usize;
        let root_inode_offset = (superblock.root_inode & 0xffff) as usize;
        trace!("root_inode_start:  0x{root_inode_start:02x?}");
        trace!("root_inode_offset: 0x{root_inode_offset:02x?}");
        if (root_inode_start as u64) > superblock.bytes_used {
            error!("root_inode_offset > bytes_used");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }

        // Assumptions are made here that the root inode fits within two metadatas
        let seek = superblock.inode_table_start + root_inode_start as u64;
        self.seek(SeekFrom::Start(seek))?;
        let mut bytes_01 = metadata::read_block(self, Compressor::Gzip, kind)?;

        // try reading just one metdata block
        if root_inode_offset > bytes_01.len() {
            error!("root_inode_offset > bytes.len()");
            return Err(BackhandError::CorruptedOrInvalidSquashfs);
        }
        let new_bytes = &bytes_01[root_inode_offset..];
        let input_bits = new_bytes.view_bits::<::deku::bitvec::Msb0>();
        if let Ok((_, inode)) = Inode::read(
            input_bits,
            (
                superblock.bytes_used,
                superblock.block_size,
                superblock.block_log,
                kind.inner.type_endian,
            ),
        ) {
            return Ok(inode);
        }

        // if that doesn't work, we need another block
        let bytes_02 = metadata::read_block(self, Compressor::Gzip, kind)?;
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
                kind.inner.type_endian,
            ),
        ) {
            Ok((_, inode)) => Ok(inode),
            Err(e) => Err(e.into()),
        }
    }

    /// Parse required number of `Metadata`s uncompressed blocks required for `Dir`s
    fn dir_blocks(
        &mut self,
        superblock: &SuperBlock_V3_0,
        end_ptr: u64,
        kind: &Kind,
    ) -> Result<Vec<(u64, Vec<u8>)>, BackhandError> {
        todo!();
    }

    /// Parse Fragment Table
    fn fragments(
        &mut self,
        superblock: &SuperBlock_V3_0,
        kind: &Kind,
    ) -> Result<Option<(u64, Vec<Fragment>)>, BackhandError> {
        if superblock.fragments == 0 || superblock.fragment_table_start == NOT_SET {
            return Ok(None);
        }
        let (ptr, table) = self.lookup_table::<Fragment>(
            superblock,
            superblock.fragment_table_start,
            u64::from(superblock.fragments) * fragment::SIZE as u64,
            kind,
        )?;

        Ok(Some((ptr, table)))
    }

    /// Parse Export Table
    //fn export(
    //    &mut self,
    //    superblock: &SuperBlock_V3_0,
    //    kind: &Kind,
    //) -> Result<Option<(u64, Vec<Export>)>, BackhandError> {
    //}

    /// Parse UID Table
    fn uid(
        &mut self,
        superblock: &SuperBlock_V3_0,
        kind: &Kind,
        // TODO: change tuple into struct
    ) -> Result<(u64, Vec<Id>), BackhandError> {
        let ptr = superblock.uid_start;
        let count = superblock.no_uids as u64;
        let (ptr, table) = self.lookup_table::<Id>(superblock, ptr, count, kind)?;
        Ok((ptr, table))
    }

    /// Parse GUID Table
    fn guid(
        &mut self,
        superblock: &SuperBlock_V3_0,
        kind: &Kind,
        // TODO: change tuple into struct
    ) -> Result<(u64, Vec<Id>), BackhandError> {
        let ptr = superblock.guid_start;
        let count = superblock.no_guids as u64;
        let (ptr, table) = self.lookup_table::<Id>(superblock, ptr, count, kind)?;
        Ok((ptr, table))
    }

    /// Parse Lookup Table
    fn lookup_table<T: for<'a> DekuRead<'a, deku::ctx::Endian>>(
        &mut self,
        superblock: &SuperBlock_V3_0,
        seek: u64,
        size: u64,
        kind: &Kind,
    ) -> Result<(u64, Vec<T>), BackhandError> {
        // find the pointer at the initial offset
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        trace!("{:02x?}", buf);

        let bv = buf.view_bits::<deku::bitvec::Msb0>();
        let (_, ptr) = u64::read(bv, kind.inner.type_endian)?;

        let block_count = (size as f32 / METADATA_MAXSIZE as f32).ceil() as u64;

        let ptr = ptr;
        trace!("ptr: {:02x?}", ptr);
        let table = self.metadata_with_count::<T>(superblock, ptr, block_count, kind)?;

        Ok((ptr, table))
    }

    /// Parse count of `Metadata` block at offset into `T`
    fn metadata_with_count<T: for<'a> DekuRead<'a, deku::ctx::Endian>>(
        &mut self,
        superblock: &SuperBlock_V3_0,
        seek: u64,
        count: u64,
        kind: &Kind,
    ) -> Result<Vec<T>, BackhandError> {
        trace!("seek: {:02x?}", seek);
        self.seek(SeekFrom::Start(seek))?;

        let mut all_bytes = vec![];
        for _ in 0..count {
            let mut bytes = metadata::read_block(self, Compressor::Gzip, kind)?;
            all_bytes.append(&mut bytes);
        }

        let mut ret_vec = vec![];
        let mut all_bytes = all_bytes.view_bits::<Msb0>();
        // Read until we fail to turn bytes into `T`
        while let Ok((rest, t)) = T::read(all_bytes, kind.inner.type_endian) {
            ret_vec.push(t);
            all_bytes = rest;
        }

        Ok(ret_vec)
    }
}
impl<T: BufReadSeek> SquashFsReaderV3 for T {}
