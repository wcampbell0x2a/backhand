use std::ffi::OsStr;
use std::fmt;
use std::os::unix::prelude::OsStrExt;

use tracing::{instrument, trace};

use crate::data::Added;
use crate::dir::{Dir, DirEntry};
use crate::inode::{
    BasicDeviceSpecialFile, BasicDirectory, BasicFile, BasicSymlink, Inode, InodeHeader, InodeId,
    InodeInner,
};
use crate::kinds::Kind;
use crate::metadata::MetadataWriter;
use crate::squashfs::SuperBlock;
use crate::{NodeHeader, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsSymlink};

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

    /// Write data and metadata for path node
    #[allow(clippy::too_many_arguments)]
    pub fn path(
        name: &'a OsStr,
        header: NodeHeader,
        inode: u32,
        parent_inode: u32,
        inode_writer: &mut MetadataWriter,
        file_size: u16,
        block_offset: u16,
        block_index: u32,
        superblock: &SuperBlock,
        kind: Kind,
    ) -> Self {
        let dir_inode = Inode {
            id: InodeId::BasicDirectory,
            header: InodeHeader {
                inode_number: inode,
                ..header.into()
            },
            inner: InodeInner::BasicDirectory(BasicDirectory {
                block_index,
                link_count: 2,
                file_size,
                block_offset,
                parent_inode,
            }),
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
        kind: Kind,
    ) -> Self {
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
            },
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

        let file_inode = Inode {
            id: InodeId::BasicFile,
            header: InodeHeader {
                inode_number: inode,
                ..header.into()
            },
            inner: InodeInner::BasicFile(basic_file),
        };

        file_inode.to_bytes(node_path.as_bytes(), inode_writer, superblock, kind)
    }

    /// Write data and metadata for symlink node
    pub fn symlink(
        node_path: &'a OsStr,
        header: NodeHeader,
        symlink: &SquashfsSymlink,
        inode: u32,
        inode_writer: &mut MetadataWriter,
        superblock: &SuperBlock,
        kind: Kind,
    ) -> Self {
        let link = symlink.link.as_os_str().as_bytes();
        let sym_inode = Inode {
            id: InodeId::BasicSymlink,
            header: InodeHeader {
                inode_number: inode,
                ..header.into()
            },
            inner: InodeInner::BasicSymlink(BasicSymlink {
                link_count: 0x1,
                target_size: link.len() as u32,
                target_path: link.to_vec(),
            }),
        };

        sym_inode.to_bytes(node_path.as_bytes(), inode_writer, superblock, kind)
    }

    /// Write data and metadata for char device node
    pub fn char(
        node_path: &'a OsStr,
        header: NodeHeader,
        char_device: &SquashfsCharacterDevice,
        inode: u32,
        inode_writer: &mut MetadataWriter,
        superblock: &SuperBlock,
        kind: Kind,
    ) -> Self {
        let char_inode = Inode {
            id: InodeId::BasicCharacterDevice,
            header: InodeHeader {
                inode_number: inode,
                ..header.into()
            },
            inner: InodeInner::BasicCharacterDevice(BasicDeviceSpecialFile {
                link_count: 0x1,
                device_number: char_device.device_number,
            }),
        };

        char_inode.to_bytes(node_path.as_bytes(), inode_writer, superblock, kind)
    }

    /// Write data and metadata for block device node
    pub fn block_device(
        node_path: &'a OsStr,
        header: NodeHeader,
        block_device: &SquashfsBlockDevice,
        inode: u32,
        inode_writer: &mut MetadataWriter,
        superblock: &SuperBlock,
        kind: Kind,
    ) -> Self {
        let block_inode = Inode {
            id: InodeId::BasicBlockDevice,
            header: InodeHeader {
                inode_number: inode,
                ..header.into()
            },
            inner: InodeInner::BasicBlockDevice(BasicDeviceSpecialFile {
                link_count: 0x1,
                device_number: block_device.device_number,
            }),
        };

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
    fn create_dir(creating_dir: &Vec<&Self>, start: u32) -> Dir {
        // find lowest inode number
        let mut lowest_inode = None;
        for e in creating_dir {
            if lowest_inode.is_none() {
                lowest_inode = Some(e.inode);
            }
            if let Some(low) = lowest_inode {
                if e.inode < low {
                    lowest_inode = Some(e.inode);
                }
            }
        }

        let mut dir = Dir::new(lowest_inode.unwrap());
        dir.count = creating_dir.len() as u32;
        if dir.count >= 256 {
            panic!("dir.count({}) >= 256:", dir.count);
        }
        dir.start = start;
        for e in creating_dir {
            let new_entry = DirEntry {
                offset: e.offset,
                inode_offset: (e.inode - lowest_inode.unwrap()) as i16,
                t: e.t,
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
        let mut iter = entries.iter().peekable();
        let mut creating_start = if let Some(entry) = iter.peek() {
            entry.start
        } else {
            return vec![];
        };

        while let Some(e) = iter.next() {
            creating_dir.push(e);

            // last entry
            if let Some(next) = &iter.peek() {
                // make sure entires have the correct start and amount of directories
                if next.start != creating_start || creating_dir.len() >= 255 {
                    let dir = Self::create_dir(&creating_dir, creating_start);
                    dirs.push(dir);
                    creating_dir = vec![];
                    creating_start = next.start;
                }
            }
            // last entry
            if iter.peek().is_none() {
                let dir = Self::create_dir(&creating_dir, creating_start);
                dirs.push(dir);
            }
        }

        trace!("DIIIIIIIIIIR: {:#02x?}", &dirs);
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
