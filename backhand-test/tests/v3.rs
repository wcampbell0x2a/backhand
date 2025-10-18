#![cfg(feature = "v3")]

mod common;
use std::fs::File;
use std::io::BufReader;

use backhand::kind::{Kind, BE_V3_0, LE_V3_0};
use backhand::v3::filesystem::reader::FilesystemReader;
use common::test_bin_unsquashfs_with_kind;
use test_assets_ureq::TestAssetDef;
use test_log::test;
use tracing::info;

fn only_read(
    kind: Kind,
    assets_defs: &[TestAssetDef],
    filepath: &str,
    test_path: &str,
    offset: u64,
    kind_str: &str,
) {
    common::download_backoff(assets_defs, test_path);

    let og_path = format!("{test_path}/{filepath}");
    let file = BufReader::new(File::open(&og_path).unwrap());
    info!("calling from_reader");
    let _ = FilesystemReader::from_reader_with_offset_and_kind(file, offset, kind).unwrap();

    test_bin_unsquashfs_with_kind(&og_path, Some(offset), true, false, Some(kind_str.to_string()));
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_be() {
    const FILE_NAME: &str = "squashfs_v3_be.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "4e9493d9c6f868005dea8f11992129c3399e0bcb8f3a966c750cb925989ca97c".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_v3_be";
    only_read(Kind::from_const(BE_V3_0).unwrap(), &asset_defs, FILE_NAME, TEST_PATH, 0, "be_v3_0");
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_le() {
    const FILE_NAME: &str = "squashfs_v3_le.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "0161351caec8e9da6e3e5ac7b046fd11d832efb18eb09e33011e6d19d50cd1f7".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_v3_le";
    only_read(Kind::from_const(LE_V3_0).unwrap(), &asset_defs, FILE_NAME, TEST_PATH, 0, "le_v3_0");
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_be_lzma() {
    use backhand::kind::BE_V3_0_LZMA;

    const FILE_NAME: &str = "squashfs_v3_be.lzma.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "71971c7a05d2d0b44bc976bc168f7b9134af8dc9fec83553889a9b5473469495".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_v3_be_lzma";
    only_read(
        Kind::from_const(BE_V3_0_LZMA).unwrap(),
        &asset_defs,
        FILE_NAME,
        TEST_PATH,
        0,
        "be_v3_0_lzma",
    );
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_le_lzma() {
    use backhand::kind::LE_V3_0_LZMA;

    const FILE_NAME: &str = "squashfs_v3_le.lzma.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "2ac15e5c993f330854497b2fd032d2e87873a288fab6da2fe72106bb40b43124".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_v3_le_lzma";
    only_read(
        Kind::from_const(LE_V3_0_LZMA).unwrap(),
        &asset_defs,
        FILE_NAME,
        TEST_PATH,
        0,
        "le_v3_0_lzma",
    );
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_le_more() {
    use backhand::kind::LE_V3_0;

    const FILE_NAME: &str = "test_v3.sqfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "6934ad5af828172ff10a6c0996f0d415df894c317d3a8cdee042005ba5243802".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_v3_more";
    only_read(Kind::from_const(LE_V3_0).unwrap(), &asset_defs, FILE_NAME, TEST_PATH, 0, "le_v3_0");
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_netgear() {
    use backhand::kind::NETGEAR_BE_V3_0_LZMA_STANDARD;

    const FILE_NAME: &str = "netgear_v3.sqsh";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "a89cdf1cc45f99bd938b7e1d9b63b56f0b16edf803307956d1ba030c62a919ac".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_v3_netgear";
    only_read(
        Kind::from_const(NETGEAR_BE_V3_0_LZMA_STANDARD).unwrap(),
        &asset_defs,
        FILE_NAME,
        TEST_PATH,
        0,
        "netgear_be_v3_0_lzma_standard",
    );
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_more_deep_directory_structure() {
    use backhand::kind::LE_V3_0;

    const FILE_NAME: &str = "test_v3.sqfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "6934ad5af828172ff10a6c0996f0d415df894c317d3a8cdee042005ba5243802".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_v3_more";
    only_read(Kind::from_const(LE_V3_0).unwrap(), &asset_defs, FILE_NAME, TEST_PATH, 0, "le_v3_0");
}

#[test]
#[cfg(feature = "v3")]
fn test_v3_many_dirs() {
    use backhand::kind::LE_V3_0;

    const FILE_NAME: &str = "many_dirs_v3.sqsh";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "00d9b4d0dbe98fdb57997da7481b1a6ffaa812aa536d3d8e1c72a8c3eb71e6c4".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/many_dirs_v3";
    only_read(Kind::from_const(LE_V3_0).unwrap(), &asset_defs, FILE_NAME, TEST_PATH, 0, "le_v3_0");
}

#[test]
#[cfg(feature = "v3_lzma")]
fn test_v3_openwrt() {
    use backhand::kind::BE_V3_0_LZMA;

    const FILE_NAME: &str = "openwrt-ar71xx-root.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "ac202e0676872deddaafc2ed5f26349ab51f3f34c12500cf896f615ad8ee00ca".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/openwrt-ar71xx-root";
    only_read(
        Kind::from_const(BE_V3_0_LZMA).unwrap(),
        &asset_defs,
        FILE_NAME,
        TEST_PATH,
        0,
        "be_v3_0_lzma",
    );
}
