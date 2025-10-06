use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct BackhandNodeHeader {
    pub permissions: u16,
    pub uid: u32,
    pub gid: u32,
    pub mtime: u32,
}

impl From<crate::v4::filesystem::node::NodeHeader> for BackhandNodeHeader {
    fn from(header: crate::v4::filesystem::node::NodeHeader) -> Self {
        Self {
            permissions: header.permissions,
            uid: header.uid,
            gid: header.gid,
            mtime: header.mtime,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct BackhandDataSize {
    pub size: u32,
    pub uncompressed: bool,
}

impl From<crate::v4::data::DataSize> for BackhandDataSize {
    fn from(ds: crate::v4::data::DataSize) -> Self {
        Self { size: ds.size(), uncompressed: ds.uncompressed() }
    }
}

impl BackhandDataSize {
    pub fn to_v4_datasize(self) -> crate::v4::data::DataSize {
        crate::v4::data::DataSize::new(self.size, self.uncompressed)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BackhandSquashfsFileReader {
    Basic {
        blocks_start: u64,
        frag_index: u32,
        block_offset: u32,
        file_size: u64,
        block_sizes: Vec<BackhandDataSize>,
    },
    Extended {
        blocks_start: u64,
        frag_index: u32,
        block_offset: u32,
        file_size: u64,
        sparse: u64,
        link_count: u32,
        xattr_index: u32,
        block_sizes: Vec<BackhandDataSize>,
    },
}

impl BackhandSquashfsFileReader {
    pub fn file_len(&self) -> usize {
        match self {
            Self::Basic { file_size, .. } => *file_size as usize,
            Self::Extended { file_size, .. } => *file_size as usize,
        }
    }

    pub fn frag_index(&self) -> usize {
        match self {
            Self::Basic { frag_index, .. } => *frag_index as usize,
            Self::Extended { frag_index, .. } => *frag_index as usize,
        }
    }

    pub fn block_sizes(&self) -> &[BackhandDataSize] {
        match self {
            Self::Basic { block_sizes, .. } => block_sizes,
            Self::Extended { block_sizes, .. } => block_sizes,
        }
    }

    pub fn blocks_start(&self) -> u64 {
        match self {
            Self::Basic { blocks_start, .. } => *blocks_start,
            Self::Extended { blocks_start, .. } => *blocks_start,
        }
    }

    pub fn block_offset(&self) -> u32 {
        match self {
            Self::Basic { block_offset, .. } => *block_offset,
            Self::Extended { block_offset, .. } => *block_offset,
        }
    }
}

impl From<&crate::v4::filesystem::node::SquashfsFileReader> for BackhandSquashfsFileReader {
    fn from(v4_file: &crate::v4::filesystem::node::SquashfsFileReader) -> Self {
        match v4_file {
            crate::v4::filesystem::node::SquashfsFileReader::Basic(basic) => Self::Basic {
                blocks_start: basic.blocks_start as u64,
                frag_index: basic.frag_index,
                block_offset: basic.block_offset,
                file_size: basic.file_size as u64,
                block_sizes: basic.block_sizes.iter().map(|&ds| ds.into()).collect(),
            },
            crate::v4::filesystem::node::SquashfsFileReader::Extended(extended) => Self::Extended {
                blocks_start: extended.blocks_start,
                frag_index: extended.frag_index,
                block_offset: extended.block_offset,
                file_size: extended.file_size,
                sparse: extended.sparse,
                link_count: extended.link_count,
                xattr_index: extended.xattr_index,
                block_sizes: extended.block_sizes.iter().map(|&ds| ds.into()).collect(),
            },
        }
    }
}

impl From<&crate::v4::filesystem::node::Node<crate::v4::filesystem::node::SquashfsFileReader>>
    for BackhandNode
{
    fn from(
        v4_node: &crate::v4::filesystem::node::Node<
            crate::v4::filesystem::node::SquashfsFileReader,
        >,
    ) -> Self {
        let inner = match &v4_node.inner {
            crate::v4::filesystem::node::InnerNode::File(file) => {
                BackhandInnerNode::File(file.into())
            }
            crate::v4::filesystem::node::InnerNode::Symlink(symlink) => {
                BackhandInnerNode::Symlink { link: symlink.link.clone() }
            }
            crate::v4::filesystem::node::InnerNode::Dir(_) => BackhandInnerNode::Dir,
            crate::v4::filesystem::node::InnerNode::CharacterDevice(dev) => {
                BackhandInnerNode::CharacterDevice { device_number: dev.device_number }
            }
            crate::v4::filesystem::node::InnerNode::BlockDevice(dev) => {
                BackhandInnerNode::BlockDevice { device_number: dev.device_number }
            }
            crate::v4::filesystem::node::InnerNode::NamedPipe => BackhandInnerNode::NamedPipe,
            crate::v4::filesystem::node::InnerNode::Socket => BackhandInnerNode::Socket,
        };
        Self { fullpath: v4_node.fullpath.clone(), header: v4_node.header.into(), inner }
    }
}

#[derive(Debug, Clone)]
pub struct BackhandNode {
    pub fullpath: PathBuf,
    pub header: BackhandNodeHeader,
    pub inner: BackhandInnerNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackhandInnerNode {
    File(BackhandSquashfsFileReader),
    Symlink { link: PathBuf },
    Dir,
    CharacterDevice { device_number: u32 },
    BlockDevice { device_number: u32 },
    NamedPipe,
    Socket,
}

pub trait FilesystemReaderTrait: Send + Sync {
    /// Get all files as a vector of owned backhand nodes
    fn files(&self) -> Vec<BackhandNode>;

    /// Get a file handle that can be used to read file data
    fn file_data(&self, file: &BackhandSquashfsFileReader) -> Vec<u8>;
}

impl<'b> FilesystemReaderTrait for crate::v4::filesystem::reader::FilesystemReader<'b> {
    fn files(&self) -> Vec<BackhandNode> {
        self.files().map(|node| node.into()).collect()
    }

    fn file_data(&self, file: &BackhandSquashfsFileReader) -> Vec<u8> {
        // Convert back to v4 format temporarily for the call
        let v4_file = match file {
            BackhandSquashfsFileReader::Basic {
                blocks_start,
                frag_index,
                block_offset,
                file_size,
                block_sizes,
            } => crate::v4::filesystem::node::SquashfsFileReader::Basic(
                crate::v4::inode::BasicFile {
                    blocks_start: *blocks_start as u32,
                    frag_index: *frag_index,
                    block_offset: *block_offset,
                    file_size: *file_size as u32,
                    block_sizes: block_sizes.iter().map(|&ds| ds.to_v4_datasize()).collect(),
                },
            ),
            BackhandSquashfsFileReader::Extended {
                blocks_start,
                frag_index,
                block_offset,
                file_size,
                sparse,
                link_count,
                xattr_index,
                block_sizes,
            } => crate::v4::filesystem::node::SquashfsFileReader::Extended(
                crate::v4::inode::ExtendedFile {
                    blocks_start: *blocks_start,
                    frag_index: *frag_index,
                    block_offset: *block_offset,
                    file_size: *file_size,
                    sparse: *sparse,
                    link_count: *link_count,
                    xattr_index: *xattr_index,
                    block_sizes: block_sizes.iter().map(|&ds| ds.to_v4_datasize()).collect(),
                },
            ),
        };

        let file_handle = self.file(&v4_file);
        let mut reader = file_handle.reader();
        let mut data = Vec::new();
        if let Err(_e) = std::io::Read::read_to_end(&mut reader, &mut data) {
            // sparse
            return Vec::new();
        }
        data
    }
}
