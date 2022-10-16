use std::fs::{self, File};
use std::path::Path;

use squashfs_deku::Squashfs;

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2 -always-use-fragments
#[test]
fn test_00() {
    let file = File::open("./lfs/test_00/out.squashfs").unwrap();
    let squashfs = Squashfs::from_reader(file).unwrap();

    let (path, bytes) = squashfs.extract_file(&squashfs, "squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_00/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2
#[test]
fn test_01() {
    let file = File::open("./lfs/test_01/out.squashfs").unwrap();
    let squashfs = Squashfs::from_reader(file).unwrap();

    let (path, bytes) = squashfs.extract_file(&squashfs, "squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_01/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files(&squashfs).unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_01/").join(path);
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz
#[test]
fn test_02() {
    let file = File::open("./lfs/test_02/out.squashfs").unwrap();
    let squashfs = Squashfs::from_reader(file).unwrap();

    let (path, bytes) = squashfs.extract_file(&squashfs, "squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_02/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files(&squashfs).unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_02/").join(path);
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }
}

/// mksquashfs ./target/release/squashfs-deku Cargo.toml out.squashfs -comp xz
#[test]
fn test_03() {
    let file = File::open("./lfs/test_03/out.squashfs").unwrap();
    let squashfs = Squashfs::from_reader(file).unwrap();

    let (path, bytes) = squashfs.extract_file(&squashfs, "squashfs-deku").unwrap();
    let expected_bytes = fs::read("./lfs/test_03/squashfs-deku").unwrap();
    assert_eq!(path.as_os_str(), "squashfs-deku");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file(&squashfs, "Cargo.toml").unwrap();
    let expected_bytes = fs::read("./lfs/test_03/Cargo.toml").unwrap();
    assert_eq!(path.as_os_str(), "Cargo.toml");
    assert_eq!(bytes, expected_bytes);

    //let path_bytes = io.extract_all_files(&squashfs).unwrap();
    //for (path, bytes) in path_bytes {
    //    let filepath = Path::new("./lfs/test_03/").join(path);
    //    let expected_bytes = fs::read(filepath).unwrap();
    //    assert_eq!(bytes, expected_bytes);
    //}
}

#[test]
fn test_04() {
    let file = File::open("./lfs/test_04/out.squashfs").unwrap();
    let squashfs = Squashfs::from_reader(file).unwrap();

    let (path, bytes) = squashfs.extract_file(&squashfs, "01").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/what/yikes/01").unwrap();
    assert_eq!(path.as_os_str(), "what/yikes/01");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file(&squashfs, "02").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/what/yikes/02").unwrap();
    assert_eq!(path.as_os_str(), "what/yikes/02");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file(&squashfs, "03").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/03").unwrap();
    assert_eq!(path.as_os_str(), "03");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file(&squashfs, "04").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/what/04").unwrap();
    assert_eq!(path.as_os_str(), "what/04");
    assert_eq!(bytes, expected_bytes);

    let (path, bytes) = squashfs.extract_file(&squashfs, "05").unwrap();
    let expected_bytes = fs::read("./lfs/test_04/testing/woah/05").unwrap();
    assert_eq!(path.as_os_str(), "woah/05");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files(&squashfs).unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_04/testing/").join(path);
        println!("{}", filepath.display());
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }
}

#[test]
fn test_05() {
    let file = File::open("./lfs/test_05/out.squashfs").unwrap();
    let squashfs = Squashfs::from_reader(file).unwrap();

    let (path, bytes) = squashfs.extract_file(&squashfs, "d").unwrap();
    let expected_bytes = fs::read("./lfs/test_05/a/b/c/d").unwrap();
    assert_eq!(path.as_os_str(), "b/c/d");
    assert_eq!(bytes, expected_bytes);

    let path_bytes = squashfs.extract_all_files(&squashfs).unwrap();
    for (path, bytes) in path_bytes {
        let filepath = Path::new("./lfs/test_05/a/").join(path);
        let expected_bytes = fs::read(filepath).unwrap();
        assert_eq!(bytes, expected_bytes);
    }
}
