mod common;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;

use assert_cmd::prelude::*;
use assert_cmd::Command;
use backhand::kind::{Kind, BE_V3_0, LE_V3_0};
use backhand::v3::filesystem::reader::FilesystemReader;
use common::{test_bin_unsquashfs, test_squashfs_tools_unsquashfs};
use tempfile::tempdir;
use test_assets_ureq::TestAssetDef;
use test_log::test;
use tracing::{info, trace};

fn only_read_be(assets_defs: &[TestAssetDef], filepath: &str, test_path: &str, offset: u64) {
    common::download_backoff(assets_defs, test_path);

    let og_path = format!("{test_path}/{filepath}");
    let file = BufReader::new(File::open(&og_path).unwrap());
    info!("calling from_reader");
    let _ = FilesystemReader::from_reader_with_offset_and_kind(
        file,
        offset,
        Kind::from_const(BE_V3_0).unwrap(),
    )
    .unwrap();

    // TODO: this should still check our own unsquashfs
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
    only_read_be(&asset_defs, FILE_NAME, TEST_PATH, 0);
}

fn only_read_le(assets_defs: &[TestAssetDef], filepath: &str, test_path: &str, offset: u64) {
    common::download_backoff(assets_defs, test_path);

    let og_path = format!("{test_path}/{filepath}");
    let file = BufReader::new(File::open(&og_path).unwrap());
    info!("calling from_reader");
    let _ = FilesystemReader::from_reader_with_offset_and_kind(
        file,
        offset,
        Kind::from_const(LE_V3_0).unwrap(),
    )
    .unwrap();

    // TODO: this should still check our own unsquashfs
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
    only_read_le(&asset_defs, FILE_NAME, TEST_PATH, 0);
}
