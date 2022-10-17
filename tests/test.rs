use std::fs::{self, File};
use std::path::Path;

use squashfs_deku::compressor::Gzip;
use squashfs_deku::{CompressionOptions, Squashfs};
// use RUST_LOG tracing in test binaries
use test_log::test;
use tracing::info;

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2 -always-use-fragments
#[test]
fn test_00() {
    let file = File::open("./lfs/test_00/out.squashfs").unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    assert_eq!(
        squashfs.compression_options,
        Some(CompressionOptions::Gzip(Gzip {
            compression_level: 2,
            window_size: 15,
            strategies: 0
        }))
    );

    let (path, bytes) = squashfs.extract_file("squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_00/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let _expected_bytes = fs::read("./lfs/test_00/out.squashfs").unwrap();
    let _bytes = squashfs.to_bytes().unwrap();
    // TODO(#8) Don't assert the same bytes, as they won't be (it still works with unsquashfs)
    //assert_eq!(expected_bytes, bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2
#[test]
fn test_01() {
    let file = File::open("./lfs/test_01/out.squashfs").unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let (path, bytes) = squashfs.extract_file("squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_01/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files().unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_01/").join(path);
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }
    let _expected_bytes = fs::read("./lfs/test_00/out.squashfs").unwrap();
    let _bytes = squashfs.to_bytes().unwrap();
    // TODO(#8) Don't assert the same bytes, as they won't be (it still works with unsquashfs)
    //assert_eq!(expected_bytes, bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz
#[test]
fn test_02() {
    let file = File::open("./lfs/test_02/out.squashfs").unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let (path, bytes) = squashfs.extract_file("squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_02/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files().unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_02/").join(path);
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }

    let expected_bytes = fs::read("./lfs/test_02/out.squashfs").unwrap();
    let bytes = squashfs.to_bytes().unwrap();
    assert_eq!(expected_bytes, bytes);
}

/// mksquashfs ./target/release/squashfs-deku Cargo.toml out.squashfs -comp xz
#[test]
fn test_03() {
    let file = File::open("./lfs/test_03/out.squashfs").unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let (path, bytes) = squashfs.extract_file("squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_03/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("Cargo.toml").unwrap();
    let expected_bytes = fs::read("./lfs/test_03/Cargo.toml").unwrap();
    assert_eq!(path.as_os_str(), "Cargo.toml");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files().unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_03/").join(path);
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }

    let expected_bytes = fs::read("./lfs/test_03/out.squashfs").unwrap();
    let bytes = squashfs.to_bytes().unwrap();
    assert_eq!(expected_bytes, bytes);
}

#[test]
fn test_04() {
    let file = File::open("./lfs/test_04/out.squashfs").unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let (path, bytes) = squashfs.extract_file("01").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/what/yikes/01").unwrap();
    assert_eq!(path.as_os_str(), "what/yikes/01");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("02").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/what/yikes/02").unwrap();
    assert_eq!(path.as_os_str(), "what/yikes/02");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("03").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/03").unwrap();
    assert_eq!(path.as_os_str(), "03");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("04").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/what/04").unwrap();
    assert_eq!(path.as_os_str(), "what/04");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file("05").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/woah/05").unwrap();
    assert_eq!(path.as_os_str(), "woah/05");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files().unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_04/testing/").join(path);
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }

    let expected_bytes = fs::read("./lfs/test_04/out.squashfs").unwrap();
    let bytes = squashfs.to_bytes().unwrap();
    assert_eq!(expected_bytes, bytes);
}

#[test]
fn test_05() {
    let file = File::open("./lfs/test_05/out.squashfs").unwrap();
    info!("{file:?}");
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let (path, bytes) = squashfs.extract_file("d").unwrap();
    let expected_bytes = fs::read("./lfs/test_05/a/b/c/d").unwrap();
    assert_eq!(path.as_os_str(), "b/c/d");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files().unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_05/a/").join(path);
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }

    let expected_bytes = fs::read("./lfs/test_05/out.squashfs").unwrap();
    let bytes = squashfs.to_bytes().unwrap();
    assert_eq!(expected_bytes, bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -always-use-fragments
#[test]
fn test_06() {
    let file = File::open("./lfs/test_06/out.squashfs").unwrap();
    info!("{file:?}");
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let (path, bytes) = squashfs.extract_file("squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_06/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let _expected_bytes = fs::read("./lfs/test_06/out.squashfs").unwrap();
    let _bytes = squashfs.to_bytes().unwrap();
    // TODO(#8) Don't assert the same bytes, as they won't be (it still works with unsquashfs)
    //assert_eq!(expected_bytes, bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip
#[test]
fn test_07() {
    let file = File::open("./lfs/test_07/out.squashfs").unwrap();
    info!("{file:?}");
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let (path, bytes) = squashfs.extract_file("squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_07/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let _expected_bytes = fs::read("./lfs/test_07/out.squashfs").unwrap();
    let _bytes = squashfs.to_bytes().unwrap();
    // TODO(#8) Don't assert the same bytes, as they won't be (it still works with unsquashfs)
    //assert_eq!(expected_bytes, bytes);
}

// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz -Xbcj arm
#[test]
fn test_08() {
    let file = File::open("./lfs/test_08/out.squashfs").unwrap();
    info!("{file:?}");
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let (path, bytes) = squashfs.extract_file("squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_08/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let _expected_bytes = fs::read("./lfs/test_08/out.squashfs").unwrap();
    let _bytes = squashfs.to_bytes().unwrap();
    assert_eq!(expected_bytes, bytes);
}
