#![no_main]

use std::io::Cursor;

use backhand::{FilesystemReader, FilesystemWriter, InnerNode, NodeHeader, SquashfsFileReader};
use libfuzzer_sys::fuzz_target;

mod common;

use common::Squashfs;

/// The tree as it survives a round trip: path, metadata, and node kind.
fn tree(fs: &FilesystemReader<'_>) -> Vec<(std::path::PathBuf, NodeHeader, u8)> {
    fs.files()
        .map(|node| {
            let kind = match &node.inner {
                InnerNode::File(SquashfsFileReader::Basic(_)) => 0,
                InnerNode::File(SquashfsFileReader::Extended(_)) => 1,
                InnerNode::Symlink(_) => 2,
                InnerNode::Dir(_) => 3,
                InnerNode::CharacterDevice(_) => 4,
                InnerNode::BlockDevice(_) => 5,
                InnerNode::NamedPipe => 6,
                InnerNode::Socket => 7,
            };
            (node.fullpath.clone(), node.header, kind)
        })
        .collect()
}

fuzz_target!(|input: Squashfs| {
    // step 1: generate a squashfs image from the random input
    let mut file_1 = Vec::new();
    let _ = input.to_writer().write(Cursor::new(&mut file_1)).unwrap();

    // step 2: parse the generated image
    // everything FilesystemWriter produces should be readable
    let fs_1 = FilesystemReader::from_reader(Cursor::new(&file_1)).unwrap();

    // step 3: use the parsed image to generate another one
    let mut writer_2 = FilesystemWriter::from_fs_reader(&fs_1).unwrap();
    let mut file_2 = Vec::new();
    let _ = writer_2.write(Cursor::new(&mut file_2)).unwrap();

    // step 4: the second image has to describe the same filesystem as the first
    let fs_2 = FilesystemReader::from_reader(Cursor::new(&file_2)).unwrap();
    assert_eq!(tree(&fs_1), tree(&fs_2));

    // Byte-for-byte identity is a stronger claim than the above: duplicate detection,
    // fragment packing, id table ordering and padding all give the second write room to
    // differ legitimately. Kept behind a feature so it can be explored on purpose rather
    // than drowning the default runs.
    #[cfg(feature = "strict-roundtrip")]
    assert_eq!(file_1, file_2);
});
