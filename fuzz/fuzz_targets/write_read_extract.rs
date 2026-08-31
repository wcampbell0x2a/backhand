#![no_main]

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::PathBuf;

use backhand::compression::Compressor;
use backhand::{
    FilesystemCompressor, FilesystemReader, FilesystemWriter, InnerNode, MIN_BLOCK_SIZE, NodeHeader,
};
use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;

mod common;

use common::{Header, MyData, MyPath};

// write_read_write compares the *tree* a round trip produces, with compression off and
// files of at most ten bytes, so every file it writes lands in a single fragment. This
// target closes both gaps: it writes with a real compressor at a fuzzed block size, makes
// files large enough to span several blocks, and compares the file *contents* rather than
// just the metadata. That is what puts the block writer, the fragment packer and the
// decompressors on the same round trip.

/// Compressors a default build can write. Lz4 is absent on purpose: it errors without
/// options, so it would only ever exercise the rejection path. Lzma and Lzo are not
/// supported for writing at all.
const COMPRESSORS: &[Compressor] =
    &[Compressor::Uncompressed, Compressor::Gzip, Compressor::Xz, Compressor::Zstd];

/// Enough to cross a block boundary several times at the smallest block size, while
/// keeping an execution fast enough to be worth running millions of times.
const MAX_TOTAL_DATA: usize = 512 << 10;

#[derive(Debug, Arbitrary)]
struct FileSpec<'a> {
    path: MyPath<'a>,
    header: Header,
    /// Repeated to reach `repeat` copies: a short seed keeps the input small while still
    /// producing files big enough to be split into blocks, and repetition is what makes
    /// the compressors actually compress.
    seed: MyData<'a>,
    repeat: u16,
}

#[derive(Debug, Arbitrary)]
struct Input<'a> {
    compressor: u8,
    /// Selects a power-of-two block size in `MIN_BLOCK_SIZE..=MAX_BLOCK_SIZE`.
    block_size_log: u8,
    time: u32,
    files: Vec<FileSpec<'a>>,
}

/// Every file in the image, by path, with its contents.
fn contents(fs: &FilesystemReader<'_>) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    for node in fs.files() {
        if let InnerNode::File(file) = &node.inner {
            let mut buf = Vec::new();
            // We wrote this image ourselves, so reading it back has to succeed. An error
            // here is the finding.
            fs.file(file).reader().read_to_end(&mut buf).unwrap();
            out.insert(node.fullpath.clone(), buf);
        }
    }
    out
}

fuzz_target!(|input: Input| {
    let compressor = COMPRESSORS[usize::from(input.compressor) % COMPRESSORS.len()];
    // MIN_BLOCK_SIZE is 4 KiB and MAX_BLOCK_SIZE 1 MiB, ie. logs 12 through 20
    let block_size = MIN_BLOCK_SIZE << (u32::from(input.block_size_log) % 9);

    // The buffers have to outlive the writer that borrows them, so expand them all first
    let mut budget = MAX_TOTAL_DATA;
    let data: Vec<Vec<u8>> = input
        .files
        .iter()
        .map(|spec| {
            if spec.seed.0.is_empty() {
                return Vec::new();
            }
            let want = spec.seed.0.len().saturating_mul(usize::from(spec.repeat));
            let len = want.min(budget);
            budget -= len;
            let mut buf = spec.seed.0.repeat(len.div_ceil(spec.seed.0.len()));
            buf.truncate(len);
            buf
        })
        .collect();

    let mut fs = FilesystemWriter::default();
    fs.set_compressor(FilesystemCompressor::new(compressor, None).unwrap());
    fs.set_block_size(block_size);
    fs.set_time(input.time);

    for (spec, data) in input.files.iter().zip(data.iter()) {
        if let Some(parent) = spec.path.0.parent() {
            let _ = fs.push_dir_all(parent, NodeHeader::default());
        }
        // ignore errors from push_file, a rejected node is not a bug
        let _ = fs.push_file(Cursor::new(data.as_slice()), spec.path.0, spec.header.0);
    }

    // step 1: everything FilesystemWriter produces has to be readable, contents included
    let mut file_1 = Vec::new();
    fs.write(Cursor::new(&mut file_1)).unwrap();
    let fs_1 = FilesystemReader::from_reader(Cursor::new(&file_1)).unwrap();
    let contents_1 = contents(&fs_1);

    // step 2: rebuilding from the parsed image has to preserve every byte. This is the
    // path `add`/`replace` take, and the one where a block size or fragment carried over
    // from the source image would corrupt the copy.
    let mut writer_2 = FilesystemWriter::from_fs_reader(&fs_1).unwrap();
    let mut file_2 = Vec::new();
    writer_2.write(Cursor::new(&mut file_2)).unwrap();
    let fs_2 = FilesystemReader::from_reader(Cursor::new(&file_2)).unwrap();

    assert_eq!(contents_1, contents(&fs_2));
});
