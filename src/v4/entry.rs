use std::ffi::OsStr;
use std::fmt;
use std::os::unix::prelude::OsStrExt;

use tracing::instrument;

use crate::kinds::Kind;
use crate::v4::data::Added;
use crate::v4::dir::{Dir, DirEntry};
use crate::v4::inode::{
    BasicDeviceSpecialFile, BasicDirectory, BasicFile, BasicSymlink, ExtendedDirectory, Inode,
    InodeHeader, InodeId, InodeInner,
};
use crate::v4::metadata::MetadataWriter;
use crate::v4::squashfs::SuperBlock;
use crate::v4::{Id, NodeHeader, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsSymlink};

#[derive(Clone)]
pub(crate) struct Entry<'a> {
    pub start: u32,
    pub offset: u16,
    pub inode: u32,
    pub t: InodeId,
    pub name_size: u16,
    pub name: &'a [u8],
}

impl<'a> Entry<'a> {
    pub fn name(&self) -> String {
        std::str::from_utf8(self.name).unwrap().to_string()
    }

    /// Write data and metadata for path node (Basic Directory or ExtendedDirectory)
    #[allow(clippy::too_many_arguments)]
    pub fn path(
        name: &'a OsStr,
        header: NodeHeader,
        inode: u32,
        children_num: usize,
        parent_inode: u32,
        inode_writer: &mut MetadataWriter,
        file_size: usize,
        block_offset: u16,
        block_index: u32,
        superblock: &SuperBlock,
        kind: &Kind,
        id_table: &[Id],
    ) -> Self {
        let uid = id_table.iter().position(|a| a.num == header.uid).unwrap() as u16;
        let gid = id_table.iter().position(|a| a.num == header.gid).unwrap() as u16;
        let header = InodeHeader {
            inode_number: inode,
            uid,
            gid,
            permissions: header.permissions,
            mtime: header.mtime,
        };
        // if entry won't fit in file_size of regular dir entry, create extended directory
        let dir_inode = if file_size > u16::MAX as usize {
            Inode::new(
                InodeId::ExtendedDirectory,
                header,
                InodeInner::ExtendedDirectory(ExtendedDirectory {
                    link_count: 2 + u32::try_from(children_num).unwrap(),
                    file_size: file_size.try_into().unwrap(), // u32
                    block_index,
                    parent_inode,
                    // TODO: Support Directory Index
                    index_count: 0,
                    block_offset,
                    // TODO(#32): Support xattr
                    xattr_index: 0xffff_ffff,
                    // TODO: Support Directory Index
                    dir_index: vec![],
                }),
            )
        } else {
            Inode::new(
                InodeId::BasicDirectory,
                header,
                InodeInner::BasicDirectory(BasicDirectory {
                    block_index,
                    link_count: 2 + u32::try_from(children_num).unwrap(),
                    file_size: file_size.try_into().unwrap(), // u16
                    block_offset,
                    parent_inode,
                }),
            )
        };

        dir_inode.to_bytes(name.as_bytes(), inode_writer, superblock, kind)
    }

    /// Write data and metadata for file node
    #[allow(clippy::too_many_arguments)]
    pub fn file(
        node_path: &'a OsStr,
        header: NodeHeader,
        inode: u32,
        inode_writer: &mut MetadataWriter,
        file_size: usize,
        added: &Added,
        superblock: &SuperBlock,
        kind: &Kind,
        id_table: &[Id],
    ) -> Self {
        let uid = id_table.iter().position(|a| a.num == header.uid).unwrap() as u16;
        let gid = id_table.iter().position(|a| a.num == header.gid).unwrap() as u16;
        let header = InodeHeader {
            inode_number: inode,
            uid,
            gid,
            permissions: header.permissions,
            mtime: header.mtime,
        };
        let basic_file = match added {
            Added::Data {
                blocks_start,
                block_sizes,
            } => {
                BasicFile {
                    blocks_start: *blocks_start,
                    frag_index: 0xffffffff, // <- no fragment
                    block_offset: 0x0,      // <- no fragment
                    file_size: file_size.try_into().unwrap(),
                    block_sizes: block_sizes.to_vec(),
                }
            }
            Added::Fragment {
                frag_index,
                block_offset,
            } => BasicFile {
                blocks_start: 0,
                frag_index: *frag_index,
                block_offset: *block_offset,
                file_size: file_size.try_into().unwrap(),
                block_sizes: vec![],
            },
        };

        let file_inode = Inode::new(
            InodeId::BasicFile,
            header,
            InodeInner::BasicFile(basic_file),
        );

        file_inode.to_bytes(node_path.as_bytes(), inode_writer, superblock, kind)
    }

