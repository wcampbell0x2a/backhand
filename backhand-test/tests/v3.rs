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
    let file = BufReader::new(File::open(&og_path).unwrap());
    info!("calling from_reader");
    let _ = FilesystemReader::from_reader_with_offset_and_kind(file, offset, kind).unwrap();

    test_bin_unsquashfs_with_kind(&og_path, Some(offset), true, false, Some(kind_str.to_string()));
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
