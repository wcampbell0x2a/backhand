use deku::prelude::*;

use crate::compression::Compressor;
use crate::kind::Kind;
use crate::{DEFAULT_BLOCK_LOG, DEFAULT_BLOCK_SIZE};

pub(crate) const NOT_SET: u64 = 0xffff_ffff_ffff_ffff;

pub(crate) fn new(kind: &Kind, compressor: Compressor) -> Box<dyn SuperBlockTrait> {
    let superblock = match (kind.inner.version_major, kind.inner.version_minor) {
        (4, 0) => SuperBlock_V4_0 {
            magic: kind.inner.magic,
            inode_count: 0,
            mod_time: 0,
            block_size: DEFAULT_BLOCK_SIZE,
            frag_count: 0,
            compressor,
            block_log: DEFAULT_BLOCK_LOG,
            flags: 0,
            id_count: 0,
            version_major: kind.inner.version_major,
            version_minor: kind.inner.version_minor,
            root_inode: 0,
            bytes_used: 0,
            id_table: 0,
            xattr_table: NOT_SET,
            inode_table: 0,
            dir_table: 0,
            frag_table: NOT_SET,
            export_table: NOT_SET,
        },
        _ => todo!(),
    };

    Box::new(superblock)
}

pub trait SuperBlockTrait:
    for<'a> DekuRead<'a, ([u8; 4], u16, u16, deku::ctx::Endian)>
    + DekuWrite<([u8; 4], u16, u16, deku::ctx::Endian)>
    + std::fmt::Debug
{
    fn bytes_used(&self) -> u64;
    fn mut_bytes_used(&mut self) -> &mut u64;
    fn root_inode(&self) -> u64;
    fn mut_root_inode(&mut self) -> &mut u64;
    fn inode_count(&self) -> u32;
    fn mut_inode_count(&mut self) -> &mut u32;
    fn id_table(&self) -> u64;
    fn mut_id_table(&mut self) -> &mut u64;
    fn flags(&self) -> u16;
    fn mut_flags(&mut self) -> &mut u16;
    fn inode_table(&self) -> u64;
    fn mut_inode_table(&mut self) -> &mut u64;
    fn dir_table(&self) -> u64;
    fn mut_dir_table(&mut self) -> &mut u64;
    fn xattr_table(&self) -> u64;
    fn mut_xattr_table(&mut self) -> &mut u64;
    fn frag_table(&self) -> u64;
    fn mut_frag_table(&mut self) -> &mut u64;
    fn frag_count(&self) -> u32;
    fn mut_frag_count(&mut self) -> &mut u32;
    fn export_table(&self) -> u64;
    fn mut_export_table(&mut self) -> &mut u64;
    fn block_size(&self) -> u32;
    fn mut_block_size(&mut self) -> &mut u32;
    fn block_log(&self) -> u16;
    fn mut_block_log(&mut self) -> &mut u16;
    fn id_count(&self) -> u16;
    fn mut_id_count(&mut self) -> &mut u16;
    fn mod_time(&self) -> u32;
    fn mut_mod_time(&mut self) -> &mut u32;
    fn compressor(&self) -> Compressor;

    /* flags */
    fn inodes_uncompressed(&self) -> bool {
        self.flags() & SuperBlockFlags::InodesStoredUncompressed as u16 != 0
    }

    fn data_block_stored_uncompressed(&self) -> bool {
        self.flags() & SuperBlockFlags::DataBlockStoredUncompressed as u16 != 0
    }

    fn fragments_stored_uncompressed(&self) -> bool {
        self.flags() & SuperBlockFlags::FragmentsStoredUncompressed as u16 != 0
    }

    fn fragments_are_not_used(&self) -> bool {
        self.flags() & SuperBlockFlags::FragmentsAreNotUsed as u16 != 0
    }

    fn fragments_are_always_generated(&self) -> bool {
        self.flags() & SuperBlockFlags::FragmentsAreAlwaysGenerated as u16 != 0
    }

    fn data_has_been_duplicated(&self) -> bool {
        self.flags() & SuperBlockFlags::DataHasBeenDeduplicated as u16 != 0
    }

    fn nfs_export_table_exists(&self) -> bool {
        self.flags() & SuperBlockFlags::NFSExportTableExists as u16 != 0
    }

    fn xattrs_are_stored_uncompressed(&self) -> bool {
        self.flags() & SuperBlockFlags::XattrsAreStoredUncompressed as u16 != 0
    }

    fn no_xattrs_in_archive(&self) -> bool {
        self.flags() & SuperBlockFlags::NoXattrsInArchive as u16 != 0
    }

    fn compressor_options_are_present(&self) -> bool {
        self.flags() & SuperBlockFlags::CompressorOptionsArePresent as u16 != 0
    }
}