    /// Write data and metadata for symlink node
    #[allow(clippy::too_many_arguments)]
    pub fn symlink(
        node_path: &'a OsStr,
        header: NodeHeader,
        symlink: &SquashfsSymlink,
        inode: u32,
        inode_writer: &mut MetadataWriter,
        superblock: &SuperBlock,
        kind: &Kind,
        id_table: &[Id],
    ) -> Self {
        let uid = id_table.iter().position(|a| a.num == header.uid).unwrap() as u16;
        let gid = id_table.iter().position(|a| a.num == header.gid).unwrap() as u16;
        let header = InodeHeader {
            inode_number: inode,
            uid,
            gid,
            permissions: header.permissions,
            mtime: header.mtime,
        };
        let link = symlink.link.as_os_str().as_bytes();
        let sym_inode = Inode::new(
            InodeId::BasicSymlink,
            header,
            InodeInner::BasicSymlink(BasicSymlink {
                link_count: 0x1,
                target_size: link.len().try_into().unwrap(),
                target_path: link.to_vec(),
            }),
        );

        sym_inode.to_bytes(node_path.as_bytes(), inode_writer, superblock, kind)
    }

    /// Write data and metadata for char device node
    #[allow(clippy::too_many_arguments)]
    pub fn char(
        node_path: &'a OsStr,
        header: NodeHeader,
        char_device: &SquashfsCharacterDevice,
        inode: u32,
        inode_writer: &mut MetadataWriter,
        superblock: &SuperBlock,
        kind: &Kind,
        id_table: &[Id],
    ) -> Self {
        let uid = id_table.iter().position(|a| a.num == header.uid).unwrap() as u16;
        let gid = id_table.iter().position(|a| a.num == header.gid).unwrap() as u16;
        let header = InodeHeader {
            inode_number: inode,
            uid,
            gid,
            permissions: header.permissions,
            mtime: header.mtime,
        };
        let char_inode = Inode::new(
            InodeId::BasicCharacterDevice,
            header,
            InodeInner::BasicCharacterDevice(BasicDeviceSpecialFile {
                link_count: 0x1,
                device_number: char_device.device_number,
            }),
        );

        char_inode.to_bytes(node_path.as_bytes(), inode_writer, superblock, kind)
    }

    /// Write data and metadata for block device node
    #[allow(clippy::too_many_arguments)]
    pub fn block_device(
        node_path: &'a OsStr,
        header: NodeHeader,
        block_device: &SquashfsBlockDevice,
        inode: u32,
        inode_writer: &mut MetadataWriter,
        superblock: &SuperBlock,
        kind: &Kind,
        id_table: &[Id],
    ) -> Self {
        let uid = id_table.iter().position(|a| a.num == header.uid).unwrap() as u16;
        let gid = id_table.iter().position(|a| a.num == header.gid).unwrap() as u16;
        let header = InodeHeader {
            inode_number: inode,
            uid,
            gid,
            permissions: header.permissions,
            mtime: header.mtime,
        };
        let block_inode = Inode::new(
            InodeId::BasicBlockDevice,
            header,
            InodeInner::BasicBlockDevice(BasicDeviceSpecialFile {
                link_count: 0x1,
                device_number: block_device.device_number,
            }),
        );

        block_inode.to_bytes(node_path.as_bytes(), inode_writer, superblock, kind)
    }
}

