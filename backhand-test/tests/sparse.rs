//! Sparse file (hole) support in the v4 writer
//!
//! A data block of all zeros is stored as a hole: a block size of 0 with no bytes in the image.
//! The `sparse` field of the inode holds the number of hole bytes. The kernel uses it for
//! `st_blocks`, and squashfs-tools/unsquashfs uses it to write holes on extract.

#![cfg(feature = "xz")]

use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

use backhand::{
    DEFAULT_BLOCK_SIZE, FilesystemReader, FilesystemWriter, InnerNode, NodeHeader,
    SquashfsFileReader,
};

const BLOCK: usize = DEFAULT_BLOCK_SIZE as usize;
const FILE_NAME: &str = "file";

/// Content of one data block in a test file
#[derive(Clone, Copy, Debug)]
enum Block {
    Zero,
    Data,
}

/// Make file content from `blocks`, each [`BLOCK`] bytes long, then `tail` bytes of `tail_kind`
fn make_content(blocks: &[Block], tail: usize, tail_kind: Block) -> Vec<u8> {
    let fill = |kind: Block, len: usize, seed: usize| -> Vec<u8> {
        match kind {
            Block::Zero => vec![0; len],
            // Never all zeros, and not the same for each block
            Block::Data => (0..len).map(|i| ((i + seed) % 251) as u8 + 1).collect(),
        }
    };
    let mut content: Vec<u8> =
        blocks.iter().enumerate().flat_map(|(n, kind)| fill(*kind, BLOCK, n)).collect();
    content.extend(fill(tail_kind, tail, blocks.len()));
    content
}

/// The hole bytes that mksquashfs records for `content`, capped at `len - 1`
///
/// A file smaller than one block goes into a fragment and has no holes.
fn expected_sparse(content: &[u8]) -> u64 {
    if content.len() < BLOCK {
        return 0;
    }
    let holes: u64 = content
        .chunks(BLOCK)
        .filter(|chunk| chunk.iter().all(|b| *b == 0))
        .map(|chunk| chunk.len() as u64)
        .sum();
    holes.min(content.len() as u64 - 1)
}

fn write_image(content: &[u8]) -> Vec<u8> {
    let mut fs = FilesystemWriter::default();
    // squashfs-tools/unsquashfs must be able to read the file
    fs.set_root_mode(0o755);
    let header = NodeHeader::new(0o644, 0, 0, 0);
    fs.push_file(Cursor::new(content.to_vec()), FILE_NAME, header).unwrap();
    let mut out = Cursor::new(vec![]);
    fs.write(&mut out).unwrap();
    out.into_inner()
}

/// Read the image again through [`FilesystemWriter::from_fs_reader`]
fn copy_image(image: &[u8]) -> Vec<u8> {
    let reader = FilesystemReader::from_reader(Cursor::new(image)).unwrap();
    let mut fs = FilesystemWriter::from_fs_reader(&reader).unwrap();
    let mut out = Cursor::new(vec![]);
    fs.write(&mut out).unwrap();
    out.into_inner()
}

/// Return the inode and the content of the file named `name`
fn read_file(image: &[u8], name: &str) -> (SquashfsFileReader, Vec<u8>) {
    let reader = FilesystemReader::from_reader(Cursor::new(image)).unwrap();
    let node =
        reader.files().find(|node| node.fullpath.file_name().is_some_and(|n| n == name)).unwrap();
    let InnerNode::File(file) = &node.inner else { panic!("{name} is not a file") };
    let mut content = vec![];
    reader.file(file).reader().read_to_end(&mut content).unwrap();
    (file.clone(), content)
}

fn hole_count(file: &SquashfsFileReader) -> usize {
    file.block_sizes().iter().filter(|size| size.size() == 0).count()
}

/// Check the image written from `content`, and a copy of that image
fn assert_round_trip(content: &[u8]) {
    let image = write_image(content);
    let (file, read) = read_file(&image, FILE_NAME);
    assert!(read == content, "content changed, len {}", content.len());
    assert_eq!(file.sparse(), expected_sparse(content));
    match (&file, expected_sparse(content)) {
        (SquashfsFileReader::Basic(_), 0) => {}
        (SquashfsFileReader::Extended(extended), 1..) => assert_eq!(extended.link_count, 1),
        (file, sparse) => panic!("wrong inode type for sparse {sparse}: {file:?}"),
    }

    let copy = copy_image(&image);
    let (copied_file, copied_read) = read_file(&copy, FILE_NAME);
    assert!(copied_read == content, "copied content changed, len {}", content.len());
    assert_eq!(copied_file.sparse(), file.sparse());
    assert_eq!(hole_count(&copied_file), hole_count(&file));
}

#[test]
fn test_zero_blocks_become_holes() {
    let content =
        make_content(&[Block::Data, Block::Zero, Block::Data, Block::Zero], 0, Block::Data);
    let (file, _) = read_file(&write_image(&content), FILE_NAME);
    assert_eq!(hole_count(&file), 2);
    assert_eq!(file.sparse(), 2 * BLOCK as u64);
    assert_round_trip(&content);
}

