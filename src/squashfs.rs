//! Module containing `Squashfs`
//!
//! # `Squashfs`
//! Contains both the `SuperBlock` and the parsed fields/sections that are stored in a squashfs image
//! and are populated with a call to `Squashfs::from_reader(..)`.

use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use deku::bitvec::{BitVec, Msb0};
use deku::ctx::Endian;
use deku::prelude::*;
use flate2::read::ZlibEncoder;
use flate2::Compression;
use tracing::{debug, info, instrument, trace};
use xz2::read::{XzDecoder, XzEncoder};
use xz2::stream::{Check, Filters, LzmaOptions, MtStreamBuilder};

use crate::error::SquashfsError;
use crate::fragment::FRAGMENT_SIZE;
use crate::{
    BasicDirectory, BasicFile, CompressionOptions, Compressor, Dir, DirEntry, Fragment, Inode,
    Metadata,
};

#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "big")]
pub struct Export(u64);

#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "big")]
pub struct Id(u32);

#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct SuperBlock {
    // Superblock
    #[deku(assert_eq = "0x73717368")]
    magic: u32,
    inode_count: u32,
    mod_time: u32,
    // TODO: remove assert, see BasicFile::count()
    #[deku(assert_eq = "0x20000")]
    block_size: u32,
    frag_count: u32,
    compressor: Compressor,
    // TODO: remove assert, see BasicFile::count()
    #[deku(assert_eq = "0x11")]
    block_log: u16,
    //TODO: read as u16, then map to flags
    flags: u16,
    id_count: u16,
    //TODO: add assert
    version_major: u16,
    //TODO: add assert
    version_minor: u16,
    root_inode: u64,
    bytes_used: u64,
    id_table: u64,
    //TODO: add read into Squashfs
    xattr_table: u64,
    inode_table: u64,
    dir_table: u64,
    frag_table: u64,
    //TODO: add read into Squashfs
    export_table: u64,
}

impl SuperBlock {
    // Extract size of optional compression options
    pub fn compression_options_size(&self) -> Option<usize> {
        if self.compressor_options_are_present() {
            let size = match self.compressor {
                Compressor::Lzma => 0,
                Compressor::Gzip => 8,
                Compressor::Lzo => 8,
                Compressor::Xz => 8,
                Compressor::Lz4 => 8,
                Compressor::Zstd => 4,
                Compressor::None => 0,
            };
            // add metadata size
            Some(size + 2)
        } else {
            None
        }
    }

    pub fn inodes_uncompressed(&self) -> bool {
        self.flags & Flags::InodesStoredUncompressed as u16 != 0
    }

    pub fn data_block_stored_uncompressed(&self) -> bool {
        self.flags & Flags::DataBlockStoredUncompressed as u16 != 0
    }

    pub fn fragments_stored_uncompressed(&self) -> bool {
        self.flags & Flags::FragmentsStoredUncompressed as u16 != 0
    }

    pub fn fragments_are_not_used(&self) -> bool {
        self.flags & Flags::FragmentsAreNotUsed as u16 != 0
    }

    pub fn fragments_are_always_generated(&self) -> bool {
        self.flags & Flags::FragmentsAreAlwaysGenerated as u16 != 0
    }

    pub fn data_has_been_duplicated(&self) -> bool {
        self.flags & Flags::DataHasBeenDeduplicated as u16 != 0
    }

    pub fn nfs_export_table_exists(&self) -> bool {
        self.flags & Flags::NFSExportTableExists as u16 != 0
    }

    pub fn xattrs_are_stored_uncompressed(&self) -> bool {
        self.flags & Flags::XattrsAreStoredUncompressed as u16 != 0
    }

    pub fn no_xattrs_in_archive(&self) -> bool {
        self.flags & Flags::NoXattrsInArchive as u16 != 0
    }

    pub fn compressor_options_are_present(&self) -> bool {
        self.flags & Flags::CompressorOptionsArePresent as u16 != 0
    }
}

