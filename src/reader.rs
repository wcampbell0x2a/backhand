//! Reader traits

use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::io::{Read, Seek, SeekFrom, Write};

use deku::bitvec::BitView;
use deku::prelude::*;
use tracing::{instrument, trace};

use crate::error::SquashfsError;
use crate::fragment::Fragment;
use crate::inode::Inode;
use crate::squashfs::{Export, Id, SuperBlock};
use crate::{fragment, metadata};

/// Private struct containing logic to read the `Squashfs` section from a file
pub struct SquashfsReaderWithOffset<R: SquashFsReader> {
    io: R,
    /// Offset from start of file to squashfs
    offset: u64,
}

impl<R: SquashFsReader> SquashfsReaderWithOffset<R> {
    pub fn new(io: R, offset: u64) -> Self {
        Self { io, offset }
    }
}

impl<R: SquashFsReader> Read for SquashfsReaderWithOffset<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.io.read(buf)
    }
}

impl<R: SquashFsReader> Seek for SquashfsReaderWithOffset<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(start) => self.io.seek(SeekFrom::Start(self.offset + start)),
            seek => self.io.seek(seek).map(|x| x - self.offset),
        }
    }
}

impl<T: Read + Seek> SquashFsReader for T {}
/// Squashfs data extraction methods implemented over [`Read`] and [`Seek`]
pub trait SquashFsReader: Read + Seek {
    /// Read in entire data and fragments
    #[instrument(skip_all)]
    fn data_and_fragments(&mut self, superblock: &SuperBlock) -> Result<Vec<u8>, SquashfsError> {
        self.rewind()?;
        let mut buf = vec![0u8; superblock.inode_table as usize];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    #[instrument(skip_all)]
    fn inodes(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<HashMap<u32, Inode, BuildHasherDefault<twox_hash::XxHash64>>, SquashfsError> {
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
            let mut bytes = metadata::read_block(self, superblock)?;
            ret_bytes.append(&mut bytes);
        }
        //tracing::trace!("TRACE: TOTAL BYTES: {02x?}", ret_bytes.len());

        let mut ret_vec = HashMap::default();
        while !ret_bytes.is_empty() {
            let input_bits = ret_bytes.view_bits::<deku::bitvec::Msb0>();
            match Inode::read(input_bits, (superblock.block_size, superblock.block_log)) {
                Ok((rest, inode)) => {
                    // Push the new Inode to the return, with the position this was read from
                    ret_vec.insert(inode.header.inode_number, inode);
                    ret_bytes = rest.domain().region().unwrap().1.to_vec();
                },
                Err(e) => {
                    // TODO: this should return an error
                    panic!("{e}");
                },
            }
        }

        Ok(ret_vec)
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    #[instrument(skip_all)]
    fn root_inode(&mut self, superblock: &SuperBlock) -> Result<Inode, SquashfsError> {
        let root_inode_start = (superblock.root_inode >> 16) as usize;
        let root_inode_offset = (superblock.root_inode & 0xffff) as usize;
        trace!("root_inode_start:  0x{root_inode_start:02x?}");
        trace!("root_inode_offset: 0x{root_inode_offset:02x?}");

        // Assumptions are made here that the root inode fits within two metadatas
        let seek = superblock.inode_table + root_inode_start as u64;
        self.seek(SeekFrom::Start(seek))?;
        let mut bytes_01 = metadata::read_block(self, superblock)?;
        let bytes_02 = metadata::read_block(self, superblock)?;
        bytes_01.write_all(&bytes_02)?;
        let new_bytes = &bytes_01[root_inode_offset..];

        let input_bits = new_bytes.view_bits::<::deku::bitvec::Msb0>();
        match Inode::read(input_bits, (superblock.block_size, superblock.block_log)) {
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
    ) -> Result<Vec<(u64, Vec<u8>)>, SquashfsError> {
        let seek = superblock.dir_table;
        self.seek(SeekFrom::Start(seek))?;
        let mut all_bytes = vec![];
        while self.stream_position()? != end_ptr {
            let metadata_start = self.stream_position()?;
            let bytes = metadata::read_block(self, superblock)?;
            all_bytes.push((metadata_start - seek, bytes));
        }

        Ok(all_bytes)
    }

    /// Parse Fragment Table
    #[instrument(skip_all)]
    fn fragments(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<Option<(u64, Vec<Fragment>)>, SquashfsError> {
        if superblock.frag_count == 0 || superblock.frag_table == 0xffffffffffffffff {
            return Ok(None);
        }
        let (ptr, table) = self.lookup_table::<Fragment>(
            superblock,
            superblock.frag_table,
            u64::from(superblock.frag_count) * fragment::SIZE as u64,
        )?;

        Ok(Some((ptr, table)))
    }

    /// Parse Export Table
    #[instrument(skip_all)]
    fn export(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<Option<(u64, Vec<Export>)>, SquashfsError> {
        if superblock.nfs_export_table_exists() && superblock.export_table != 0xffffffffffffffff {
            let ptr = superblock.export_table;
            let count = (superblock.inode_count as f32 / 1024_f32).ceil() as u64;
            let (ptr, table) = self.lookup_table::<Export>(superblock, ptr, count)?;
            Ok(Some((ptr, table)))
        } else {
            Ok(None)
        }
    }

    /// Parse ID Table
    #[instrument(skip_all)]
    fn id(&mut self, superblock: &SuperBlock) -> Result<Option<(u64, Vec<Id>)>, SquashfsError> {
        if superblock.id_count > 0 {
            let ptr = superblock.id_table;
            let count = superblock.id_count as u64;
            let (ptr, table) = self.lookup_table::<Id>(superblock, ptr, count)?;
            Ok(Some((ptr, table)))
        } else {
            Ok(None)
        }
    }
    /// Parse Lookup Table
    #[instrument(skip_all)]
    fn lookup_table<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        size: u64,
    ) -> Result<(u64, Vec<T>), SquashfsError> {
        // find the pointer at the initial offset
        self.seek(SeekFrom::Start(seek))?;
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        let ptr = u32::from_le_bytes(buf);

        let block_count = (size as f32 / 8192_f32).ceil() as u64;

        let ptr = u64::from(ptr);
        let table = self.metadata_with_count::<T>(superblock, ptr, block_count)?;

        Ok((ptr, table))
    }

    /// Parse count of `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadata_with_count<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        superblock: &SuperBlock,
        seek: u64,
        count: u64,
    ) -> Result<Vec<T>, SquashfsError> {
        self.seek(SeekFrom::Start(seek))?;

        let mut all_bytes = vec![];
        for _ in 0..count {
            let mut bytes = metadata::read_block(self, superblock)?;
            all_bytes.append(&mut bytes);
        }

        let mut ret_vec = vec![];
        // Read until we fail to turn bytes into `T`
        while let Ok(((rest, _), t)) = T::from_bytes((&all_bytes, 0)) {
            ret_vec.push(t);
            all_bytes = rest.to_vec();
        }

        Ok(ret_vec)
    }
}
