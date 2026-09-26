//! Duplicate file detection in the v4 writer
//!
//! A file with the same content as an earlier file uses the data of that file. The writer must
//! compare the whole file, not only the first block.

#![cfg(feature = "xz")]

use std::io::{Cursor, Read};
use std::process::Command;

use backhand::{
    DEFAULT_BLOCK_SIZE, FilesystemReader, FilesystemWriter, InnerNode, NodeHeader,
    SquashfsFileReader,
};

const BLOCK: usize = DEFAULT_BLOCK_SIZE as usize;

/// Bytes that do not compress well, from `seed`
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(seed);
    (0..len).map(|_| rng.u8(..)).collect()
}

/// Write `files` as (name, content) into an image
fn write_image(files: &[(&str, &[u8])], duplicate_check: bool) -> Vec<u8> {
    let mut fs = FilesystemWriter::default();
    fs.set_no_duplicate_files(duplicate_check);
    // squashfs-tools/unsquashfs must be able to read the files
    fs.set_root_mode(0o755);
    for (name, content) in files {
        let header = NodeHeader::new(0o644, 0, 0, 0);
        fs.push_file(Cursor::new(content.to_vec()), name, header).unwrap();
    }
    let mut out = Cursor::new(vec![]);
    fs.write(&mut out).unwrap();
    out.into_inner()
}

/// Return the inode and the content of each file, in the order of `names`
fn read_files(image: &[u8], names: &[&str]) -> Vec<(SquashfsFileReader, Vec<u8>)> {
    let reader = FilesystemReader::from_reader(Cursor::new(image)).unwrap();
    names
        .iter()
        .map(|name| {
            let node = reader
                .files()
                .find(|node| node.fullpath.file_name().is_some_and(|n| n == *name))
                .unwrap();
            let InnerNode::File(file) = &node.inner else { panic!("{name} is not a file") };
            let mut content = vec![];
            reader.file(file).reader().read_to_end(&mut content).unwrap();
            (file.clone(), content)
        })
        .collect()
}

/// Check that each file reads back with its own content, and return the inodes
fn assert_contents(image: &[u8], files: &[(&str, &[u8])]) -> Vec<SquashfsFileReader> {
    let names: Vec<&str> = files.iter().map(|(name, _)| *name).collect();
    read_files(image, &names)
        .into_iter()
        .zip(files)
        .map(|((inode, read), (name, content))| {
            assert!(read == *content, "{name}: content changed, len {}", content.len());
            inode
        })
        .collect()
}

#[test]
fn test_same_first_block_is_not_duplicate() {
    let first = noise(3 * BLOCK, 1);
    let mut second = first.clone();
    second[2 * BLOCK + 5] ^= 0xff;
    let files: [(&str, &[u8]); 2] = [("first", &first), ("second", &second)];

    let inodes = assert_contents(&write_image(&files, true), &files);
    assert_ne!(inodes[0].blocks_start(), inodes[1].blocks_start());
}

#[test]
fn test_same_first_block_and_different_len_is_not_duplicate() {
    let first = noise(2 * BLOCK, 2);
    let second = [&first[..], &noise(10, 3)].concat();
    let files: [(&str, &[u8]); 2] = [("first", &first), ("second", &second)];
    assert_contents(&write_image(&files, true), &files);
}

#[test]
fn test_duplicate_uses_same_data() {
    let content = noise(2 * BLOCK + 100, 4);
    let files: [(&str, &[u8]); 2] = [("first", &content), ("second", &content)];

    let with_check = write_image(&files, true);
    let inodes = assert_contents(&with_check, &files);
    assert_eq!(inodes[0].blocks_start(), inodes[1].blocks_start());
    assert_eq!(inodes[0].block_sizes(), inodes[1].block_sizes());

    let without_check = write_image(&files, false);
    let inodes = assert_contents(&without_check, &files);
    assert_ne!(inodes[0].blocks_start(), inodes[1].blocks_start());
    assert!(with_check.len() < without_check.len());
}

/// The data after a duplicate goes where the duplicate data was written first
#[test]
fn test_data_after_duplicate() {
    let content = noise(2 * BLOCK, 5);
    let other = noise(3 * BLOCK + 7, 6);
    let small = noise(20, 7);
    let files: [(&str, &[u8]); 4] =
        [("a", &content), ("b", &content), ("c", &other), ("d", &small)];
    let inodes = assert_contents(&write_image(&files, true), &files);
    assert_eq!(inodes[0].blocks_start(), inodes[1].blocks_start());
    assert_eq!(inodes[2].blocks_start(), inodes[0].blocks_start() + 2 * BLOCK as u64);
}

/// squashfs-tools must read an image where the last file is a duplicate
#[test]
#[cfg(feature = "__test_unsquashfs")]
fn test_squashfs_tools_reads_duplicate_at_end() {
    let content = noise(4 * BLOCK, 8);
    let files: [(&str, &[u8]); 2] = [("first", &content), ("last", &content)];
    let tmp = tempfile::tempdir().unwrap();
    let image = tmp.path().join("image.squashfs");
    std::fs::write(&image, write_image(&files, true)).unwrap();

    let dest = tmp.path().join("out");
    let output =
        Command::new("unsquashfs").arg("-no-xattrs").arg("-d").arg(&dest).arg(&image).output();
    let output = output.unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    for (name, content) in files {
        assert!(std::fs::read(dest.join(name)).unwrap() == content, "{name}: content changed");
    }
}

/// Random files made from a small set of blocks, thus with many shared first blocks
#[test]
fn test_random_shared_blocks() {
    let pieces: Vec<Vec<u8>> = (0..3).map(|seed| noise(BLOCK, 100 + seed)).collect();
    let mut rng = fastrand::Rng::with_seed(0xd0b);
    for _ in 0..8 {
        let contents: Vec<Vec<u8>> = (0..6)
            .map(|_| {
                let blocks = rng.usize(1..4);
                let mut content: Vec<u8> =
                    (0..blocks).flat_map(|_| pieces[rng.usize(..pieces.len())].clone()).collect();
                content.truncate(content.len() - rng.usize(0..2) * rng.usize(..BLOCK));
                content
            })
            .collect();
        let names: Vec<String> = (0..contents.len()).map(|n| format!("file{n}")).collect();
        let files: Vec<(&str, &[u8])> =
            names.iter().map(String::as_str).zip(contents.iter().map(Vec::as_slice)).collect();
        assert_contents(&write_image(&files, true), &files);
    }
}