// TODO: is_flagname() for all of these in `SuperBlock`
pub enum Flags {
    InodesStoredUncompressed    = 0b0000_0000_0000_0001,
    DataBlockStoredUncompressed = 0b0000_0000_0000_0010,
    Unused                      = 0b0000_0000_0000_0100,
    FragmentsStoredUncompressed = 0b0000_0000_0000_1000,
    FragmentsAreNotUsed         = 0b0000_0000_0001_0000,
    FragmentsAreAlwaysGenerated = 0b0000_0000_0010_0000,
    DataHasBeenDeduplicated     = 0b0000_0000_0100_0000,
    NFSExportTableExists        = 0b0000_0000_1000_0000,
    XattrsAreStoredUncompressed = 0b0000_0001_0000_0000,
    NoXattrsInArchive           = 0b0000_0010_0000_0000,
    CompressorOptionsArePresent = 0b0000_0100_0000_0000,
}

pub struct Squashfs {
    pub superblock: SuperBlock,
    pub compression_options: Option<CompressionOptions>,

    /// Section containing Data and Fragments.
    ///
    /// This also contains the superblock and option bytes b/c that is how BasicFile uses its
    /// blocks_starts.
    /// TODO: maybe bzero those bytes just in case?
    pub data_and_fragments: Vec<u8>,
    pub inodes: Vec<Inode>,
    pub root_inode: BasicDirectory,
    pub dir_blocks: Vec<Vec<u8>>,
    pub fragments: Option<Vec<Fragment>>,
    pub export: Option<Vec<Export>>,
    pub id: Option<Vec<Id>>,
}

/// public
impl Squashfs {
    /// Create `Squashfs` from `Read`er, with the resulting squashfs having read all fields needed
    /// to regenerate the original squashfs and interact with the fs in memory without needing to
    /// read again from `Read`er.
    #[instrument(skip_all)]
    pub fn from_reader<R: ReadSeek + 'static>(mut reader: R) -> Result<Squashfs, SquashfsError> {
        // Size of metadata + optional compression options metadata block
        let mut superblock = [0u8; 96];
        reader.read_exact(&mut superblock)?;

        // Parse SuperBlock
        let (_, superblock) = SuperBlock::from_bytes((&superblock, 0))?;
        trace!("{superblock:#08x?}");

        // Parse Compression Options, if any
        let compression_options = if superblock.compressor != Compressor::None {
            if let Some(size) = superblock.compression_options_size() {
                let mut options = vec![0u8; size];
                reader.read_exact(&mut options)?;

                let bv = BitVec::from_slice(&options).unwrap();
                let (_, m) = Metadata::read(&bv, ()).unwrap();

                // data -> compression options
                let bv = BitVec::from_slice(&m.data).unwrap();
                let (_, c) = CompressionOptions::read(
                    &bv,
                    (deku::ctx::Endian::Little, superblock.compressor),
                )?;
                Some(c)
            } else {
                None
            }
        } else {
            None
        };
        trace!("{compression_options:08x?}");

        // Create SquashfsReader
        let mut squashfs_io = SquashfsReader {
            io: Box::new(reader),
        };

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Data and Fragments");
        let data_and_fragments = squashfs_io.data_and_fragments(&superblock)?;
        info!("Reading Inodes");
        let inodes = squashfs_io.inodes(&superblock)?;
        info!("Reading Root Inode");
        let root_inode = squashfs_io.root_inode(&superblock)?;
        info!("Reading Dirs");
        let dir_blocks = squashfs_io.dir_blocks(&superblock, &inodes)?;
        info!("Reading Fragments");
        let fragments = squashfs_io.fragments(&superblock)?;
        info!("Reading Exports");
        let export = squashfs_io.export(&superblock)?;
        info!("Reading Ids");
        let id = squashfs_io.id(&superblock)?;

        let squashfs = Squashfs {
            superblock,
            compression_options,
            data_and_fragments,
            inodes,
            root_inode,
            dir_blocks,
            fragments,
            export,
            id,
        };

