#![no_main]

use std::io::{Cursor, Read};
use std::path::PathBuf;

use backhand::{FilesystemReader, FilesystemWriter, InnerNode};
use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;

mod common;

use common::{Header, MyData, MyPath};

// The other writer targets only ever build an image and read it back. Nothing fuzzes the
// path `backhand-add` and `backhand-replace` actually take: parse an existing image, edit
// the tree in place, and write it out again - often at an offset, and with the padding,
// deduplication and id-table switches flipped. Each of those changes the layout of the
// image being produced, so each is a chance to emit something the reader cannot parse.

#[derive(Debug, Arbitrary)]
enum Op<'a> {
    /// Replace a file that exists, picked by position rather than by name: a random path
    /// would miss every time and only ever exercise the `FileNotFound` arm.
    ReplaceFile { index: u16, data: MyData<'a> },
    PushFile { path: MyPath<'a>, header: Header, data: MyData<'a> },
    /// Small on purpose: `pad_kib` is scaled by 1024 into the length of a run of zeros
    /// that gets written out, so a large value here would only measure how fast the disk
    /// is.
    KibPadding(u8),
    NoPadding,
    NoDuplicateFiles(bool),
    EmitCompressionOptions(bool),
    OnlyRootId,
    RootMode(u16),
    RootUid(u32),
    RootGid(u32),
}

#[derive(Debug, Arbitrary)]
struct Input<'a> {
    base: common::Squashfs<'a>,
    ops: Vec<Op<'a>>,
    /// Images live at an offset inside firmware dumps; `write_with_offset` and
    /// `from_reader_with_offset` are the pair that has to agree on where it starts.
    offset: u16,
}

/// Reads every file, which for an image we just wrote has to succeed.
fn read_all(fs: &FilesystemReader<'_>) {
    let mut buf = Vec::new();
    for node in fs.files() {
        if let InnerNode::File(file) = &node.inner {
            buf.clear();
            fs.file(file).reader().read_to_end(&mut buf).unwrap();
        }
    }
}

fuzz_target!(|input: Input| {
    // step 1: build a base image and parse it back, as the other writer targets do
    let mut file_1 = Vec::new();
    input.base.to_writer().write(Cursor::new(&mut file_1)).unwrap();
    let fs_1 = FilesystemReader::from_reader(Cursor::new(&file_1)).unwrap();

    let paths: Vec<PathBuf> = fs_1
        .files()
        .filter(|node| matches!(node.inner, InnerNode::File(_)))
        .map(|node| node.fullpath.clone())
        .collect();

    // step 2: edit it the way the add/replace binaries do
    let mut fs = FilesystemWriter::from_fs_reader(&fs_1).unwrap();
    for op in input.ops.iter() {
        match op {
            Op::ReplaceFile { index, data } => {
                if !paths.is_empty() {
                    let path = &paths[usize::from(*index) % paths.len()];
                    // the path came out of the image, so this has to find it
                    fs.replace_file(path, Cursor::new(data.0)).unwrap();
                }
            }
            // a rejected node is not a bug
            Op::PushFile { path, header, data } => {
                if let Some(parent) = path.0.parent() {
                    let _ = fs.push_dir_all(parent, Default::default());
                }
                let _ = fs.push_file(Cursor::new(data.0), path.0, header.0);
            }
            Op::KibPadding(kib) => fs.set_kib_padding(u32::from(*kib)),
            Op::NoPadding => fs.set_no_padding(),
            Op::NoDuplicateFiles(value) => fs.set_no_duplicate_files(*value),
            Op::EmitCompressionOptions(value) => fs.set_emit_compression_options(*value),
            Op::OnlyRootId => fs.set_only_root_id(),
            Op::RootMode(mode) => fs.set_root_mode(*mode),
            Op::RootUid(uid) => fs.set_root_uid(*uid),
            Op::RootGid(gid) => fs.set_root_gid(*gid),
        }
    }

    // step 3: whatever the edits were, the result has to be a readable image at `offset`
    let offset = u64::from(input.offset);
    let mut file_2 = Vec::new();
    fs.write_with_offset(Cursor::new(&mut file_2), offset).unwrap();
    let fs_2 = FilesystemReader::from_reader_with_offset(Cursor::new(&file_2), offset).unwrap();
    read_all(&fs_2);
});
