use std::fmt;

use tracing::{instrument, trace};

use crate::dir::{Dir, DirEntry};
use crate::inode::InodeId;

#[derive(Clone)]
pub(crate) struct Entry {
    pub start: u32,
    pub offset: u16,
    pub inode: u32,
    pub t: InodeId,
    pub name_size: u16,
    pub name: Vec<u8>,
}

impl Entry {
    pub fn name(&self) -> String {
        std::str::from_utf8(&self.name).unwrap().to_string()
    }
}

impl fmt::Debug for Entry {
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

impl Entry {
    fn create_dir(creating_dir: &Vec<&Entry>, start: u32) -> Dir {
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
                name: e.name.clone(),
            };
            dir.push(new_entry);
        }

        dir
    }

    /// Create alphabetically sorted entries
    #[instrument(skip_all)]
    pub(crate) fn into_dir(entries: &mut [Entry]) -> Vec<Dir> {
        entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

        let mut dirs = vec![];
        let mut creating_dir = vec![];
        let mut creating_start = entries[0].start;
        let mut iter = entries.iter().peekable();

        while let Some(e) = iter.next() {
            creating_dir.push(e);

            // last entry
            if let Some(next) = &iter.peek() {
                if next.start != creating_start {
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

        let mut mwriter = MetadataWriter::new(Compressor::Xz, None);

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
        let mut entries = vec![
            Entry {
                start: 0,
                offset: 0x100,
                inode: 1,
                t: InodeId::BasicDirectory,
                name_size: 0x01,
                name: [b'a', b'a'].to_vec(),
            },
            Entry {
                start: 1,
                offset: 0x200,
                inode: 6,
                t: InodeId::BasicDirectory,
                name_size: 0x01,
                name: [b'z', b'z'].to_vec(),
            },
            Entry {
                start: 1,
                offset: 0x300,
                inode: 5,
                t: InodeId::BasicDirectory,
                name_size: 0x01,
                name: [b'b', b'b'].to_vec(),
            },
        ];

        let dir = Entry::into_dir(&mut entries);
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