        Ok(squashfs)
    }

    /// Serialize `Squashfs` to bytes
    ///
    /// Give our current `Squashfs` write all fields to the file, with an updated `Superblock` that
    /// corrently identifies the position of the fields.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SquashfsError> {
        let mut c = Cursor::new(vec![]);

        // copy of the superblock to write the new positions, but we don't mutate the one stored in
        // Squashfs
        let mut write_superblock = self.superblock;

        c.write_all(&[0x00; 96])?;
        // Compression Options
        info!("Writing compressions options");
        if self.compression_options.is_some() {
            //TODO: make correct by writing the length and uncompressed Metadata
            c.write_all(&[0x08, 0x80])?;
            let mut bv: BitVec<Msb0, u8> = BitVec::new();
            self.compression_options
                .write(&mut bv, (Endian::Little, self.superblock.compressor))?;
            c.write_all(bv.as_raw_slice())?;
        }

        // Data and Fragment Bytes
        c.write_all(
            &self.data_and_fragments
                [96 + self.superblock.compression_options_size().unwrap_or(0)..],
        )?;

        // Inode Bytes
        info!("Writing Inodes");
        write_superblock.inode_table = c.position();
        let inode_bytes = {
            let bytes = self
                .inodes
                .iter()
                .flat_map(|a| a.to_bytes().unwrap())
                .collect();
            if self.superblock.inodes_uncompressed() {
                bytes
            } else {
                compress(bytes, self.superblock.compressor, &self.compression_options).unwrap()
            }
        };
        c.write_all(&(inode_bytes.len() as u16).to_le_bytes())?;
        c.write_all(&inode_bytes)?;

        // Dir Bytes
        info!("Writing Dirs");
        write_superblock.dir_table = c.position();
        let dir_bytes: Vec<u8> = self.dir_blocks.iter().flatten().cloned().collect();
        let metadata_len = Metadata::set_if_compressed(dir_bytes.len() as u16).to_le_bytes();
        c.write_all(&metadata_len)?;
        c.write_all(&dir_bytes)?;

        // Fragment Lookup Table Bytes
        info!("Writing Fragment Lookup Table");
        if let Some(fragments) = &self.fragments {
            let fragment_table_dat = c.position();
            let bytes: Vec<u8> = fragments
                .iter()
                .flat_map(|a| a.to_bytes().unwrap())
                .collect();
            let metadata_len = Metadata::set_if_compressed(bytes.len() as u16).to_le_bytes();
            c.write_all(&metadata_len)?;
            c.write_all(&bytes)?;
            write_superblock.frag_table = c.position();
            c.write_all(&fragment_table_dat.to_le_bytes())?;
        }

        // Export Lookup Table
        info!("Writing Export Lookup Table");
        if let Some(export) = &self.export {
            let export_table_dat = c.position();
            let bytes: Vec<u8> = export.iter().flat_map(|a| a.to_bytes().unwrap()).collect();
            let metadata_len = Metadata::set_if_compressed(bytes.len() as u16).to_le_bytes();
            c.write_all(&metadata_len)?;
            c.write_all(&bytes)?;
            write_superblock.export_table = c.position();
            c.write_all(&export_table_dat.to_le_bytes())?;
        }

        // Export Id Table
        info!("Writing Export Id Table");
        if let Some(id) = &self.id {
            let id_table_dat = c.position();
            let bytes: Vec<u8> = id.iter().flat_map(|a| a.to_bytes().unwrap()).collect();
            let metadata_len = Metadata::set_if_compressed(bytes.len() as u16).to_le_bytes();
            c.write_all(&metadata_len)?;
            c.write_all(&bytes)?;
            write_superblock.id_table = c.position();
            c.write_all(&id_table_dat.to_le_bytes())?;
        }

        // Pad out block_size
        info!("Writing Padding");
        write_superblock.bytes_used = c.position();
        let blocks_used = write_superblock.bytes_used as u32 / 0x1000;
        let pad_len = (blocks_used + 1) * 0x1000;
        let pad_len = pad_len - write_superblock.bytes_used as u32;
        c.write_all(&vec![0x00; pad_len as usize])?;

        // Seek back the beginning and write the superblock
        info!("Writing Superblock");
        c.seek(SeekFrom::Start(0))?;
        c.write_all(&write_superblock.to_bytes().unwrap())?;

        info!("Writing Finished");
        Ok(c.into_inner())
    }

    /// Extract all files
    pub fn extract_all_files(&self) -> Result<Vec<(PathBuf, Vec<u8>)>, SquashfsError> {
        let mut ret = vec![];
        for inode in &self.inodes {
            if let Inode::BasicDirectory(basic_dir) = inode {
                let block = &self.dir_blocks[basic_dir.block_index as usize];
                let bytes = &block[basic_dir.block_offset as usize..];
                let (_, dir) = Dir::from_bytes((bytes, 0))?;
                for entry in &dir.dir_entries {
                    // TODO: use type enum
                    if entry.t == 2 {
                        debug!("{dir:?}");
                        ret.push(self.file(inode, dir.inode_num, entry)?);
                    }
                }
            }
        }

        Ok(ret)
    }

    /// Given a file_name from a squashfs filepath, extract the file and the filepath from the
    /// internal squashfs stored in the fields
    #[instrument(skip_all)]
    pub fn extract_file(&self, name: &str) -> Result<(PathBuf, Vec<u8>), SquashfsError> {
        // Search through inodes and parse directory table at the specified location
        // searching for first file name that matches
        for inode in &self.inodes {
            if let Inode::BasicDirectory(basic_dir) = inode {
                let block = &self.dir_blocks[basic_dir.block_index as usize];
                let bytes = &block[basic_dir.block_offset as usize..];
                let (_, dir) = Dir::from_bytes((bytes, 0))?;
                trace!("Searching following Dir for filename({name}): {dir:#02x?}");
                for entry in &dir.dir_entries {
                    // TODO: use type enum
                    if entry.t == 2 {
                        let entry_name = std::str::from_utf8(&entry.name)?;
                        debug!(entry_name);
                        if name == entry_name {
                            let file = self.file(inode, dir.inode_num, entry)?;
                            return Ok(file);
                        }
                    }
                }
            }
        }
        Err(SquashfsError::FileNotFound)
    }

    #[instrument(skip_all)]
    fn data(&self, basic_file: &BasicFile) -> Result<Vec<u8>, SquashfsError> {
        debug!("extracting: {:#02x?}", basic_file.header);
        let mut bytes = Cursor::new(&self.data_and_fragments[basic_file.blocks_start as usize..]);

        // Add data
        info!("Extracting Data");
        let mut data_bytes = vec![];
        for block_size in &basic_file.block_sizes {
            let uncompressed = block_size & (1 << 24) != 0;
            let size = block_size & !(1 << 24);
            let mut data = vec![0u8; size as usize];
            bytes.read_exact(&mut data)?;

            let mut bytes = if uncompressed {
                data
            } else {
                decompress(data, self.superblock.compressor)?
            };
            data_bytes.append(&mut bytes);
        }

        // Add fragments
        // TODO: this should be constant
        info!("Extracting Fragment");
        if basic_file.frag_index != 0xffffffff {
            if let Some(fragments) = &self.fragments {
                let frag = fragments[basic_file.frag_index as usize];
                let mut bytes = Cursor::new(&self.data_and_fragments[frag.start as usize..]);

                let uncompressed = frag.size & (1 << 24) != 0;
                let size = frag.size & !(1 << 24);

                let mut buf = vec![0u8; size as usize];
                bytes.read_exact(&mut buf)?;

                let mut bytes = if uncompressed {
                    buf
                } else {
                    decompress(buf, self.superblock.compressor)?
                };
                data_bytes.append(&mut bytes);
            }
        }

        data_bytes = data_bytes[basic_file.block_offset as usize..]
            [..basic_file.file_size as usize]
            .to_vec();

        Ok(data_bytes)
    }

    #[instrument(skip_all)]
    fn file(
        &self,
        found_inode: &Inode,
        found_inode_num: u32,
        found_entry: &DirEntry,
    ) -> Result<(PathBuf, Vec<u8>), SquashfsError> {
        //  TODO: remove
        let dir_inode = found_inode;
        let base_inode = found_inode_num;
        let entry = found_entry;
        let entry_name = std::str::from_utf8(&entry.name)?;
        let dir_inode = dir_inode.expect_dir();
        // Used for searching for the file later when we want to extract the bytes
        let looking_inode = base_inode as i16 + entry.inode_offset;

        let root_inode = self.root_inode.header.inode_number;
        debug!("searching for dir path to root inode: {:#02x?}", root_inode);

        let mut path_inodes = vec![];
        // first check if the dir inode of this file is the root inode
        if dir_inode.header.inode_number != root_inode {
            // check every inode and find the matching inode to the current dir_nodes parent
            // inode, when we find a new inode, check if it's the root inode, and continue on if it
            // isn't
            let mut next_inode = dir_inode.parent_inode;
            'outer: loop {
                for inode in &self.inodes {
                    if let Inode::BasicDirectory(basic_dir) = inode {
                        if basic_dir.header.inode_number == next_inode {
                            path_inodes.push(basic_dir);
                            if basic_dir.header.inode_number == root_inode {
                                break 'outer;
                            }
                            next_inode = basic_dir.parent_inode;
                        }
                    }
                }
            }
        }

        // Insert the first basic file
        path_inodes.insert(0, dir_inode);

        let mut paths = vec![];
        // Now we use n as the basic_dir with the inode, and look for that inode in the next
        // dir at the path directed from the path_inodes, when it matches, save it to the paths
        for n in 0..path_inodes.len() - 1 {
            let basic_dir_with_inode = path_inodes[n];
            let search_inode = basic_dir_with_inode.header.inode_number;

            let basic_dir_with_location = path_inodes[n + 1];
            let block = &self.dir_blocks[basic_dir_with_location.block_index as usize];
            let bytes = &block[basic_dir_with_location.block_offset as usize..];

            let (_, dir) = Dir::from_bytes((bytes, 0))?;
            let base_inode = dir.inode_num;

            for entry in &dir.dir_entries {
                let entry_name = String::from_utf8(entry.name.clone())?;
                if base_inode as i16 + entry.inode_offset == search_inode as i16 {
                    paths.push(entry_name);
                }
            }
        }

        // reverse the order, since we are looking at the file to the parent dirs
        let paths: Vec<&String> = paths.iter().rev().collect();

        // create PathBufs
        let mut pathbuf = PathBuf::new();
        for path in paths {
            pathbuf.push(path);
        }
        pathbuf.push(entry_name);
        debug!("path: {}", pathbuf.display());

        // look through basic file inodes in search of the one true basic_inode and extract the
        // bytes from the data and fragment sections
        for inode in &self.inodes {
            if let Inode::BasicFile(basic_file) = inode {
                if basic_file.header.inode_number == looking_inode as u32 {
                    return Ok((pathbuf, self.data(basic_file)?));
                }
            }
        }

        Err(SquashfsError::FileNotFound)
    }
}

pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

/// Private struct containing logic to read the `Squashfs` section from a file
struct SquashfsReader {
    io: Box<dyn ReadSeek>,
}

impl SquashfsReader {
    /// Read in entire data and fragments
    #[instrument(skip_all)]
    pub fn data_and_fragments(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<Vec<u8>, SquashfsError> {
        self.io.seek(SeekFrom::Start(0))?;
        let mut buf = vec![0u8; superblock.inode_table as usize];
        self.io.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Parse Inode Table into `Vec<(position_read, Inode)>`
    #[instrument(skip_all)]
    pub fn inodes(&mut self, superblock: &SuperBlock) -> Result<Vec<Inode>, SquashfsError> {
        self.metadatas::<Inode>(
            superblock,
            SeekFrom::Start(superblock.inode_table),
            superblock.dir_table - superblock.inode_table,
        )
    }

    /// Parse multiple `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadatas<T: for<'a> DekuContainerRead<'a> + std::fmt::Debug>(
        &mut self,
        superblock: &SuperBlock,
        seek: SeekFrom,
        size: u64,
    ) -> Result<Vec<T>, SquashfsError> {
        debug!("Metadata: seek {:02x?}, size: {:02x?}", seek, size);
        self.io.seek(seek)?;

        // The directory inodes store the total, uncompressed size of the entire listing, including headers.
        // Using this size, a SquashFS reader can determine if another header with further entries
        // should be following once it reaches the end of a run.

        // TODO: with capacity?
        let mut ret_bytes = vec![];

        let mut buf = vec![0u8; size as usize];
        self.io.read_exact(&mut buf)?;

        let og_len = buf.len();
        let mut metadata_offsets = vec![];
        let mut rest = buf;
        while !rest.is_empty() {
            // parse into metadata
            if let Ok(((inner_rest, _), m)) = Metadata::from_bytes((&rest, 0)) {
                metadata_offsets.push(og_len - rest.len());
                rest = inner_rest.to_vec();
                // decompress
                let mut bytes = if Metadata::is_compressed(m.len) {
                    decompress(m.data, superblock.compressor)?
                } else {
                    m.data
                };

                ret_bytes.append(&mut bytes);
            }
        }

        // TODO: with capacity?
        let mut ret_vec = vec![];
        while !ret_bytes.is_empty() {
            match T::from_bytes((&ret_bytes, 0)) {
                Ok(((rest, _), t)) => {
                    // Push the new T to the return, with the position this was read from
                    ret_vec.push(t);
                    ret_bytes = rest.to_vec();
                },
                Err(e) => {
                    return Err(e.into());
                },
            }
        }

        Ok(ret_vec)
    }

    /// Extract the root `Inode` as a `BasicDirectory`
    #[instrument(skip_all)]
    pub fn root_inode(
        &mut self,
        superblock: &SuperBlock,
        //inodes: &[(usize, usize, Inode)],
    ) -> Result<BasicDirectory, SquashfsError> {
        // I think we can always be in one metadata? This assumption is taken with this following
        // code
        let root_inode_start = (superblock.root_inode >> 16) as usize;
        let root_inode_offset = (superblock.root_inode & 0xfff) as usize;
        self.io.seek(SeekFrom::Start(
            superblock.inode_table + root_inode_start as u64,
        ))?;
        let mut buf = [0u8; 2];
        self.io.read_exact(&mut buf)?;
        let metadata_len = u16::from_le_bytes(buf);

        let byte_len = Metadata::len(metadata_len);
        let mut buf = vec![0u8; byte_len as usize];
        self.io.read_exact(&mut buf)?;

        let bytes = if Metadata::is_compressed(metadata_len) {
            decompress(buf, superblock.compressor)?
        } else {
            buf
        };

        let bytes = &bytes[root_inode_offset..];

        match Inode::from_bytes((bytes, 0)) {
            Ok((_, inode)) => Ok(inode.expect_dir().clone()),
            Err(e) => Err(e.into()),
        }
    }

    /// Parse required number of `Metadata`s uncompressed blocks required for `Dir`s
    #[instrument(skip_all)]
    pub fn dir_blocks(
        &mut self,
        superblock: &SuperBlock,
        inodes: &Vec<Inode>,
    ) -> Result<Vec<Vec<u8>>, SquashfsError> {
        let mut max_metadata = 0;
        for inode in inodes {
            if let Inode::BasicDirectory(basic_dir) = inode {
                if basic_dir.block_index > max_metadata {
                    max_metadata = basic_dir.block_index;
                }
            }
        }

        let offset = superblock.dir_table;
        let seek = SeekFrom::Start(offset);
        self.metadata_blocks(superblock, seek, u64::from(max_metadata) + 1, true)
    }

    /// Parse Fragment Table
    #[instrument(skip_all)]
    pub fn fragments(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<Option<Vec<Fragment>>, SquashfsError> {
        if superblock.frag_count == 0 {
            return Ok(None);
        }
        let fragment = self.lookup_table::<Fragment>(
            superblock,
            SeekFrom::Start(superblock.frag_table),
            u64::from(superblock.frag_count) * FRAGMENT_SIZE as u64,
        )?;

        Ok(Some(fragment))
    }

    /// Parse Export Table
    #[instrument(skip_all)]
    pub fn export(
        &mut self,
        superblock: &SuperBlock,
    ) -> Result<Option<Vec<Export>>, SquashfsError> {
        if superblock.nfs_export_table_exists() {
            let ptr = superblock.export_table;
            let count = (superblock.inode_count as f32 / 1024_f32).ceil() as u64;
            let res = self.lookup_table::<Export>(superblock, SeekFrom::Start(ptr), count)?;
            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    /// Parse ID Table
    #[instrument(skip_all)]
    pub fn id(&mut self, superblock: &SuperBlock) -> Result<Option<Vec<Id>>, SquashfsError> {
        if superblock.nfs_export_table_exists() {
            let ptr = superblock.id_table;
            let count = superblock.id_count as u64;
            let res = self.lookup_table::<Id>(superblock, SeekFrom::Start(ptr), count)?;
            Ok(Some(res))
        } else {
            Ok(None)
        }
    }
}

/// private
impl SquashfsReader {
    /// Parse Lookup Table
    #[instrument(skip_all)]
    fn lookup_table<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        superblock: &SuperBlock,
        seek: SeekFrom,
        size: u64,
    ) -> Result<Vec<T>, SquashfsError> {
        debug!(
            "Lookup Table: seek {:02x?}, metadata size: {:02x?}",
            seek, size
        );
        // find the pointer at the initial offset
        self.io.seek(seek)?;
        let mut buf = [0u8; 4];
        self.io.read_exact(&mut buf)?;
        let ptr = u32::from_le_bytes(buf);

        let block_count = (size as f32 / 8192_f32).ceil() as u64;

        self.metadata_with_count::<T>(
            superblock,
            SeekFrom::Start(u64::from(ptr)),
            block_count,
            false,
        )
    }

    /// Parse count of `Metadata` block at offset into `T`
    #[instrument(skip_all)]
    fn metadata_with_count<T: for<'a> DekuContainerRead<'a>>(
        &mut self,
        superblock: &SuperBlock,
        seek: SeekFrom,
        count: u64,
        can_be_compressed: bool,
    ) -> Result<Vec<T>, SquashfsError> {
        debug!(
            "Metadata with count: seek {:02x?}, count: {:02x?}",
            seek, count
        );
        self.io.seek(seek)?;

        let mut all_bytes = vec![];
        // in order to grab a `count` of Metadatas, we can't use Deku for usage of std::io::Read
        for _ in 0..count {
            let mut buf = [0u8; 2];
            self.io.read_exact(&mut buf)?;
            let metadata_len = u16::from_le_bytes(buf);

            let byte_len = Metadata::len(metadata_len);
            let mut buf = vec![0u8; byte_len as usize];
            self.io.read_exact(&mut buf)?;

            let mut bytes = if can_be_compressed && Metadata::is_compressed(metadata_len) {
                decompress(buf, superblock.compressor)?
            } else {
                buf
            };
            all_bytes.append(&mut bytes);
        }

        // TODO: with capacity?
        let mut ret_vec = vec![];
        // Read until we fail to turn bytes into `T`
        while let Ok(((rest, _), t)) = T::from_bytes((&all_bytes, 0)) {
            ret_vec.push(t);
            all_bytes = rest.to_vec();
        }

        Ok(ret_vec)
    }

    /// Parse into Metadata uncompressed blocks
    #[instrument(skip_all)]
    fn metadata_blocks(
        &mut self,
        superblock: &SuperBlock,
        seek: SeekFrom,
        count: u64,
        can_be_compressed: bool,
    ) -> Result<Vec<Vec<u8>>, SquashfsError> {
        debug!("Seeking to 0x{seek:02x?}");
        self.io.seek(seek)?;

        let mut all_bytes = vec![];
        // in order to grab a `count` of Metadatas, we can't use Deku for usage of std::io::Read
        for _ in 0..count {
            let mut buf = [0u8; 2];
            self.io.read_exact(&mut buf)?;
            let metadata_len = u16::from_le_bytes(buf);

            let byte_len = Metadata::len(metadata_len);
            let mut buf = vec![0u8; byte_len as usize];
            self.io.read_exact(&mut buf)?;

            let bytes = if can_be_compressed && Metadata::is_compressed(metadata_len) {
                decompress(buf, superblock.compressor)?
            } else {
                buf
            };
            all_bytes.push(bytes);
        }

        Ok(all_bytes)
    }
}

/// Using the current compressor from the superblock, decompress bytes
#[instrument(skip_all)]
fn decompress(bytes: Vec<u8>, compressor: Compressor) -> Result<Vec<u8>, SquashfsError> {
    let mut out = vec![];
    match compressor {
        Compressor::Gzip => {
            let mut decoder = flate2::read::ZlibDecoder::new(std::io::Cursor::new(bytes));
            decoder.read_to_end(&mut out)?;
        },
        Compressor::Xz => {
            let mut decoder = XzDecoder::new(std::io::Cursor::new(bytes));
            decoder.read_to_end(&mut out)?;
        },
        _ => return Err(SquashfsError::UnsupportedCompression(compressor)),
    }
    Ok(out)
}

fn compress(
    bytes: Vec<u8>,
    compressor: Compressor,
    options: &Option<CompressionOptions>,
) -> Result<Vec<u8>, SquashfsError> {
    match (compressor, options) {
        (Compressor::Xz, Some(CompressionOptions::Xz(xz))) => {
            let level = 7;
            let check = Check::Crc32;
            let mut opts = LzmaOptions::new_preset(level).unwrap();
            let dict_size = xz.dictionary_size;
            opts.dict_size(dict_size);

            let mut filters = Filters::new();
            filters.lzma2(&opts);

            let stream = MtStreamBuilder::new()
                .threads(2)
                .filters(filters)
                .check(check)
                .encoder()
                .unwrap();

            let mut encoder = XzEncoder::new_stream(Cursor::new(bytes), stream);
            let mut buf = vec![];
            encoder.read_to_end(&mut buf).unwrap();
            Ok(buf)
        },
        (Compressor::Xz, None) => {
            let level = 7;
            let check = Check::Crc32;
            let mut opts = LzmaOptions::new_preset(level).unwrap();
            let dict_size = 0x2000;
            opts.dict_size(dict_size);

            let mut filters = Filters::new();
            filters.lzma2(&opts);

            let stream = MtStreamBuilder::new()
                .threads(2)
                .filters(filters)
                .check(check)
                .encoder()
                .unwrap();

            let mut encoder = XzEncoder::new_stream(Cursor::new(bytes), stream);
            let mut buf = vec![];
            encoder.read_to_end(&mut buf).unwrap();
            Ok(buf)
        },
        (Compressor::Gzip, Some(CompressionOptions::Gzip(gzip))) => {
            // TODO(#8): Use window_size and strategies
            let mut encoder =
                ZlibEncoder::new(Cursor::new(bytes), Compression::new(gzip.compression_level));
            let mut buf = vec![];
            encoder.read_to_end(&mut buf).unwrap();
            Ok(buf)
        },
        (Compressor::Gzip, None) => {
            let mut encoder = ZlibEncoder::new(Cursor::new(bytes), Compression::new(9));
            let mut buf = vec![];
            encoder.read_to_end(&mut buf).unwrap();
            Ok(buf)
        },
        _ => todo!(),
    }
}
