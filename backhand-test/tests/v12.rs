//! Tests for SquashFS v1 and v2
//!
//! The images come from `backhand-test/generate-v1-v2-images.sh`, which builds
//! them with the original tools. Each packs the same tree, so one set of
//! expectations covers every version and endianness.

#![cfg(any(feature = "v1", feature = "v2"))]

mod common;

use std::fs::File;
use std::io::{BufReader, Read};

use backhand::kind::Kind;
use backhand::v12::filesystem::reader::FilesystemReader;
use common::test_bin_unsquashfs_with_kind;
use test_log::test;
use tracing::info;

/// Every path the generated images hold, in order
const EXPECTED_PATHS: &[&str] = &[
    "/",
    "/a.txt",
    "/big.bin",
    "/dir_a",
    "/dir_a/nested",
    "/dir_a/nested/second.txt",
    "/dir_a/nested/zeros.bin",
    "/dir_a/small.txt",
    "/dir_b",
    "/dir_b/a_fifo",
    "/dir_b/link_to_a",
];

/// The contents of every file except `big.bin`, whose bytes are random
const EXPECTED_FILES: &[(&str, &str)] = &[
    ("/a.txt", "hello from the backhand v1/v2 test image\n"),
    ("/dir_a/nested/second.txt", "second file, to exercise duplicate detection\n"),
    ("/dir_a/small.txt", "small\n"),
];

/// Read the image, check its tree and contents, then extract it with our
/// unsquashfs so the CLI path is covered too.
fn read_and_extract(asset: &str, path: &str, kind: Kind, kind_str: &str) {
    common::download_asset(asset);

    let file = BufReader::new(File::open(path).unwrap());
    info!("calling from_reader");
    let fs = FilesystemReader::from_reader_with_offset_and_kind(file, 0, kind).unwrap();

    let paths: Vec<String> = fs.files().map(|node| node.fullpath.display().to_string()).collect();
    assert_eq!(paths, EXPECTED_PATHS, "{path}: tree differs");

    for (wanted_path, wanted) in EXPECTED_FILES {
        let node = fs
            .files()
            .find(|node| node.fullpath.to_str() == Some(*wanted_path))
            .unwrap_or_else(|| panic!("{path}: {wanted_path} is missing"));
        let backhand::v12::filesystem::node::InnerNode::File(inner) = &node.inner else {
            panic!("{path}: {wanted_path} is not a file");
        };
        let mut bytes = vec![];
        fs.file(inner).reader().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, wanted.as_bytes(), "{path}: {wanted_path} differs");
    }

    // A 300 KiB random file, which is what covers the multi-block path.
    let node = fs.files().find(|node| node.fullpath.to_str() == Some("/big.bin")).unwrap();
    let backhand::v12::filesystem::node::InnerNode::File(inner) = &node.inner else {
        panic!("{path}: big.bin is not a file");
    };
    let mut bytes = vec![];
    fs.file(inner).reader().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes.len(), 300_000, "{path}: big.bin is the wrong length");

    test_bin_unsquashfs_with_kind(path, Some(0), true, false, Some(kind_str.to_string()));
}

#[test]
#[cfg(feature = "v1")]
fn test_v1_le() {
    read_and_extract(
        "v1_le",
        "test-assets/squashfs_v1_le.sqfs",
        Kind::from_const(backhand::kind::LE_V1_0).unwrap(),
        "le_v1_0",
    );
}

#[test]
#[cfg(feature = "v2")]
fn test_v2_le() {
    read_and_extract(
        "v2_le",
        "test-assets/squashfs_v2_le.sqfs",
        Kind::from_const(backhand::kind::LE_V2_0).unwrap(),
        "le_v2_0",
    );
}

#[test]
#[cfg(feature = "v2")]
fn test_v2_be() {
    read_and_extract(
        "v2_be",
        "test-assets/squashfs_v2_be.sqfs",
        Kind::from_const(backhand::kind::BE_V2_0).unwrap(),
        "be_v2_0",
    );
}

/// Minor version 0 differs from 1 only in whether the writer sorted the
/// directory entries, so one kind reads both.
#[test]
#[cfg(feature = "v2")]
fn test_v2_0_le() {
    read_and_extract(
        "v2_0_le",
        "test-assets/squashfs_v2_0_le.sqfs",
        Kind::from_const(backhand::kind::LE_V2_0).unwrap(),
        "le_v2_0",
    );
}

#[test]
#[cfg(feature = "v2")]
fn test_v2_0_be() {
    read_and_extract(
        "v2_0_be",
        "test-assets/squashfs_v2_0_be.sqfs",
        Kind::from_const(backhand::kind::BE_V2_0).unwrap(),
        "be_v2_0",
    );
}

#[test]
#[cfg(feature = "v2_lzma")]
fn test_v2_lzma_le() {
    read_and_extract(
        "v2_lzma_le",
        "test-assets/squashfs_v2_lzma_le.sqfs",
        Kind::from_const(backhand::kind::AVM_LE_V2_LZMA).unwrap(),
        "avm_le_v2_lzma",
    );
}

#[test]
#[cfg(feature = "v2_lzma")]
fn test_v2_lzma_be() {
    read_and_extract(
        "v2_lzma_be",
        "test-assets/squashfs_v2_lzma_be.sqfs",
        Kind::from_const(backhand::kind::AVM_BE_V2_LZMA).unwrap(),
        "avm_be_v2_lzma",
    );
}

/// A kind of the wrong version must fail, because the CLI finds a kind by
/// trying each in turn and taking the first that parses. v1, v2 and v3 share a
/// magic and a superblock prefix, so only the version check separates them.
#[test]
#[cfg(all(feature = "v1", feature = "v2"))]
fn test_a_kind_of_the_wrong_version_is_rejected() {
    common::download_assets(&["v1_le", "v2_le"]);

    let file = BufReader::new(File::open("test-assets/squashfs_v2_le.sqfs").unwrap());
    let kind = Kind::from_const(backhand::kind::LE_V1_0).unwrap();
    assert!(FilesystemReader::from_reader_with_offset_and_kind(file, 0, kind).is_err());

    let file = BufReader::new(File::open("test-assets/squashfs_v1_le.sqfs").unwrap());
    let kind = Kind::from_const(backhand::kind::LE_V2_0).unwrap();
    assert!(FilesystemReader::from_reader_with_offset_and_kind(file, 0, kind).is_err());
}

/// The minor version is the only record of which compressor a v2 image uses, so
/// the superblock check has to catch a kind that disagrees.
#[test]
#[cfg(feature = "v2_lzma")]
fn test_a_gzip_kind_will_not_open_an_lzma_image() {
    common::download_assets(&["v2_le", "v2_lzma_le"]);

    let file = BufReader::new(File::open("test-assets/squashfs_v2_lzma_le.sqfs").unwrap());
    let kind = Kind::from_const(backhand::kind::LE_V2_0).unwrap();
    assert!(FilesystemReader::from_reader_with_offset_and_kind(file, 0, kind).is_err());

    let file = BufReader::new(File::open("test-assets/squashfs_v2_le.sqfs").unwrap());
    let kind = Kind::from_const(backhand::kind::AVM_LE_V2_LZMA).unwrap();
    assert!(FilesystemReader::from_reader_with_offset_and_kind(file, 0, kind).is_err());
}
