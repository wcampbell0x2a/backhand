//! [`Squashfs`], [`Id`], and [`Export`]

use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use deku::bitvec::{BitVec, Msb0};
use deku::ctx::Endian;
use deku::prelude::*;
use tracing::{debug, info, instrument, trace};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::dir::{Dir, DirEntry};
use crate::error::SquashfsError;
use crate::fragment::Fragment;
use crate::inode::{BasicDirectory, BasicFile, Inode};
use crate::metadata;
use crate::reader::{ReadSeek, SquashfsReader};

/// NFS export support
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Export(u64);

/// 32 bit user and group IDs
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Id(u32);

/// Contains important information about the archive, including the locations of other sections
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct SuperBlock {
    // Superblock
    #[deku(assert_eq = "0x73717368")]
    pub magic: u32,
    pub inode_count: u32,
    pub mod_time: u32,
    pub block_size: u32,
    pub frag_count: u32,
    pub compressor: Compressor,
    pub block_log: u16,
    pub flags: u16,
    pub id_count: u16,
    #[deku(assert_eq = "4")]
    pub version_major: u16,
    #[deku(assert_eq = "0")]
    pub version_minor: u16,
    pub root_inode: u64,
    pub bytes_used: u64,
    pub id_table: u64,
    //TODO: add read into Squashfs
    pub xattr_table: u64,
    pub inode_table: u64,
    pub dir_table: u64,
    pub frag_table: u64,
    //TODO: add read into Squashfs
    pub export_table: u64,
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

#[derive(Debug)]
pub enum Unsquashfs {
    File((PathBuf, Vec<u8>)),
    /// (FilePath, original, link)
    Symlink((PathBuf, String, String)),
    Path(PathBuf),
}

/// Container for a Squashfs Image stored in memory
pub struct Squashfs {
    pub superblock: SuperBlock,
    pub compression_options: Option<CompressionOptions>,
    /// Section containing Data and Fragments.
    ///
    /// This also contains the superblock and option bytes b/c that is how BasicFile uses its
    /// blocks_starts.
    pub data_and_fragments: Vec<u8>,
    // All Inodes
    pub inodes: Vec<Inode>,
    /// Root Inode
    pub root_inode: BasicDirectory,
    /// Bytes containing Directory Table
    pub dir_blocks: Vec<(u64, Vec<u8>)>,
    /// Fragments Lookup Table
    pub fragments: Option<Vec<Fragment>>,
    /// Export Lookup Table
    pub export: Option<Vec<Export>>,
    /// Id Lookup Table
    pub id: Option<Vec<Id>>,
}

impl Squashfs {
    /// Create `Squashfs` from `Read`er, with the resulting squashfs having read all fields needed
    /// to regenerate the original squashfs and interact with the fs in memory without needing to
    /// read again from `Read`er. `reader` needs to start with the beginning of the Image.
    #[instrument(skip_all)]
    pub fn from_reader<R: ReadSeek + 'static>(reader: R) -> Result<Squashfs, SquashfsError> {
        Self::from_reader_with_offset(reader, 0)
    }

    /// Same as `from_reader`, but with a starting `offset` to the image in the `reader`
    pub fn from_reader_with_offset<R: ReadSeek + 'static>(
        mut reader: R,
        offset: u64,
    ) -> Result<Squashfs, SquashfsError> {
        // do the initial seek from the start of `reader`
        reader.seek(SeekFrom::Start(offset))?;

        // Size of metadata + optional compression options metadata block
        let mut superblock = [0u8; 96];
        reader.read_exact(&mut superblock)?;

        // Parse SuperBlock
        let (_, superblock) = SuperBlock::from_bytes((&superblock, 0))?;
        trace!("{superblock:#08x?}");

        // Parse Compression Options, if any
        info!("Reading Compression options");
        let compression_options = if superblock.compressor != Compressor::None {
            if let Some(size) = superblock.compression_options_size() {
                // ingore the size above, that is just a warning that some other information is in
                // the Option that isn't in the squashfs spec.
                //
                // This appears in TP-Link rootfs squashfs.
                let bytes = metadata::read_block(&mut reader, &superblock)?;

                if bytes.len() != size {
                    tracing::warn!(
                        "Non standard compression options! CompressionOptions might be incorrect"
                    );
                }
                // data -> compression options
                let bv = BitVec::from_slice(&bytes).unwrap();
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
        let mut squashfs_reader = SquashfsReader::new(reader, offset);

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Data and Fragments");
        let data_and_fragments = squashfs_reader.data_and_fragments(&superblock)?;
        info!("Reading Inodes");
        let inodes = squashfs_reader.inodes(&superblock)?;
        info!("Reading Root Inode");
        let root_inode = squashfs_reader.root_inode(&superblock)?;
        info!("Reading Dirs");
        let dir_blocks = squashfs_reader.dir_blocks(&superblock, &inodes)?;
        info!("Reading Fragments");
        let fragments = squashfs_reader.fragments(&superblock)?;
        info!("Reading Exports");
        let export = squashfs_reader.export(&superblock)?;
        info!("Reading Ids");
        let id = squashfs_reader.id(&superblock)?;

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
    /// Given our current `Squashfs`, write all fields to the file, with an updated `Superblock` that
    /// correctly identifies the position of the fields
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
                .flat_map(|a| {
                    let mut v = BitVec::<Msb0, u8>::new();
                    a.write(&mut v, (0, 0)).unwrap();
                    v.as_raw_slice().to_vec()
                })
                .collect();
            if self.superblock.inodes_uncompressed() {
                bytes
            } else {
                compressor::compress(bytes, self.superblock.compressor, &self.compression_options)?
            }
        };
        c.write_all(&(inode_bytes.len() as u16).to_le_bytes())?;
        c.write_all(&inode_bytes)?;

        // Dir Bytes
        info!("Writing Dirs");
        write_superblock.dir_table = c.position();
        let dir_bytes: Vec<u8> = self.dir_blocks.iter().flat_map(|a| &a.1).cloned().collect();
        let metadata_len = metadata::set_if_compressed(dir_bytes.len() as u16).to_le_bytes();
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
            let metadata_len = metadata::set_if_compressed(bytes.len() as u16).to_le_bytes();
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
            let metadata_len = metadata::set_if_compressed(bytes.len() as u16).to_le_bytes();
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
            let metadata_len = metadata::set_if_compressed(bytes.len() as u16).to_le_bytes();
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

    /// From a `block_index`, grab the bytes from that index and next and return the Dirs at that
    /// `block_offset`
    ///
    /// # Returns
    /// - `Ok(Some(Vec<Dir>))` when found dir
    /// - `Ok(None)`           when empty dir
    #[instrument(skip_all)]
    fn dir_from_index(
        &self,
        block_index: u64,
        file_size: u32,
        block_offset: usize,
    ) -> Result<Option<Vec<Dir>>, SquashfsError> {
        if file_size < 4 {
            return Ok(None);
        }
        // TODO: we don't really need to grab the whole next block, so maybe perf this later (and
        // clone)
        //
        // For now, we grab the next block since that might contain the rest of the metadta
        let mut iter = self.dir_blocks.iter();
        let mut block = iter.find(|a| a.0 == block_index).unwrap().1.clone();
        if let Some((_, block_1)) = iter.next() {
            block = [block, block_1.to_vec()].concat();
        }

        let mut bytes = &block[block_offset..][..file_size as usize - 3];
        let mut dirs = vec![];
        while !bytes.is_empty() {
            let (rest, dir) = Dir::from_bytes((bytes, 0))?;
            bytes = rest.0;
            dirs.push(dir);
        }

        trace!("dirs: {:02x?}", dirs);
        Ok(Some(dirs))
    }

    /// Extract all files(File/Symlink/Path) from image
    #[instrument(skip_all)]
    pub fn extract_all_files(&self) -> Result<Vec<Unsquashfs>, SquashfsError> {
        let mut ret = vec![];
        for inode in &self.inodes {
            trace!("inodes: {:02x?}", inode);
            if let Inode::BasicDirectory(basic_dir) = inode {
                if let Some(dirs) = self.dir_from_index(
                    basic_dir.block_index as u64,
                    basic_dir.file_size as u32,
                    basic_dir.block_offset as usize,
                )? {
                    // add dirs found
                    let mut paths = self.dir_paths(inode, &dirs)?;
                    ret.append(&mut paths);

                    // add files found
                    let mut files = self.extract_files_from_dir(dirs, inode)?;
                    ret.append(&mut files);
                }
            }
            if let Inode::ExtendedDirectory(ext_dir) = inode {
                if let Some(dirs) = self.dir_from_index(
                    ext_dir.block_index as u64,
                    ext_dir.file_size,
                    ext_dir.block_offset as usize,
                )? {
                    // add dirs found
                    let mut paths = self.dir_paths(inode, &dirs)?;
                    ret.append(&mut paths);

                    // add files found
                    let mut files = self.extract_files_from_dir(dirs, inode)?;
                    ret.append(&mut files);
                }
            }
        }

        Ok(ret)
    }

    /// From `inode` and it's `dirs`, extract all directories
    #[instrument(skip_all)]
    fn dir_paths(&self, inode: &Inode, dirs: &Vec<Dir>) -> Result<Vec<Unsquashfs>, SquashfsError> {
        let mut ret = vec![];
        for dir in dirs {
            for entry in &dir.dir_entries {
                // TODO use enum
                if entry.t == 1 {
                    let dir = self.path(inode, entry)?;
                    ret.push(Unsquashfs::Path(dir));
                }
            }
        }
        Ok(ret)
    }

    /// Extract all files from `inode` and `dirs`
    #[instrument(skip_all)]
    fn extract_files_from_dir(
        &self,
        dirs: Vec<Dir>,
        inode: &Inode,
    ) -> Result<Vec<Unsquashfs>, SquashfsError> {
        let mut ret = vec![];
        for dir in &dirs {
            for entry in &dir.dir_entries {
                // TODO: use type enum
                // file
                if entry.t == 2 {
                    let file = Unsquashfs::File(self.file(inode, dir.inode_num, entry)?);
                    ret.push(file);
                }
                // TODO: use type enum
                // soft link
                if entry.t == 3 {
                    let symlink = Unsquashfs::Symlink(self.symlink(inode, dir.inode_num, entry)?);
                    ret.push(symlink);
                }
            }
        }
        Ok(ret)
    }

    /// Given a file_name from a squashfs filepath, extract the file and the filepath from the
    /// internal squashfs stored in the fields
    ///
    /// TODO: this should be reworked into "extract_filepath"
    #[instrument(skip_all)]
    pub fn extract_file(&self, name: &str) -> Result<(PathBuf, Vec<u8>), SquashfsError> {
        // Search through inodes and parse directory table at the specified location
        // searching for first file name that matches
        for inode in &self.inodes {
            trace!("inodes: {:02x?}", inode);
            if let Inode::BasicDirectory(basic_dir) = inode {
                if let Some(dirs) = self.dir_from_index(
                    basic_dir.block_index as u64,
                    basic_dir.file_size as u32,
                    basic_dir.block_offset as usize,
                )? {
                    for dir in dirs {
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
            }
            if let Inode::ExtendedDirectory(ex_dir) = inode {
                if let Some(dirs) = self.dir_from_index(
                    ex_dir.block_index as u64,
                    ex_dir.file_size,
                    ex_dir.block_offset as usize,
                )? {
                    for dir in dirs {
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
            }
        }
        Err(SquashfsError::FileNotFound)
    }

    // From `basic_file`, extract total data including from data blocks and data fragments
    #[instrument(skip_all)]
    fn data(&self, basic_file: &BasicFile) -> Result<Vec<u8>, SquashfsError> {
        debug!("extracting: {:#02x?}", basic_file);

        // Add data
        info!("Extracting Data");
        let mut reader = Cursor::new(&self.data_and_fragments[basic_file.blocks_start as usize..]);
        let mut data_bytes = vec![];
        for block_size in &basic_file.block_sizes {
            let mut bytes = self.read_data(&mut reader, *block_size as usize)?;
            data_bytes.append(&mut bytes);
        }

        // Add fragments
        // TODO: this should be constant
        if basic_file.frag_index != 0xffffffff {
            if let Some(fragments) = &self.fragments {
                let frag = fragments[basic_file.frag_index as usize];
                trace!("Extracing frag: {:02x?}", frag);
                let mut reader = Cursor::new(&self.data_and_fragments[frag.start as usize..]);
                let mut bytes = self.read_data(&mut reader, frag.size as usize)?;
                data_bytes.append(&mut bytes);
            }
        }

        data_bytes = data_bytes[basic_file.block_offset as usize..]
            [..basic_file.file_size as usize]
            .to_vec();

        Ok(data_bytes)
    }

    /// Read from either Data blocks or Fragments blocks
    fn read_data<R: Read>(&self, reader: &mut R, size: usize) -> Result<Vec<u8>, SquashfsError> {
        let uncompressed = size & (1 << 24) != 0;
        let size = size & !(1 << 24);

        let mut buf = vec![0u8; size];
        reader.read_exact(&mut buf)?;

        let bytes = if uncompressed {
            buf
        } else {
            compressor::decompress(buf, self.superblock.compressor)?
        };
        Ok(bytes)
    }

    /// Symlink Details
    ///
    /// # Returns
    /// `Ok(filepath, original, link)
    #[instrument(skip_all)]
    fn symlink(
        &self,
        found_inode: &Inode,
        dir_inode: u32,
        entry: &DirEntry,
    ) -> Result<(PathBuf, String, String), SquashfsError> {
        let pathbuf = self.path(found_inode, entry)?;
        let link_inode = (dir_inode as i16 + entry.inode_offset) as u32;
        for inode in &self.inodes {
            if let Inode::BasicSymlink(basic_sym) = inode {
                if basic_sym.header.inode_number == link_inode {
                    return Ok((
                        pathbuf,
                        String::from_utf8(entry.name.clone())?,
                        String::from_utf8(basic_sym.target_path.clone())?,
                    ));
                }
            }
        }
        Err(SquashfsError::FileNotFound)
    }

    fn path(&self, found_inode: &Inode, found_entry: &DirEntry) -> Result<PathBuf, SquashfsError> {
        //  TODO: remove
        let entry_name = std::str::from_utf8(&found_entry.name)?;
        trace!("extracting: {entry_name}");
        let dir_inode = found_inode.expect_dir();
        // Used for searching for the file later when we want to extract the bytes

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
                            path_inodes.push(basic_dir.clone());
                            if basic_dir.header.inode_number == root_inode {
                                break 'outer;
                            }
                            next_inode = basic_dir.parent_inode;
                        }
                    }
                    if let Inode::ExtendedDirectory(ext_dir) = inode {
                        tracing::warn!("CONVERSION: {:02x?}", ext_dir);
                        for a in &ext_dir.dir_index {
                            let name = String::from_utf8(a.name.clone()).unwrap();
                            tracing::warn!("{name}");
                        }
                        if ext_dir.header.inode_number == next_inode {
                            path_inodes.push(BasicDirectory::from(ext_dir).clone());
                            if ext_dir.header.inode_number == root_inode {
                                break 'outer;
                            }
                            next_inode = ext_dir.parent_inode;
                        }
                    }
                }
            }
        }

        // Insert the first basic file
        path_inodes.insert(0, dir_inode);
        trace!("path: {:#02x?}", path_inodes);

        let mut paths = vec![];
        // Now we use n as the basic_dir with the inode, and look for that inode in the next
        // dir at the path directed from the path_inodes, when it matches, save it to the paths
        for n in 0..path_inodes.len() - 1 {
            let curr_basic_dir = &path_inodes[n];
            let search_inode = curr_basic_dir.header.inode_number;

            // Used for location
            let next_basic_dir = &path_inodes[n + 1];
            trace!("curr: {:02x?}", curr_basic_dir);
            trace!("next: {:02x?}", next_basic_dir);

            if let Some(dirs) = self.dir_from_index(
                next_basic_dir.block_index as u64,
                next_basic_dir.file_size as u32,
                next_basic_dir.block_offset as usize,
            )? {
                for dir in dirs {
                    trace!("dir: {dir:02x?}");
                    let base_inode = dir.inode_num;

                    for entry in &dir.dir_entries {
                        let entry_name = String::from_utf8(entry.name.clone())?;
                        trace!("entry: {entry_name}");
                        if base_inode as i16 + entry.inode_offset == search_inode as i16 {
                            paths.push(entry_name);
                            break;
                        }
                    }
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

        Ok(pathbuf)
    }

    /// From file details, extract (PathBuf, FileBytes)
    #[instrument(skip_all)]
    fn file(
        &self,
        found_inode: &Inode,
        found_inode_num: u32,
        found_entry: &DirEntry,
    ) -> Result<(PathBuf, Vec<u8>), SquashfsError> {
        let pathbuf = self.path(found_inode, found_entry)?;

        // look through basic file inodes in search of the one true basic_inode and extract the
        // bytes from the data and fragment sections
        let looking_inode = found_inode_num as i16 + found_entry.inode_offset;
        trace!("looking for inode: {:02x?}", looking_inode);
        for inode in &self.inodes {
            if let Inode::BasicFile(basic_file) = inode {
                if basic_file.header.inode_number == looking_inode as u32 {
                    return Ok((pathbuf, self.data(basic_file)?));
                }
            }
            if let Inode::ExtendedFile(ext_file) = inode {
                let basic_file = BasicFile::from(ext_file);
                if basic_file.header.inode_number == looking_inode as u32 {
                    return Ok((pathbuf, self.data(&basic_file)?));
                }
            }
        }

        Err(SquashfsError::FileNotFound)
    }
}
