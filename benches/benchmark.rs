use std::fs::File;
use std::io::Cursor;

use backhand::{FilesystemReader, FilesystemWriter};
use criterion::*;
use test_assets::TestAssetDef;

fn read_write(file: File, offset: u64) {
    let og_filesystem = FilesystemReader::from_reader_with_offset(file, offset).unwrap();
    let mut new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();

    // convert to bytes
    let mut output = Cursor::new(vec![]);
    black_box(new_filesystem.write(&mut output).unwrap());
}

fn read(file: File, offset: u64) {
    black_box(FilesystemReader::from_reader_with_offset(file, offset).unwrap());
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
    const TEST_PATH_00: &str = "test-assets/test_openwrt_netgear_ex6100v2";
    test_assets::download_test_files(&asset_defs, TEST_PATH_00, true).unwrap();
    let og_path = format!("{TEST_PATH_00}/{FILE_NAME_00}");
    group.bench_function("netgear_ax6100v2", |b| {
        b.iter(|| {
            let file = File::open(&og_path).unwrap();
            read_write(file, 0x2c0080)
        })
    });

    const FILE_NAME_01: &str = "img-1571203182_vol-ubi_rootfs.ubifs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME_01.to_string(),
        hash: "e6adbea10615a8ed9f88e403e2478010696f421f4d69a790d37d97fe8921aa81".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_tplink1800/{FILE_NAME_01}"),
    }];
    const TEST_PATH_01: &str = "test-assets/test_tplink_ax1800";
    test_assets::download_test_files(&asset_defs, TEST_PATH_01, true).unwrap();
    let og_path = format!("{TEST_PATH_01}/{FILE_NAME_01}");
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
    const TEST_PATH_00: &str = "test-assets/test_openwrt_netgear_ex6100v2";
    test_assets::download_test_files(&asset_defs, TEST_PATH_00, true).unwrap();
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
        url: format!("wcampbell.dev/squashfs/testing/test_tplink1800/{FILE_NAME_01}"),
    }];
    const TEST_PATH_01: &str = "test-assets/test_tplink_ax1800";
    test_assets::download_test_files(&asset_defs, TEST_PATH_01, true).unwrap();
    let og_path = format!("{TEST_PATH_01}/{FILE_NAME_01}");
    group.bench_function("tplink_ax1800", |b| {
        b.iter(|| {
            let file = File::open(&og_path).unwrap();
            read(file, 0)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_read_write, bench_read);
criterion_main!(benches);
