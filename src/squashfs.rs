//! [`Squashfs`], [`Id`], and [`Export`]

use std::fmt;
use std::io::{Cursor, Read, SeekFrom};
use std::path::PathBuf;

use deku::bitvec::BitVec;
use deku::prelude::*;
use tracing::{debug, info, instrument, trace};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::dir::{Dir, DirEntry};
use crate::error::SquashfsError;
use crate::fragment::Fragment;
use crate::inode::{BasicDirectory, BasicFile, Inode, InodeHeader, InodeInner};
use crate::metadata;
use crate::reader::{ReadSeek, SquashfsReader};

/// NFS export support
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Export(pub u64);

/// 32 bit user and group IDs
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct Id(pub u32);

/// Contains important information about the archive, including the locations of other sections
#[derive(Debug, Copy, Clone, DekuRead, DekuWrite)]
#[deku(endian = "little")]
pub struct SuperBlock {
    #[deku(assert_eq = "Self::MAGIC")]
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
    const MAGIC: u32 = 0x73717368;
    const BLOCK_SIZE: u32 = 0x20000;
    const BLOCK_LOG: u16 = 0x11;
    const VERSION_MAJ: u16 = 4;
    const VERSION_MIN: u16 = 0;
    const NOT_SET: u64 = 0xffffffffffffffff;