#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(
    endian = "ctx_type_endian",
    ctx = "ctx_magic: [u8; 4], ctx_version_major: u16, ctx_version_minor: u16, ctx_type_endian: deku::ctx::Endian"
)]
pub struct SuperBlock_V4_0 {
    /// Must be set to 0x73717368 ("hsqs" on disk).
    #[deku(assert_eq = "ctx_magic")]
    pub magic: [u8; 4],
    /// The number of inodes stored in the archive.
    pub inode_count: u32,
    /// Last modification time of the archive. Count seconds since 00:00, Jan 1st 1970 UTC (not counting leap seconds).
    /// This is unsigned, so it expires in the year 2106 (as opposed to 2038).
    pub mod_time: u32,
    /// The size of a data block in bytes. Must be a power of two between 4096 (4k) and 1048576 (1 MiB).
    pub block_size: u32,
    /// The number of entries in the fragment table.
    pub frag_count: u32,
    /// Compressor used for data
    pub compressor: Compressor,
    /// The log2 of the block size. If the two fields do not agree, the archive is considered corrupted.
    pub block_log: u16,
    /// Bit wise OR of the flag bits
    pub flags: u16,
    /// The number of entries in the ID lookup table.
    pub id_count: u16,
    #[deku(assert_eq = "ctx_version_major")]
    /// Major version of the format. Must be set to 4.
    pub version_major: u16,
    #[deku(assert_eq = "ctx_version_minor")]
    /// Minor version of the format. Must be set to 0.
    pub version_minor: u16,
    /// A reference to the inode of the root directory.
    pub root_inode: u64,
    /// The number of bytes used by the archive.
    /// Because SquashFS archives must be padded to a multiple of the underlying device block size, this can be less than the actual file size.
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

impl SuperBlockTrait for SuperBlock_V4_0 {
    fn bytes_used(&self) -> u64 {
        self.bytes_used
    }

    fn mut_bytes_used(&mut self) -> &mut u64 {
        &mut self.bytes_used
    }

    fn root_inode(&self) -> u64 {
        self.root_inode
    }

    fn mut_root_inode(&mut self) -> &mut u64 {
        &mut self.root_inode
    }

    fn inode_count(&self) -> u32 {
        self.inode_count
    }

    fn mut_inode_count(&mut self) -> &mut u32 {
        &mut self.inode_count
    }

    fn id_table(&self) -> u64 {
        self.id_table
    }

    fn mut_id_table(&mut self) -> &mut u64 {
        &mut self.id_table
    }

    fn flags(&self) -> u16 {
        self.flags
    }

    fn mut_flags(&mut self) -> &mut u16 {
        &mut self.flags
    }

    fn inode_table(&self) -> u64 {
        self.inode_table
    }

    fn mut_inode_table(&mut self) -> &mut u64 {
        &mut self.inode_table
    }

    fn dir_table(&self) -> u64 {
        self.dir_table
    }

    fn mut_dir_table(&mut self) -> &mut u64 {
        &mut self.dir_table
    }

    fn xattr_table(&self) -> u64 {
        self.xattr_table
    }

    fn mut_xattr_table(&mut self) -> &mut u64 {
        &mut self.xattr_table
    }

    fn frag_table(&self) -> u64 {
        self.frag_table
    }

    fn mut_frag_table(&mut self) -> &mut u64 {
        &mut self.frag_table
    }

    fn frag_count(&self) -> u32 {
        self.frag_count
    }

    fn mut_frag_count(&mut self) -> &mut u32 {
        &mut self.frag_count
    }

    fn export_table(&self) -> u64 {
        self.export_table
    }

    fn mut_export_table(&mut self) -> &mut u64 {
        &mut self.export_table
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn mut_block_size(&mut self) -> &mut u32 {
        &mut self.block_size
    }

    fn block_log(&self) -> u16 {
        self.block_log
    }

    fn mut_block_log(&mut self) -> &mut u16 {
        &mut self.block_log
    }

    fn mod_time(&self) -> u32 {
        self.mod_time
    }

    fn mut_mod_time(&mut self) -> &mut u32 {
        &mut self.mod_time
    }

    fn id_count(&self) -> u16 {
        self.id_count
    }

    fn mut_id_count(&mut self) -> &mut u16 {
        &mut self.id_count
    }

    fn compressor(&self) -> Compressor {
        self.compressor
    }
}

#[rustfmt::skip]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub(crate) enum SuperBlockFlags {
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

#[derive(Debug, Copy, Clone, DekuRead, DekuWrite, PartialEq, Eq)]
#[deku(
    endian = "ctx_type_endian",
    ctx = "ctx_magic: [u8; 4], ctx_version_major: u16, ctx_version_minor: u16, ctx_type_endian: deku::ctx::Endian"
)]
pub struct SuperBlock_V3_0 {
    #[deku(assert_eq = "ctx_magic")]
    pub magic: [u8; 4],
    pub inode_count: u32,
    pub bytes_used_2: u32,
    pub uid_start_2: u32,
    pub guid_start_2: u32,
    pub inode_table_start_2: u32,
    pub directory_table_start_2: u32,
    pub s_major: u16,
    pub s_minor: u16,
    pub block_size_1: u16,
    pub block_log: u16,
    pub flags: u8,
    pub no_uids: u8,
    pub no_guids: u8,
    pub mkfs_time: u8,
    pub root_inode: u64,
    pub block_size: u64,
    pub fragments: u64,
    pub fragment_table_start_2: u64,
    pub bytes_used: u64,
    pub uid_start: u64,
    pub guid_start: u64,
    pub inode_table_start: u64,
    pub directory_table_start: u64,
    pub fragment_table_start: u64,
    pub unused: u64,
}
