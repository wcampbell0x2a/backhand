use std::fs::{self, File};

use squashfs_deku::Squashfs;

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2 -always-use-fragments
#[test]
fn test_00() {
    let file = File::open("./lfs/test_00/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let pos_and_inodes = squashfs.inodes();
    let root_inode = squashfs.root_inode(&pos_and_inodes);
    let inodes = squashfs.discard_pos(&pos_and_inodes);
    let dir_blocks = squashfs.dir_blocks(&inodes);
    let fragments = squashfs.fragments();

    let (path, bytes) = squashfs.extract_file(
        "squashfs-deku",
        &dir_blocks,
        &inodes,
        &fragments,
        &root_inode,
    );
    let expected_bytes = fs::read("./lfs/test_00/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2
#[test]
fn test_01() {
    let file = File::open("./lfs/test_01/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let pos_and_inodes = squashfs.inodes();
    let root_inode = squashfs.root_inode(&pos_and_inodes);
    let inodes = squashfs.discard_pos(&pos_and_inodes);
    let dir_blocks = squashfs.dir_blocks(&inodes);
    let fragments = squashfs.fragments();

    let (path, bytes) = squashfs.extract_file(
        "squashfs-deku",
        &dir_blocks,
        &inodes,
        &fragments,
        &root_inode,
    );
    let expected_bytes = fs::read("./lfs/test_01/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz
#[test]
fn test_02() {
    let file = File::open("./lfs/test_02/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let pos_and_inodes = squashfs.inodes();
    let root_inode = squashfs.root_inode(&pos_and_inodes);
    let inodes = squashfs.discard_pos(&pos_and_inodes);
    let dir_blocks = squashfs.dir_blocks(&inodes);
    let fragments = squashfs.fragments();

    let (path, bytes) = squashfs.extract_file(
        "squashfs-deku",
        &dir_blocks,
        &inodes,
        &fragments,
        &root_inode,
    );
    let expected_bytes = fs::read("./lfs/test_02/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);
}

/// mksquashfs ./target/release/squashfs-deku Cargo.toml out.squashfs -comp xz
#[test]
fn test_03() {
    let file = File::open("./lfs/test_03/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let pos_and_inodes = squashfs.inodes();
    let root_inode = squashfs.root_inode(&pos_and_inodes);
    let inodes = squashfs.discard_pos(&pos_and_inodes);
    let dir_blocks = squashfs.dir_blocks(&inodes);
    let fragments = squashfs.fragments();

    let (path, bytes) = squashfs.extract_file(
        "squashfs-deku",
        &dir_blocks,
        &inodes,
        &fragments,
        &root_inode,
    );
    let expected_bytes = fs::read("./lfs/test_03/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) =
        squashfs.extract_file("Cargo.toml", &dir_blocks, &inodes, &fragments, &root_inode);
    let expected_bytes = fs::read("./lfs/test_03/Cargo.toml").unwrap();
    assert_eq!(path.as_os_str(), "Cargo.toml");
    assert_eq!(bytes, expected_bytes);
}

#[test]
fn test_04() {
    let file = File::open("./lfs/test_04/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let pos_and_inodes = squashfs.inodes();
    let root_inode = squashfs.root_inode(&pos_and_inodes);
    let inodes = squashfs.discard_pos(&pos_and_inodes);
    let dir_blocks = squashfs.dir_blocks(&inodes);
    let fragments = squashfs.fragments();

    let (path, bytes) = squashfs.extract_file("01", &dir_blocks, &inodes, &fragments, &root_inode);
    let expected_bytes = fs::read("./lfs/test_04/testing/what/yikes/01").unwrap();
    assert_eq!(path.as_os_str(), "what/yikes/01");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("02", &dir_blocks, &inodes, &fragments, &root_inode);
    let expected_bytes = fs::read("./lfs/test_04/testing/what/yikes/02").unwrap();
    assert_eq!(path.as_os_str(), "what/yikes/02");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("03", &dir_blocks, &inodes, &fragments, &root_inode);
    let expected_bytes = fs::read("./lfs/test_04/testing/03").unwrap();
    assert_eq!(path.as_os_str(), "03");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("04", &dir_blocks, &inodes, &fragments, &root_inode);
    let expected_bytes = fs::read("./lfs/test_04/testing/what/04").unwrap();
    assert_eq!(path.as_os_str(), "what/04");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("05", &dir_blocks, &inodes, &fragments, &root_inode);
    let expected_bytes = fs::read("./lfs/test_04/testing/woah/05").unwrap();
    assert_eq!(path.as_os_str(), "woah/05");
    assert_eq!(bytes, expected_bytes);
}

#[test]
fn test_05() {
    let file = File::open("./lfs/test_05/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let pos_and_inodes = squashfs.inodes();
    let root_inode = squashfs.root_inode(&pos_and_inodes);
    let inodes = squashfs.discard_pos(&pos_and_inodes);
    let dir_blocks = squashfs.dir_blocks(&inodes);
    let fragments = squashfs.fragments();

    let (path, bytes) = squashfs.extract_file("d", &dir_blocks, &inodes, &fragments, &root_inode);
    let expected_bytes = fs::read("./lfs/test_05/a/b/c/d").unwrap();
    assert_eq!(path.as_os_str(), "b/c/d");
    assert_eq!(bytes, expected_bytes);
}
