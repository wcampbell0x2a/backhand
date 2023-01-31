use std::ffi::OsStr;
use std::fmt;
use std::io::{Seek, Write};
use std::os::unix::prelude::OsStrExt;

use tracing::{instrument, trace};

use crate::data::{Added, DataWriter};
use crate::dir::{Dir, DirEntry};
use crate::filesystem::{
    SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir, SquashfsFileWriter, SquashfsSymlink,
};
use crate::inode::{
    BasicDeviceSpecialFile, BasicDirectory, BasicFile, BasicSymlink, Inode, InodeHeader, InodeId,
    InodeInner,
};
use crate::metadata::MetadataWriter;

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
        std::str::from_utf8(&self.name).unwrap().to_string()
    }
    /// Write data and metadata for path node
    #[allow(clippy::too_many_arguments)]
    pub fn path(
        name: &'a OsStr,
        path: &SquashfsDir,
        inode: u32,
        parent_inode: u32,
        inode_writer: &mut MetadataWriter,
        file_size: u16,
        block_offset: u16,
        block_index: u32,
    ) -> Self {
        let dir_inode = Inode {
            id: InodeId::BasicDirectory,
            header: InodeHeader {
                inode_number: inode,
                ..path.header.into()
            },
            inner: InodeInner::BasicDirectory(BasicDirectory {
                block_index,
                link_count: 2,
                file_size,
                block_offset,
                parent_inode,
            }),
        };

        dir_inode.to_bytes(name.as_bytes(), inode_writer)
    }
    /// Write data and metadata for file node
    pub fn file<W: Write + Seek>(
        node_path: &'a OsStr,
        file: &SquashfsFileWriter<'_>,
        writer: &mut W,
        inode: u32,
        data_writer: &mut DataWriter,
        inode_writer: &mut MetadataWriter,
    ) -> Self {
        let (file_size, added) = data_writer.add_bytes(file.reader.borrow_mut().as_mut(), writer);

        let basic_file = match added {
            Added::Data {
                blocks_start,
                block_sizes,
            } => {
                BasicFile {
                    blocks_start,
                    frag_index: 0xffffffff, // <- no fragment
                    block_offset: 0x0,      // <- no fragment
                    file_size: file_size.try_into().unwrap(),
                    block_sizes,
                }
            },
            Added::Fragment {
                frag_index,
                block_offset,
            } => BasicFile {
                blocks_start: 0,
                frag_index,
                block_offset,
                file_size: file_size.try_into().unwrap(),
                block_sizes: vec![],
            },
        };

        let file_inode = Inode {
            id: InodeId::BasicFile,
            header: InodeHeader {
                inode_number: inode,
                ..file.header.into()
            },
            inner: InodeInner::BasicFile(basic_file),
        };

        file_inode.to_bytes(node_path.as_bytes(), inode_writer)
    }

    /// Write data and metadata for symlink node
    pub fn symlink(
        node_path: &'a OsStr,
        symlink: &SquashfsSymlink,
        inode: u32,
        inode_writer: &mut MetadataWriter,
    ) -> Self {
        let link = symlink.link.as_os_str().as_bytes();
        let sym_inode = Inode {
            id: InodeId::BasicSymlink,
            header: InodeHeader {
                inode_number: inode,
                ..symlink.header.into()
            },
            inner: InodeInner::BasicSymlink(BasicSymlink {
                link_count: 0x1,
                target_size: link.len() as u32,
                target_path: link.to_vec(),
            }),
        };

        sym_inode.to_bytes(node_path.as_bytes(), inode_writer)
    }

    /// Write data and metadata for char device node
    pub fn char(
        node_path: &'a OsStr,
        char_device: &SquashfsCharacterDevice,
        inode: u32,
        inode_writer: &mut MetadataWriter,
    ) -> Self {
        let char_inode = Inode {
            id: InodeId::BasicCharacterDevice,
            header: InodeHeader {
                inode_number: inode,
                ..char_device.header.into()
            },
            inner: InodeInner::BasicCharacterDevice(BasicDeviceSpecialFile {
                link_count: 0x1,
                device_number: char_device.device_number,
            }),
        };

        char_inode.to_bytes(node_path.as_bytes(), inode_writer)
    }

    /// Write data and metadata for block device node
    pub fn block_device(
        node_path: &'a OsStr,
        block_device: &SquashfsBlockDevice,
        inode: u32,
        inode_writer: &mut MetadataWriter,
    ) -> Self {
        let block_inode = Inode {
            id: InodeId::BasicBlockDevice,
            header: InodeHeader {
                inode_number: inode,
                ..block_device.header.into()
            },
            inner: InodeInner::BasicBlockDevice(BasicDeviceSpecialFile {
                link_count: 0x1,
                device_number: block_device.device_number,
            }),
        };

        block_inode.to_bytes(node_path.as_bytes(), inode_writer)
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

    /// Create alphabetically sorted entries
    #[instrument(skip_all)]
    pub(crate) fn into_dir(mut entries: Vec<Self>) -> Vec<Dir> {
        entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

        let mut dirs = vec![];
        let mut creating_dir = vec![];
        let mut creating_start = entries[0].start;
        let mut iter = entries.iter().peekable();

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

        trace!("DIIIIIIIIIIR: {:#02x?}", dirs);
        dirs
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::compressor::Compressor;
    use crate::metadata::{MetadataWriter, METADATA_MAXSIZE};

    #[test]
    fn test_mwriter() {
        let bytes = [0xffu8; METADATA_MAXSIZE - 3];

        let mut mwriter = MetadataWriter::new(Compressor::Xz, None, 0x2000);

        mwriter.write_all(&bytes).unwrap();
        assert_eq!(0, mwriter.metadata_start);
        assert_eq!(bytes, &*mwriter.uncompressed_bytes);
        assert!(mwriter.compressed_bytes.is_empty());

        let bytes = [0x11u8; 6];

        mwriter.write_all(&bytes).unwrap();
        assert_eq!(0x6e, mwriter.metadata_start);
        assert_eq!(bytes[3..], mwriter.uncompressed_bytes);
        assert_eq!(mwriter.compressed_bytes[0].len(), 0x6c);
    }

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
                offset: 0x200,
                inode: 6,
                t: InodeId::BasicDirectory,
                name_size: 0x01,
                name: b"zz",
            },
            Entry {
                start: 1,
                offset: 0x300,
                inode: 5,
                t: InodeId::BasicDirectory,
                name_size: 0x01,
                name: b"bb",
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
