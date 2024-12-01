#![no_main]

use std::io::Cursor;
use std::path::Path;

use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};

use libfuzzer_sys::arbitrary::{self, Arbitrary, Result, Unstructured};
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Default)]
struct Header(NodeHeader);

impl Arbitrary<'_> for Header {
    fn arbitrary(u: &mut Unstructured<'_>) -> Result<Self> {
        Ok(Self(NodeHeader {
            permissions: u.arbitrary()?,
            uid: u.arbitrary()?,
            gid: u.arbitrary()?,
            mtime: u.arbitrary()?,
        }))
    }

    #[inline]
    fn size_hint(_depth: usize) -> (usize, Option<usize>) {
        (14, Some(14))
    }
}

fn consume_path<'a>(u: &mut Unstructured<'a>, size: usize) -> &'a Path {
    // limit Paths to 255
    let bytes = u.bytes(size.min(255)).unwrap();
    use std::os::unix::ffi::*;
    let os_str = std::ffi::OsStr::from_bytes(bytes);
    Path::new(os_str)
}

// NOTE don't use the PathBuf implementation of Arbitary, because it rely on the
// &str implementation. This is a problem because it don't have a size limit
// and we want to also have paths made of non-utf8 bytes
#[derive(Debug)]
struct MyPath<'a>(&'a Path);
impl<'a> Arbitrary<'a> for MyPath<'a> {
    #[cfg(unix)]
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        let size = u.arbitrary_len::<u8>()?;
        Ok(MyPath(consume_path(u, size)))
    }

    fn arbitrary_take_rest(mut u: Unstructured<'a>) -> Result<Self> {
        let size = u.len();
        Ok(MyPath(consume_path(&mut u, size)))
    }

    #[inline]
    fn size_hint(_depth: usize) -> (usize, Option<usize>) {
        (0, None)
    }
}

#[derive(Debug)]
struct MyData<'a>(&'a [u8]);
impl<'a> Arbitrary<'a> for MyData<'a> {
    #[cfg(unix)]
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        // limit the file size to 10, for speed...
        let size = u.arbitrary_len::<u8>()?.min(10);
        Ok(MyData(u.bytes(size)?))
    }

    fn arbitrary_take_rest(mut u: Unstructured<'a>) -> Result<Self> {
        let size = u.len();
        Ok(MyData(u.bytes(size)?))
    }

    #[inline]
    fn size_hint(_depth: usize) -> (usize, Option<usize>) {
        (0, None)
    }
}

#[derive(Debug, Arbitrary)]
enum Node<'a> {
    File { path: MyPath<'a>, header: Header, data: MyData<'a> },
    Dir { path: MyPath<'a>, header: Header },
    Symlink { src: MyPath<'a>, header: Header, dst: MyPath<'a> },
    CharDev { file: MyPath<'a>, header: Header, device_num: u32 },
    BlockDev { file: MyPath<'a>, header: Header, device_num: u32 },
}

impl<'a> Node<'a> {
    fn path(&self) -> &'a Path {
        match self {
            Node::File { path, .. }
            | Node::Dir { path, .. }
            | Node::Symlink { src: path, .. }
            | Node::CharDev { file: path, .. }
            | Node::BlockDev { file: path, .. } => path.0,
        }
    }
}

#[derive(Debug, Arbitrary)]
struct Squashfs<'a> {
    time: u32,
    nodes: Vec<Node<'a>>,
}

impl<'a> Squashfs<'a> {
    fn into_writer(&'a self) -> FilesystemWriter<'static, 'static, 'a> {
        let mut fs = FilesystemWriter::default();
        // NOTE no compression to make it fast
        fs.set_compressor(
            backhand::FilesystemCompressor::new(backhand::compression::Compressor::None, None)
                .unwrap(),
        );
        fs.set_time(self.time);

        for node in self.nodes.iter() {
            if let Some(parent) = node.path().parent() {
                let _ = fs.push_dir_all(parent, NodeHeader::default());
            }
            // ignore errors from push_* functions
            let _ = match &node {
                Node::File { path, header, data } => {
                    fs.push_file(Cursor::new(data.0), path.0, header.0)
                }
                Node::Dir { path, header } => fs.push_dir(path.0, header.0),
                Node::Symlink { src, header, dst } => fs.push_symlink(dst.0, src.0, header.0),
                Node::CharDev { file, header, device_num } => {
                    fs.push_char_device(*device_num, file.0, header.0)
                }
                Node::BlockDev { file, header, device_num } => {
                    fs.push_block_device(*device_num, file.0, header.0)
                }
            };
        }
        fs
    }
}

fuzz_target!(|input: Squashfs| {
    // step 1: generate a squashfs file from the random input
    let mut file_1 = Vec::new();
    let _ = input.into_writer().write(Cursor::new(&mut file_1)).unwrap();

    // step 2: parse the generated file
    // all files create using FilesystemWriter should be valid
    let fs_reader = FilesystemReader::from_reader(Cursor::new(&file_1)).unwrap();

    // step 3: use the parsed file to generate other file
    let mut squashfs_2 = FilesystemWriter::from_fs_reader(&fs_reader).unwrap();
    let mut file_2 = Vec::new();
    let _ = squashfs_2.write(Cursor::new(&mut file_2)).unwrap();

    // step 4: verify parsed data
    // both generated files need to be equal
    assert_eq!(file_1, file_2);
});
