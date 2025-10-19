use std::fs::File;
use std::io::{BufReader, Cursor};
use std::process::Command;
use std::time::Duration;

use assert_cmd::prelude::*;
use backhand::{FilesystemReader, FilesystemWriter};
use criterion::*;
use tempfile::tempdir;
use test_assets_ureq::TestAssetDef;
use test_assets_ureq::dl_test_files_backoff;

fn read_write(file: File, offset: u64) {
    let file = BufReader::new(file);
    let og_filesystem = FilesystemReader::from_reader_with_offset(file, offset).unwrap();
    let mut new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();

    // convert to bytes
    let mut output = Cursor::new(vec![]);
    std::hint::black_box(new_filesystem.write(&mut output).unwrap());
}

fn read(file: File, offset: u64) {
    let file = BufReader::new(file);
    std::hint::black_box(FilesystemReader::from_reader_with_offset(file, offset).unwrap());
}

pub fn bench_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_read");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);

    const FILE_NAME_00: &str =
        "openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME_00.to_string(),
        hash: "9608a6cb558f1a4aa9659257f7c0b401f94343d10ec6e964fc4a452b4f91bea4".to_string(),
        url: format!(
            "https://downloads.openwrt.org/releases/22.03.2/targets/ipq40xx/generic/{FILE_NAME_00}"
        ),
    }];
    const TEST_PATH_00: &str = "../backhand-cli/test-assets/test_openwrt_netgear_ex6100v2";
    dl_test_files_backoff(&asset_defs, TEST_PATH_00, true, Duration::from_secs(1)).unwrap();
    let og_path = format!("{TEST_PATH_00}/{FILE_NAME_00}");
    group.bench_function("netgear_ax6100v2", |b| {
        b.iter(|| {
            let file = File::open(&og_path).unwrap();
            read_write(file, 0x2c0080)
        })
    });

    const FILE_NAME: &str = "img-1571203182_vol-ubi_rootfs.ubifs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "e6adbea10615a8ed9f88e403e2478010696f421f4d69a790d37d97fe8921aa81".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/test_tplink1800/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_tplink_ax1800";
    dl_test_files_backoff(&asset_defs, TEST_PATH, true, Duration::from_secs(1)).unwrap();
    let og_path = format!("{TEST_PATH}/{FILE_NAME}");
    group.bench_function("tplink_ax1800", |b| {
        b.iter(|| {
            let file = File::open(&og_path).unwrap();
            read_write(file, 0)
        })
    });

    group.finish();
}

pub fn bench_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("only_read");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);

    const FILE_NAME_00: &str =
        "openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME_00.to_string(),
        hash: "9608a6cb558f1a4aa9659257f7c0b401f94343d10ec6e964fc4a452b4f91bea4".to_string(),
        url: format!(
            "https://downloads.openwrt.org/releases/22.03.2/targets/ipq40xx/generic/{FILE_NAME_00}"
        ),
    }];
    const TEST_PATH_00: &str = "../backhand-cli/test-assets/test_openwrt_netgear_ex6100v2";
    dl_test_files_backoff(&asset_defs, TEST_PATH_00, true, Duration::from_secs(1)).unwrap();
    let og_path = format!("{TEST_PATH_00}/{FILE_NAME_00}");
    group.bench_function("netgear_ax6100v2", |b| {
        b.iter(|| {
            let file = File::open(&og_path).unwrap();
            read(file, 0x2c0080)
        })
    });

    const FILE_NAME_01: &str = "img-1571203182_vol-ubi_rootfs.ubifs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME_01.to_string(),
        hash: "e6adbea10615a8ed9f88e403e2478010696f421f4d69a790d37d97fe8921aa81".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/test_tplink1800/{FILE_NAME_01}"),
    }];
    const TEST_PATH_01: &str = "test-assets/test_tplink_ax1800";
    dl_test_files_backoff(&asset_defs, TEST_PATH_01, true, Duration::from_secs(1)).unwrap();
    let og_path = format!("{TEST_PATH_01}/{FILE_NAME_01}");
    group.bench_function("tplink_ax1800", |b| {
        b.iter(|| {
            let file = File::open(&og_path).unwrap();
            read(file, 0)
        })
    });

    group.finish();
}

pub fn bench_unsquashfs_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("unsquashfs");

    const FILE_NAME: &str = "openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img";

    let asset_defs = &[TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "9608a6cb558f1a4aa9659257f7c0b401f94343d10ec6e964fc4a452b4f91bea4".to_string(),
        url: format!(
            "https://downloads.openwrt.org/releases/22.03.2/targets/ipq40xx/generic/{FILE_NAME}"
        ),
    }];
    // Local, because we run unsquashfs
    const TEST_PATH: &str = "test-assets/test_openwrt_netgear_ex6100v2";
    dl_test_files_backoff(asset_defs, TEST_PATH, true, Duration::from_secs(1)).unwrap();
    let path = format!("{TEST_PATH}/{FILE_NAME}");

    let tmp_dir = tempdir().unwrap();

    group.bench_function("full", |b| {
        b.iter(|| {
            let cmd = Command::new(assert_cmd::cargo::cargo_bin("unsquashfs-backhand"))
                .env("RUST_LOG", "none")
                .args([
                    "--auto-offset",
                    "--kind",
                    "le_v4_0",
                    "--quiet",
                    "-d",
                    tmp_dir.path().join("squashfs-out").to_str().unwrap(),
                    &path,
                ])
                .unwrap();
            cmd.assert().code(&[0] as &[i32]);
        })
    });

    // 38 nodes
    group.bench_function("full-path-filter", |b| {
        b.iter(|| {
            let cmd = Command::new(assert_cmd::cargo::cargo_bin("unsquashfs-backhand"))
                .env("RUST_LOG", "none")
                .args([
                    "--auto-offset",
                    "--kind",
                    "le_v4_0",
                    "--quiet",
                    "--path-filter",
                    "/usr/sbin/",
                    "-d",
                    tmp_dir.path().join("squashfs-out").to_str().unwrap(),
                    &path,
                ])
                .unwrap();
            cmd.assert().code(&[0] as &[i32]);
        })
    });

    group.bench_function("list", |b| {
        b.iter(|| {
            let cmd = Command::new(assert_cmd::cargo::cargo_bin("unsquashfs-backhand"))
                .env("RUST_LOG", "none")
                .args(["--auto-offset", "--kind", "le_v4_0", "-l", &path])
                .unwrap();
            cmd.assert().code(&[0] as &[i32]);
        })
    });

    group.bench_function("list-path-filter", |b| {
        b.iter(|| {
            let cmd = Command::new(assert_cmd::cargo::cargo_bin("unsquashfs-backhand"))
                .env("RUST_LOG", "none")
                .args([
                    "--auto-offset",
                    "--kind",
                    "le_v4_0",
                    "-l",
                    "--path-filter",
                    "/usr/sbin/",
                    &path,
                ])
                .unwrap();
            cmd.assert().code(&[0] as &[i32]);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_read_write, bench_read, bench_unsquashfs_extract);
criterion_main!(benches);
