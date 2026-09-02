#![cfg(feature = "v3")]

mod common;
use std::fs::File;
use std::io::BufReader;

use backhand::kind::{BE_V3_0, Kind, LE_V3_0};
use backhand::v3::filesystem::reader::FilesystemReader;
use common::test_bin_unsquashfs_with_kind;
use test_log::test;
use tracing::info;

fn only_read(kind: Kind, og_path: &str, offset: u64, kind_str: &str) {
    let file = BufReader::new(File::open(og_path).unwrap());
    info!("calling from_reader");
    let _ = FilesystemReader::from_reader_with_offset_and_kind(file, offset, kind).unwrap();

    test_bin_unsquashfs_with_kind(og_path, Some(offset), true, false, Some(kind_str.to_string()));
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_be() {
    common::download_asset("v3_be");
    only_read(
        Kind::from_const(BE_V3_0).unwrap(),
        "test-assets/test_v3_be/squashfs_v3_be.bin",
        0,
        "be_v3_0",
    );
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_le() {
    common::download_asset("v3_le");
    only_read(
        Kind::from_const(LE_V3_0).unwrap(),
        "test-assets/test_v3_le/squashfs_v3_le.bin",
        0,
        "le_v3_0",
    );
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_be_lzma() {
    use backhand::kind::BE_V3_0_LZMA;

    common::download_asset("v3_be_lzma");
    only_read(
        Kind::from_const(BE_V3_0_LZMA).unwrap(),
        "test-assets/test_v3_be_lzma/squashfs_v3_be.lzma.bin",
        0,
        "be_v3_0_lzma",
    );
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_le_lzma() {
    use backhand::kind::LE_V3_0_LZMA;

    common::download_asset("v3_le_lzma");
    only_read(
        Kind::from_const(LE_V3_0_LZMA).unwrap(),
        "test-assets/test_v3_le_lzma/squashfs_v3_le.lzma.bin",
        0,
        "le_v3_0_lzma",
    );
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_le_more() {
    use backhand::kind::LE_V3_0;

    common::download_asset("v3_le_more");
    only_read(
        Kind::from_const(LE_V3_0).unwrap(),
        "test-assets/test_v3_more/test_v3.sqfs",
        0,
        "le_v3_0",
    );
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_netgear() {
    use backhand::kind::NETGEAR_BE_V3_0_LZMA_STANDARD;

    common::download_asset("v3_netgear");
    only_read(
        Kind::from_const(NETGEAR_BE_V3_0_LZMA_STANDARD).unwrap(),
        "test-assets/test_v3_netgear/netgear_v3.sqsh",
        0,
        "netgear_be_v3_0_lzma_standard",
    );
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_more_deep_directory_structure() {
    use backhand::kind::LE_V3_0;

    common::download_asset("v3_le_more");
    only_read(
        Kind::from_const(LE_V3_0).unwrap(),
        "test-assets/test_v3_more/test_v3.sqfs",
        0,
        "le_v3_0",
    );
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_many_dirs() {
    use backhand::kind::LE_V3_0;

    common::download_asset("v3_many_dirs");
    only_read(
        Kind::from_const(LE_V3_0).unwrap(),
        "test-assets/many_dirs_v3/many_dirs_v3.sqsh",
        0,
        "le_v3_0",
    );
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_openwrt() {
    use backhand::kind::BE_V3_0_LZMA;

    common::download_asset("v3_openwrt");
    only_read(
        Kind::from_const(BE_V3_0_LZMA).unwrap(),
        "test-assets/openwrt-ar71xx-root/openwrt-ar71xx-root.squashfs",
        0,
        "be_v3_0_lzma",
    );
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_lzma_swap() {
    use backhand::kind::LE_V3_1_LZMA_SWAP;

    common::download_asset("v3_le_lzma_swap");
    only_read(
        Kind::from_const(LE_V3_1_LZMA_SWAP).unwrap(),
        "test-assets/squashfs_v3_le_lzma_swap.sqfs",
        0,
        "le_v3_1_lzma_swap",
    );
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_lzma_swap_standard() {
    use backhand::kind::LE_V3_0_LZMA_SWAP_STANDARD;

    common::download_asset("WNR1000v3");
    only_read(
        Kind::from_const(LE_V3_0_LZMA_SWAP_STANDARD).unwrap(),
        "test-assets/WNR1000v3.sqfs",
        0,
        "le_v3_0_lzma_swap_standard",
    );
}

/// The plain entry point must open a v3 image. It defaulted to a v4 kind once,
/// which no v3 image can match, so this path never worked.
#[test]
#[cfg(feature = "v3")]
fn test_v3_from_reader_uses_a_v3_kind_by_default() {
    common::download_asset("v3_le");

    let file = BufReader::new(File::open("test-assets/test_v3_le/squashfs_v3_le.bin").unwrap());
    let fs = FilesystemReader::from_reader(file).unwrap();
    assert!(fs.files().count() > 1);
}

/// Reading through the version-neutral path must give the same tree as the v3
/// one. It used to panic, because that path left the uid table empty and the
/// reader unwrapped it.
#[test]
#[cfg(feature = "v3")]
fn test_v3_reads_the_same_through_the_version_neutral_path() {
    common::download_asset("v3_le");
    let path = "test-assets/test_v3_le/squashfs_v3_le.bin";

    let file = BufReader::new(File::open(path).unwrap());
    let direct = FilesystemReader::from_reader_with_offset_and_kind(
        file,
        0,
        Kind::from_const(LE_V3_0).unwrap(),
    )
    .unwrap();
    let direct: Vec<String> =
        direct.files().map(|node| node.fullpath.display().to_string()).collect();

    let file = BufReader::new(File::open(path).unwrap());
    let generic =
        backhand::create_squashfs_from_kind(file, 0, Kind::from_const(LE_V3_0).unwrap()).unwrap();
    let generic: Vec<String> =
        generic.files().map(|node| node.fullpath.display().to_string()).collect();

    assert_eq!(direct, generic);
}