    pub fn new(compressor: Compressor) -> Self {
        Self {
            magic: Self::MAGIC,
            inode_count: 0,
            mod_time: 0,
            block_size: Self::BLOCK_SIZE, // use const
            frag_count: 0,
            compressor,
            block_log: Self::BLOCK_LOG,
            flags: 0,
            id_count: 0,
            version_major: Self::VERSION_MAJ,
            version_minor: Self::VERSION_MIN,
            root_inode: 0,
            bytes_used: 0,
            id_table: 0,
            xattr_table: Self::NOT_SET,
            inode_table: 0,
            dir_table: 0,
            frag_table: Self::NOT_SET,
            export_table: Self::NOT_SET,
        }
    }

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

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct FilesystemHeader {
    pub permissions: u16,
    pub uid: u16,
    pub gid: u16,
    pub mtime: u32,
}

impl From<InodeHeader> for FilesystemHeader {
    fn from(inode_header: InodeHeader) -> Self {
        Self {
            permissions: inode_header.permissions,
            uid: inode_header.uid,
            gid: inode_header.gid,
            mtime: inode_header.mtime,
        }
    }
}

/// In-memory representation of a Squashfs Image
#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct Filesystem {
    pub nodes: Vec<Node>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Node {
    File(SquashfsFile),
    Symlink(SquashfsSymlink),
    Path(SquashfsPath),
}

// TODO: We need our own header, without inode_number
#[derive(PartialEq, Eq, Clone)]
pub struct SquashfsFile {
    pub header: FilesystemHeader,
    pub path: PathBuf,
    // TODO: Maybe hold a reference to a Reader? so that something could be written to disk and read from
    // disk instead of loaded into memory
    pub bytes: Vec<u8>,
}

impl fmt::Debug for SquashfsFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirEntry")
            .field("header", &self.header)
            .field("path", &self.path)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

// TODO: We need our own header, without inode_number
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsSymlink {
    pub header: FilesystemHeader,
    pub path: PathBuf,
    pub original: String,
    pub link: String,
}

// TODO: We need our own header, without inode_number
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SquashfsPath {
    pub header: FilesystemHeader,
    pub path: PathBuf,
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
    pub root_inode: Inode,
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
        info!("{superblock:#08x?}");

        // Parse Compression Options, if any
        info!("Reading Compression options");
        let compression_options = if superblock.compressor != Compressor::None {
            if let Some(size) = superblock.compression_options_size() {
                // ingore the size above, that is just a warning that some other information is in
                // the Option that isn't in the squashfs spec.
                //
                // This appears in TP-Link rootfs squashfs.
                let bytes = metadata::read_block(&mut reader, &superblock)?;

                // TODO: test if this is correct
                if bytes.len() != size {
                    tracing::warn!(
                        "Non standard compression options! CompressionOptions might be incorrect: {:02x?}",
                        bytes
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
        trace!("compression_options: {compression_options:08x?}");

        // Create SquashfsReader
        let mut squashfs_reader = SquashfsReader::new(reader, offset);

        // Read all fields from filesystem to make a Squashfs
        info!("Reading Data and Fragments");
        let data_and_fragments = squashfs_reader.data_and_fragments(&superblock)?;

        info!("Reading Inodes");
        let inodes = squashfs_reader.inodes(&superblock)?;

        info!("Reading Root Inode");
        let root_inode = squashfs_reader.root_inode(&superblock)?;

        info!("Reading Fragments");
        let fragments = squashfs_reader.fragments(&superblock).unwrap();
        let fragment_ptr = fragments.clone().map(|a| a.0);
        let fragment_table = fragments.map(|a| a.1);

        info!("Reading Exports");
        let export = squashfs_reader.export(&superblock)?;
        let export_ptr = export.clone().map(|a| a.0);
        let export_table = export.map(|a| a.1);

        info!("Reading Ids");
        let id = squashfs_reader.id(&superblock)?;
        let id_ptr = id.clone().map(|a| a.0);
        let id_table = id.map(|a| a.1);

        let last_dir_position = if let Some(fragment_ptr) = fragment_ptr {
            trace!("using fragment for end of dir");
            fragment_ptr
        } else if let Some(export_ptr) = export_ptr {
            trace!("using export for end of dir");
            export_ptr
        } else if let Some(id_ptr) = id_ptr {
            trace!("using id for end of dir");
            id_ptr
        } else {
            unreachable!();
        };

        info!("Reading Dirs");
        let dir_blocks = squashfs_reader.dir_blocks(&superblock, last_dir_position)?;

        let squashfs = Squashfs {
            superblock,
            compression_options,
            data_and_fragments,
            inodes,
            root_inode,
            dir_blocks,
            fragments: fragment_table,
            export: export_table,
            id: id_table,
        };

        info!("Successful Read");
        Ok(squashfs)
    }

    pub fn all_dirs(&self) -> Result<Vec<Dir>, SquashfsError> {
        let bytes: Vec<u8> = self
            .dir_blocks
            .iter()
            .flat_map(|(_, b)| b.clone())
            .collect();

        let mut dirs = vec![];
        let mut rest = bytes;
        while !rest.is_empty() {
            let ((r, _), dir) = Dir::from_bytes((&rest, 0)).unwrap();
            rest = r.to_vec();
            dirs.push(dir);
        }

        Ok(dirs)
    }

    ///
    ///
    ///
    /// # Returns
    /// - `Ok(Some(Vec<Dir>))` when found dir
    /// - `Ok(None)`           when empty dir
    #[instrument(skip_all)]
    pub(crate) fn dir_from_index(
        &self,
        block_index: u64,
        file_size: u32,
        block_offset: usize,
    ) -> Result<Option<Vec<Dir>>, SquashfsError> {
        trace!("- block index : {:02x?}", block_index);
        trace!("- file_size   : {:02x?}", file_size);
        trace!("- block offset: {:02x?}", block_offset);
        if file_size < 4 {
            return Ok(None);
        }

        // ignore blocks before our block_index, grab all the rest of the bytes
        // TODO: perf
        let block: Vec<u8> = self
            .dir_blocks
            .iter()
            .filter(|(a, _)| a >= &block_index)
            .map(|(_, b)| b.clone())
            .collect::<Vec<Vec<u8>>>()
            .iter()
            .flatten()
            .copied()
            .collect();

        // Parse into Dirs
        let mut bytes = &block[block_offset..][..file_size as usize - 3];
        let mut dirs = vec![];
        while !bytes.is_empty() {
            let (rest, dir) = Dir::from_bytes((bytes, 0))?;
            bytes = rest.0;
            dirs.push(dir);
        }

        Ok(Some(dirs))
    }

    /// Extract all files(File/Symlink/Path) from image
    #[instrument(skip_all)]
    pub fn into_filesystem(&self) -> Result<Filesystem, SquashfsError> {
        let mut nodes = vec![];
        for inode in &self.inodes {
            trace!("inodes: {:02x?}", inode);
            match &inode.inner {
                InodeInner::BasicDirectory(basic_dir) => {
                    trace!("BASIC_DIR inodes: {:02x?}", basic_dir);
                    if let Some(dirs) = self.dir_from_index(
                        basic_dir.block_index as u64,
                        basic_dir.file_size as u32,
                        basic_dir.block_offset as usize,
                    )? {
                        for d in &dirs {
                            for e in &d.dir_entries {
                                trace!("name: {:02x?}", e.name());
                            }
                        }
                        // add dirs found
                        let mut paths = self.dir_paths(inode, &dirs)?;
                        nodes.append(&mut paths);

                        // add files found
                        let mut files = self.extract_files_from_dir(dirs, inode)?;
                        nodes.append(&mut files);
                    }
                },
                InodeInner::ExtendedDirectory(ext_dir) => {
                    trace!("EXT_DIR: {:#02x?}", ext_dir);
                    if let Some(dirs) = self.dir_from_index(
                        ext_dir.block_index as u64,
                        ext_dir.file_size,
                        ext_dir.block_offset as usize,
                    )? {
                        for d in &dirs {
                            for e in &d.dir_entries {
                                trace!("name: {:02x?}", e.name());
                            }
                        }
                        // add dirs found
                        let mut paths = self.dir_paths(inode, &dirs)?;
                        nodes.append(&mut paths);

                        // add files found
                        let mut files = self.extract_files_from_dir(dirs, inode)?;
                        nodes.append(&mut files);
                    }
                },
                _ => (),
            }
        }

        let filesystem = Filesystem { nodes };
        Ok(filesystem)
    }

    /// From `inode` and it's `dirs`, extract all directories
    #[instrument(skip_all)]
    fn dir_paths(&self, inode: &Inode, dirs: &Vec<Dir>) -> Result<Vec<Node>, SquashfsError> {
        let mut ret = vec![];
        for dir in dirs {
            for entry in &dir.dir_entries {
                // TODO use enum
                if entry.t == 1 {
                    let path = self.path(inode, entry)?;
                    ret.push(Node::Path(SquashfsPath {
                        header: inode.header.into(),
                        path,
                    }));
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
    ) -> Result<Vec<Node>, SquashfsError> {
        let mut ret = vec![];
        for dir in &dirs {
            for entry in &dir.dir_entries {
                // TODO: use type enum
                // file
                if entry.t == 2 {
                    // TODO: self.symlink should give the inode.header
                    trace!("before_file: {:#02x?}", entry);
                    let (path, bytes) = self.file(inode, dir.inode_num, entry)?;
                    let file = Node::File(SquashfsFile {
                        header: inode.header.into(),
                        path,
                        bytes,
                    });
                    ret.push(file);
                }
                // TODO: use type enum
                // soft link
                if entry.t == 3 {
                    // TODO: self.symlink should give the inode.header
                    let (path, original, link) = self.symlink(inode, dir.inode_num, entry)?;
                    let symlink = Node::Symlink(SquashfsSymlink {
                        header: inode.header.into(),
                        path,
                        original,
                        link,
                    });
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
        trace!("file: {name}");
        // Search through inodes and parse directory table at the specified location
        // searching for first file name that matches
        for inode in &self.inodes {
            //trace!("inodes: {:02x?}", inode);
            match &inode.inner {
                InodeInner::BasicDirectory(basic_dir) => {
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
                },
                InodeInner::ExtendedDirectory(ex_dir) => {
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
                },
                _ => (),
            }
        }
        Err(SquashfsError::FileNotFound)
    }

    // From `basic_file`, extract total data including from data blocks and data fragments
    #[instrument(skip_all)]
    fn data(&self, basic_file: &BasicFile) -> Result<Vec<u8>, SquashfsError> {
        debug!("extracting: {:#02x?}", basic_file);

        // Add data
        trace!("extracting data @ offset {:02x?}", basic_file.blocks_start);
        let mut reader = Cursor::new(&self.data_and_fragments[basic_file.blocks_start as usize..]);
        let mut data_bytes = vec![];
        for block_size in &basic_file.block_sizes {
            let mut bytes = self.read_data(&mut reader, *block_size as usize)?;
            data_bytes.append(&mut bytes);
        }

        trace!("data bytes: {:02x?}", data_bytes.len());

        // Add fragments
        // TODO: this should be constant
        if basic_file.frag_index != 0xffffffff {
            if let Some(fragments) = &self.fragments {
                let frag = fragments[basic_file.frag_index as usize];
                trace!("Extracting frag: {:02x?}", frag);
                let mut reader = Cursor::new(&self.data_and_fragments[frag.start as usize..]);
                let mut bytes = self.read_data(&mut reader, frag.size as usize)?;
                trace!("uncompressed size: {:02x?}", bytes.len());
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
            if let InodeInner::BasicSymlink(basic_sym) = &inode.inner {
                if inode.header.inode_number == link_inode {
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

    #[instrument(skip_all)]
    fn path(&self, found_inode: &Inode, found_entry: &DirEntry) -> Result<PathBuf, SquashfsError> {
        //  TODO: remove
        let entry_name = std::str::from_utf8(&found_entry.name)?;
        trace!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!extracting: {entry_name}");
        trace!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!extracting: found_inode: {found_inode:#02x?}\nfound_entry: {found_entry:#02x?}");
        let dir_inode = found_inode.expect_dir();
        let dir_inode_num = found_inode.header.inode_number;
        // Used for searching for the file later when we want to extract the bytes

        let root_inode = self.root_inode.header.inode_number;
        debug!(
            "!!! searching for dir path to root inode: {:#02x?}",
            root_inode
        );

        let mut path_inodes = vec![];
        // first check if the dir inode of this file is the root inode
        trace!("inode: {dir_inode_num} ?? root: {root_inode}");
        if dir_inode_num != root_inode {
            trace!("{dir_inode_num} != root inode");
            // check every inode and find the matching inode to the current dir_nodes parent
            // inode, when we find a new inode, check if it's the root inode, and continue on if it
            // isn't
            let mut next_inode = dir_inode.parent_inode;
            trace!("parent: {next_inode:02x?}");
            'outer: loop {
                for inode in &self.inodes {
                    match &inode.inner {
                        InodeInner::BasicDirectory(basic_dir) => {
                            if inode.header.inode_number == next_inode {
                                path_inodes.push((basic_dir.clone(), inode.header.inode_number));
                                if inode.header.inode_number == root_inode {
                                    break 'outer;
                                }
                                next_inode = basic_dir.parent_inode;
                            }
                        },
                        InodeInner::ExtendedDirectory(ext_dir) => {
                            tracing::trace!("CONVERSION: {:02x?}", ext_dir);
                            if inode.header.inode_number == next_inode {
                                path_inodes.push((
                                    BasicDirectory::from(ext_dir).clone(),
                                    inode.header.inode_number,
                                ));
                                if inode.header.inode_number == root_inode {
                                    break 'outer;
                                }
                                next_inode = ext_dir.parent_inode;
                            }
                        },
                        _ => (),
                    }
                }
            }
        }

        // Insert the first basic file
        path_inodes.insert(0, (dir_inode, dir_inode_num));
        trace!("path: {:#02x?}", path_inodes);

        let mut paths = vec![];
        // Now we use n as the basic_dir with the inode, and look for that inode in the next
        // dir at the path directed from the path_inodes, when it matches, save it to the paths
        for n in 0..path_inodes.len() - 1 {
            let curr_basic_dir = &path_inodes[n];
            let search_inode = curr_basic_dir.1;

            // Used for location
            let next_basic_dir = &path_inodes[n + 1];
            trace!("curr: {:02x?}", curr_basic_dir);
            trace!("next: {:02x?}", next_basic_dir);

            if let Some(dirs) = self.dir_from_index(
                next_basic_dir.0.block_index as u64,
                next_basic_dir.0.file_size as u32,
                next_basic_dir.0.block_offset as usize,
            )? {
                for dir in dirs {
                    trace!("dir: {dir:02x?}");
                    let base_inode = dir.inode_num;

                    for entry in &dir.dir_entries {
                        let entry_name = String::from_utf8(entry.name.clone())?;
                        trace!(
                            "entry: {entry_name}, {:02x} ?== {:02x}",
                            base_inode as i16 + entry.inode_offset,
                            search_inode as i16
                        );
                        if base_inode as i16 + entry.inode_offset == search_inode as i16 {
                            trace!("match");
                            paths.push(entry_name);
                            break;
                        }
                    }
                }
            } else {
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
        trace!("{:?}", pathbuf.display());

        // look through basic file inodes in search of the one true basic_inode and extract the
        // bytes from the data and fragment sections
        let looking_inode = found_inode_num as i16 + found_entry.inode_offset;
        trace!("looking for inode: {:02x?}", looking_inode);
        for inode in &self.inodes {
            if inode.header.inode_number == looking_inode as u32 {
                match &inode.inner {
                    InodeInner::BasicFile(basic_file) => {
                        return Ok((pathbuf, self.data(basic_file)?));
                    },
                    InodeInner::ExtendedFile(ext_file) => {
                        let basic_file = BasicFile::from(ext_file);
                        return Ok((pathbuf, self.data(&basic_file)?));
                    },
                    _ => panic!(),
                }
            }
        }

        Err(SquashfsError::FileNotFound)
    }
}
