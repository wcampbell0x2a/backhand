use std::fs::{self, File};
use std::io::{BufReader, Cursor};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use assert_cmd::prelude::*;
use backhand::{FilesystemReader, FilesystemWriter};
use criterion::*;
use tempfile::tempdir;
use test_assets_ureq::{TestAsset, dl_test_files_backoff};

static TEST_ASSETS: OnceLock<TestAsset> = OnceLock::new();

/// Get the parsed test assets from the TOML file
fn get_test_assets() -> &'static TestAsset {
    TEST_ASSETS.get_or_init(|| {
        let mut config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config_path.push("../test-assets.toml");
        let file_content = std::fs::read_to_string(config_path).unwrap();
        toml::from_str(&file_content).expect("Failed to parse test-assets.toml")
    })
}

/// Download a specific test asset by key
fn download_asset(asset_key: &str) -> String {
    let assets = get_test_assets();
    let asset = assets
        .assets
        .get(asset_key)
        .unwrap_or_else(|| panic!("Asset '{}' not found in test-assets.toml", asset_key));

    // Download to current directory (tests run from workspace root)
    let _ = dl_test_files_backoff(&[asset.clone()], ".", Duration::from_secs(60));

    asset.filepath.clone()
}

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

    let netgear_path = download_asset("netgear_ex6100v2");
    group.bench_function("netgear_ax6100v2", |b| {
        b.iter(|| {
            let file = File::open(&netgear_path).unwrap();
            read_write(file, 0x2c0080)
        })
    });

    let tplink_path = download_asset("tplink_ax1800");
    group.bench_function("tplink_ax1800", |b| {
        b.iter(|| {
            let file = File::open(&tplink_path).unwrap();
            read_write(file, 0)
        })
    });

    group.finish();
}

pub fn bench_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("only_read");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);

    let netgear_path = download_asset("netgear_ex6100v2");
    group.bench_function("netgear_ax6100v2", |b| {
        b.iter(|| {
            let file = File::open(&netgear_path).unwrap();
            read(file, 0x2c0080)
        })
    });

    let tplink_path = download_asset("tplink_ax1800");
    group.bench_function("tplink_ax1800", |b| {
        b.iter(|| {
            let file = File::open(&tplink_path).unwrap();
            read(file, 0)
        })
    });

    group.finish();
}

pub fn bench_unsquashfs_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("unsquashfs");

    let path = download_asset("netgear_ex6100v2");

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