impl<'a> fmt::Debug for Entry<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("start", &self.start)
            .field("offset", &self.offset)
            .field("inode", &self.inode)
            .field("t", &self.t)
            .field("name_size", &self.name_size)
            .field("name", &self.name())
            .finish()
    }
}

impl<'a> Entry<'a> {
    fn create_dir(creating_dir: &Vec<&Self>, start: u32, lowest_inode: u32) -> Dir {
        let mut dir = Dir::new(lowest_inode);

        dir.count = creating_dir.len().try_into().unwrap();
        if dir.count >= 256 {
            panic!("dir.count({}) >= 256:", dir.count);
        }

        dir.start = start;
        for e in creating_dir {
            let inode = e.inode;
            let new_entry = DirEntry {
                offset: e.offset,
                inode_offset: (inode - lowest_inode).try_into().unwrap(),
                t: e.t.into_base_type(),
                name_size: e.name_size,
                name: e.name.to_vec(),
            };
            dir.push(new_entry);
        }

        dir
    }

    /// Create entries, input need to be alphabetically sorted
    #[instrument(skip_all)]
    pub(crate) fn into_dir(entries: Vec<Self>) -> Vec<Dir> {
        let mut dirs = vec![];
        let mut creating_dir = vec![];
        let mut lowest_inode = u32::MAX;
        let mut iter = entries.iter().peekable();
        let mut creating_start = if let Some(entry) = iter.peek() {
            entry.start
        } else {
            return vec![];
        };

        while let Some(e) = iter.next() {
            if e.inode < lowest_inode {
                lowest_inode = e.inode;
            }
            creating_dir.push(e);

            // last entry
            if let Some(next) = &iter.peek() {
                // if the next entry would be > the lowest_inode
                let max_inode = (next.inode as u64).abs_diff(lowest_inode as u64) > i16::MAX as u64;
                // make sure entires have the correct start and amount of directories
                if next.start != creating_start || creating_dir.len() >= 255 || max_inode {
                    let dir = Self::create_dir(&creating_dir, creating_start, lowest_inode);
                    dirs.push(dir);
                    creating_dir = vec![];
                    creating_start = next.start;
                    lowest_inode = u32::MAX;
                }
            }
            // last entry
            if iter.peek().is_none() {
                let dir = Self::create_dir(&creating_dir, creating_start, lowest_inode);
                dirs.push(dir);
            }
        }

        dirs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry() {
        let entries = vec![
            Entry {
                start: 0,
                offset: 0x100,
                inode: 1,
                t: InodeId::BasicDirectory,
                name_size: 0x01,
                name: b"aa",
            },
            Entry {
                start: 1,
                offset: 0x300,
                inode: 5,
                t: InodeId::BasicDirectory,
                name_size: 0x01,
                name: b"bb",
            },
            Entry {
                start: 1,
                offset: 0x200,
                inode: 6,
                t: InodeId::BasicDirectory,
                name_size: 0x01,
                name: b"zz",
            },
        ];

        let dir = Entry::into_dir(entries);
        assert_eq!(
            vec![
                Dir {
                    count: 0x0,
                    start: 0x0,
                    inode_num: 0x1,
                    dir_entries: vec![DirEntry {
                        offset: 0x100,
                        inode_offset: 0x0,
                        t: InodeId::BasicDirectory,
                        name_size: 0x1,
                        name: b"aa".to_vec(),
                    },],
                },
                Dir {
                    count: 0x1,
                    start: 0x1,
                    inode_num: 0x5,
                    dir_entries: vec![
                        DirEntry {
                            offset: 0x300,
                            inode_offset: 0x0,
                            t: InodeId::BasicDirectory,
                            name_size: 0x1,
                            name: b"bb".to_vec(),
                        },
                        DirEntry {
                            offset: 0x200,
                            inode_offset: 0x1,
                            t: InodeId::BasicDirectory,
                            name_size: 0x1,
                            name: b"zz".to_vec(),
                        },
                    ],
                },
            ],
            dir
        );
    }
}