#[test]
fn test_zero_tail_block_becomes_hole() {
    let content = make_content(&[Block::Data], 100, Block::Zero);
    let (file, _) = read_file(&write_image(&content), FILE_NAME);
    assert_eq!(hole_count(&file), 1);
    assert_eq!(file.sparse(), 100);
    assert_round_trip(&content);
}

#[test]
fn test_all_zero_file_caps_sparse() {
    let content = make_content(&[Block::Zero, Block::Zero], 0, Block::Data);
    let (file, _) = read_file(&write_image(&content), FILE_NAME);
    assert_eq!(hole_count(&file), 2);
    assert_eq!(file.sparse(), content.len() as u64 - 1);
    assert_round_trip(&content);
}

#[test]
fn test_no_zero_blocks_stays_basic() {
    let content = make_content(&[Block::Data, Block::Data], 7, Block::Data);
    let (file, _) = read_file(&write_image(&content), FILE_NAME);
    assert!(matches!(file, SquashfsFileReader::Basic(_)));
    assert_round_trip(&content);
}

#[test]
fn test_zero_file_smaller_than_block_is_fragment() {
    let content = vec![0; BLOCK - 1];
    let (file, _) = read_file(&write_image(&content), FILE_NAME);
    assert!(matches!(file, SquashfsFileReader::Basic(_)));
    assert_eq!(hole_count(&file), 0);
    assert_round_trip(&content);
}

#[test]
fn test_empty_file() {
    assert_round_trip(&[]);
}

/// Random mixes of data blocks, zero blocks, and tails
#[test]
fn test_random_block_mix() {
    let mut rng = fastrand::Rng::with_seed(0x5ba5e);
    for _ in 0..64 {
        let kind = |rng: &mut fastrand::Rng| if rng.bool() { Block::Zero } else { Block::Data };
        let blocks: Vec<Block> = (0..rng.usize(0..6)).map(|_| kind(&mut rng)).collect();
        let tail = if rng.bool() { 0 } else { rng.usize(1..BLOCK) };
        let tail_kind = kind(&mut rng);
        assert_round_trip(&make_content(&blocks, tail, tail_kind));
    }
}

/// Write a file with real holes on disk: data, hole, data, hole
fn write_sparse_file(path: &Path) -> Vec<u8> {
    let content =
        make_content(&[Block::Data, Block::Zero, Block::Data, Block::Zero], 0, Block::Data);
    let mut file = File::create(path).unwrap();
    file.write_all(&content[..BLOCK]).unwrap();
    file.seek(SeekFrom::Start(2 * BLOCK as u64)).unwrap();
    file.write_all(&content[2 * BLOCK..3 * BLOCK]).unwrap();
    file.set_len(content.len() as u64).unwrap();
    content
}

fn run(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(output.status.success(), "{command:?}: {}", String::from_utf8_lossy(&output.stderr));
}

fn unsquashfs(image: &Path, dest: &Path) {
    run(Command::new("unsquashfs").arg("-no-xattrs").arg("-d").arg(dest).arg(image));
}

/// squashfs-tools must read our holes, and write them as holes on extract
#[test]
#[cfg(feature = "__test_unsquashfs")]
fn test_squashfs_tools_extracts_holes() {
    let tmp = tempfile::tempdir().unwrap();
    let content = make_content(&[Block::Data, Block::Zero, Block::Zero], 0, Block::Data);
    let image = tmp.path().join("image.squashfs");
    std::fs::write(&image, write_image(&content)).unwrap();

    let dest = tmp.path().join("out");
    unsquashfs(&image, &dest);
    let extracted = dest.join(FILE_NAME);
    assert!(std::fs::read(&extracted).unwrap() == content);
    let allocated = std::fs::metadata(&extracted).unwrap().blocks() * 512;
    assert!(allocated < content.len() as u64, "file has no holes: {allocated} bytes allocated");
}

/// A copy of a mksquashfs image keeps the holes and the `sparse` field
#[test]
#[cfg(feature = "__test_unsquashfs")]
fn test_copy_of_mksquashfs_image() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir(&src).unwrap();
    let content = write_sparse_file(&src.join(FILE_NAME));

    let image = tmp.path().join("image.squashfs");
    run(Command::new("mksquashfs").arg(&src).arg(&image).args([
        "-comp",
        "xz",
        "-noappend",
        "-no-xattrs",
        "-quiet",
    ]));
    let image = std::fs::read(&image).unwrap();
    let (file, _) = read_file(&image, FILE_NAME);
    assert_eq!(hole_count(&file), 2);

    let copy = copy_image(&image);
    let (copied_file, copied_read) = read_file(&copy, FILE_NAME);
    assert!(copied_read == content);
    assert_eq!(copied_file.sparse(), file.sparse());
    assert_eq!(hole_count(&copied_file), 2);

    let copy_path = tmp.path().join("copy.squashfs");
    std::fs::write(&copy_path, &copy).unwrap();
    let dest = tmp.path().join("out");
    unsquashfs(&copy_path, &dest);
    assert!(std::fs::read(dest.join(FILE_NAME)).unwrap() == content);
}
