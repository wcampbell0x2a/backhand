//! Shared pieces for the targets whose input is a *description of a filesystem to build*
//! rather than an image to parse.
//!
//! Unlike the reader targets, these are structured: `Arbitrary` is what we want here. See
//! bytes.rs for why the reader targets deliberately take `&[u8]` instead.

// Each fuzz target is its own crate root, so anything a given target does not use looks
// dead from that target's build.
#![allow(dead_code)]

use std::io::Cursor;
use std::path::Path;

use backhand::compression::Compressor;
use backhand::{FilesystemCompressor, FilesystemWriter, NodeHeader};
use libfuzzer_sys::arbitrary::{self, Arbitrary, Result, Unstructured};

/// A name longer than this cannot survive a write today: v4/inode.rs stores it as
/// `name.len() as u16 - 1` and v4/dir.rs asserts `name_size < 256`, so anything over the
/// limit truncates or trips a deku assert rather than returning an error. Capping here
/// keeps the targets on the bugs they are meant to find; drop the cap once push_* rejects
/// long names properly, since that boundary is worth fuzzing on its own.
const MAX_NAME_LEN: usize = 255;

#[derive(Debug, Default)]
pub struct Header(pub NodeHeader);

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

fn consume_path<'a>(u: &mut Unstructured<'a>, size: usize) -> Result<&'a Path> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = u.bytes(size.min(MAX_NAME_LEN))?;
    Ok(Path::new(std::ffi::OsStr::from_bytes(bytes)))
}

// NOTE: don't use the PathBuf implementation of Arbitrary, it relies on the &str one. That
// has no size limit, and we also want paths made of non-utf8 bytes.
#[derive(Debug)]
pub struct MyPath<'a>(pub &'a Path);

impl<'a> Arbitrary<'a> for MyPath<'a> {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        let size = u.arbitrary_len::<u8>()?;
        Ok(MyPath(consume_path(u, size)?))
    }

    fn arbitrary_take_rest(mut u: Unstructured<'a>) -> Result<Self> {
        let size = u.len();
        Ok(MyPath(consume_path(&mut u, size)?))
    }

    #[inline]
    fn size_hint(_depth: usize) -> (usize, Option<usize>) {
        (0, None)
    }
}

#[derive(Debug)]
pub struct MyData<'a>(pub &'a [u8]);

impl<'a> Arbitrary<'a> for MyData<'a> {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        // limit the file size, for speed
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
pub enum Node<'a> {
    File { path: MyPath<'a>, header: Header, data: MyData<'a> },
    Dir { path: MyPath<'a>, header: Header },
    Symlink { src: MyPath<'a>, header: Header, dst: MyPath<'a> },
    CharDev { file: MyPath<'a>, header: Header, device_num: u32 },
    BlockDev { file: MyPath<'a>, header: Header, device_num: u32 },
    Fifo { path: MyPath<'a>, header: Header },
    Socket { path: MyPath<'a>, header: Header },
}

impl<'a> Node<'a> {
    pub fn path(&self) -> &'a Path {
        match self {
            Node::File { path, .. }
            | Node::Dir { path, .. }
            | Node::Symlink { src: path, .. }
            | Node::CharDev { file: path, .. }
            | Node::BlockDev { file: path, .. }
            | Node::Fifo { path, .. }
            | Node::Socket { path, .. } => path.0,
        }
    }
}

#[derive(Debug, Arbitrary)]
pub struct Squashfs<'a> {
    pub time: u32,
    pub nodes: Vec<Node<'a>>,
}

impl<'a> Squashfs<'a> {
    pub fn to_writer(&'a self) -> FilesystemWriter<'static, 'static, 'a> {
        // NOTE: no compression, to make it fast. Targets that care about the compressors
        // override this with `set_compressor` on the returned writer.
        self.to_writer_with(FilesystemCompressor::new(Compressor::Uncompressed, None).unwrap())
    }

    pub fn to_writer_with(
        &'a self,
        compressor: FilesystemCompressor,
    ) -> FilesystemWriter<'static, 'static, 'a> {
        let mut fs = FilesystemWriter::default();
        fs.set_compressor(compressor);
        fs.set_time(self.time);

        for node in self.nodes.iter() {
            if let Some(parent) = node.path().parent() {
                let _ = fs.push_dir_all(parent, NodeHeader::default());
            }
            // ignore errors from the push_* functions, a rejected node is not a bug
            let _ = match node {
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
                Node::Fifo { path, header } => fs.push_fifo(path.0, header.0),
                Node::Socket { path, header } => fs.push_socket(path.0, header.0),
            };
        }
        fs
    }
}
