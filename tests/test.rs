use std::fs::{self, File};

use squashfs_deku::Squashfs;

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2 -always-use-fragments
#[test]
fn test_00() {
    let file = File::open("./lfs/test_00/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let dirs = squashfs.dirs();
    let inodes = squashfs.inodes();
    let fragments = squashfs.fragments();

    let bytes = squashfs.extract_file("squashfs-deku", &dirs, &inodes, &fragments);
    let expected_bytes = fs::read("./lfs/test_00/squashfs-deku").unwrap();
    assert_eq!(bytes, expected_bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2
#[test]
fn test_01() {
    let file = File::open("./lfs/test_01/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let dirs = squashfs.dirs();
    let inodes = squashfs.inodes();
    let fragments = squashfs.fragments();

    let bytes = squashfs.extract_file("squashfs-deku", &dirs, &inodes, &fragments);
    let expected_bytes = fs::read("./lfs/test_01/squashfs-deku").unwrap();
    assert_eq!(bytes, expected_bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz
#[test]
fn test_02() {
    let file = File::open("./lfs/test_02/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let dirs = squashfs.dirs();
    let inodes = squashfs.inodes();
    let fragments = squashfs.fragments();

    let bytes = squashfs.extract_file("squashfs-deku", &dirs, &inodes, &fragments);
    let expected_bytes = fs::read("./lfs/test_02/squashfs-deku").unwrap();
    assert_eq!(bytes, expected_bytes);
}

/// mksquashfs ./target/release/squashfs-deku Cargo.toml out.squashfs -comp xz
#[test]
fn test_03() {
    let file = File::open("./lfs/test_03/out.squashfs").unwrap();
    let mut squashfs = Squashfs::from_reader(file);

    let dirs = squashfs.dirs();
    let inodes = squashfs.inodes();
    let fragments = squashfs.fragments();

    let bytes = squashfs.extract_file("squashfs-deku", &dirs, &inodes, &fragments);
    let expected_bytes = fs::read("./lfs/test_03/squashfs-deku").unwrap();
    assert_eq!(bytes, expected_bytes);

    let bytes = squashfs.extract_file("Cargo.toml", &dirs, &inodes, &fragments);
    let expected_bytes = fs::read("./lfs/test_03/Cargo.toml").unwrap();
    assert_eq!(bytes, expected_bytes);
}
