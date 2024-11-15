mod common;
use std::fs::File;
use std::io::{BufReader, BufWriter};

use assert_cmd::prelude::*;
use assert_cmd::Command;
use backhand::{FilesystemReader, FilesystemWriter};
use common::{read_asset, test_bin_unsquashfs, test_squashfs_tools_unsquashfs};
use tempfile::tempdir;
use test_assets_ureq::TestAssetDef;
use test_log::test;
use tracing::info;

#[cfg(feature = "gzip")]
fn has_gzip_feature() -> bool {
    true
}

#[cfg(not(feature = "gzip"))]
fn has_gzip_feature() -> bool {
    false
}

enum Verify {
    Extract,
}

fn only_read(assets_defs: &[TestAssetDef], filepath: &str, test_path: &str, offset: u64) {
    common::download_backoff(assets_defs, test_path);

    let og_path = format!("{test_path}/{filepath}");
    let file = BufReader::new(File::open(&og_path).unwrap());
    info!("calling from_reader");
    let _ = FilesystemReader::from_reader_with_offset(file, offset).unwrap();

    // TODO: this should still check our own unsquashfs
}

/// - Download file
/// - Read into Squashfs
/// - Into Filesystem
/// - Into Bytes
/// - - Into Squashfs
/// - - Into Filesystem
/// - unsquashfs-tools/unsquashfs both and assert to diff in files
fn full_test(
    assets_defs: &[TestAssetDef],
    filepath: &str,
    test_path: &str,
    offset: u64,
    verify: Verify,
    assert_success: bool,
) {
    full_test_inner(assets_defs, filepath, test_path, offset, verify, assert_success, true)
}

fn full_test_inner(
    assets_defs: &[TestAssetDef],
    filepath: &str,
    test_path: &str,
    offset: u64,
    verify: Verify,
    assert_success: bool,
    run_squashfs_tools_unsquashfs: bool,
) {
    common::download_backoff(assets_defs, test_path);

    let og_path = format!("{test_path}/{filepath}");
    let new_path = format!("{test_path}/bytes.squashfs");
    let file = BufReader::new(File::open(&og_path).unwrap());
    info!("calling from_reader");
    let og_filesystem = FilesystemReader::from_reader_with_offset(file, offset).unwrap();
    let og_comp_opts = og_filesystem.compression_options;
    let mut new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();

    // convert to bytes
    info!("calling to_bytes");
    let mut output = BufWriter::new(File::create(&new_path).unwrap());
    new_filesystem.write_with_offset(&mut output, offset).unwrap();
    info!("done with writing to bytes");

    drop(new_filesystem);
    drop(og_filesystem);

    // assert that our library can at least read the output, use unsquashfs to really assert this
    info!("calling from_reader");
    let created_file = BufReader::new(File::open(&new_path).unwrap());
    let written_new_filesystem =
        FilesystemReader::from_reader_with_offset(created_file, offset).unwrap();

    // compression options are the same
    let new_comp_opts = written_new_filesystem.compression_options;
    assert_eq!(og_comp_opts, new_comp_opts);

    drop(written_new_filesystem);
    match verify {
        Verify::Extract => {
            if run_squashfs_tools_unsquashfs {
                #[cfg(feature = "__test_unsquashfs")]
                {
                    info!("starting squashfs-tools/unsquashfs test");
                    test_squashfs_tools_unsquashfs(
                        &og_path,
                        &new_path,
                        Some(offset),
                        assert_success,
                    );
                }
            }
            info!("starting backhand/unsquashfs original test");
            test_bin_unsquashfs(
                &og_path,
                Some(offset),
                assert_success,
                run_squashfs_tools_unsquashfs,
            );
            info!("starting backhand/unsquashfs created test");
            test_bin_unsquashfs(
                &new_path,
                Some(offset),
                assert_success,
                run_squashfs_tools_unsquashfs,
            );
        }
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2 -always-use-fragments
#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_00() {
    let (test_path, asset_def) = read_asset("test_00");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0);
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2
#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_01() {
    let (test_path, asset_def) = read_asset("test_01");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0);
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz
#[test]
#[cfg(feature = "xz")]
fn test_02() {
    let (test_path, asset_def) = read_asset("test_02");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}

/// mksquashfs ./target/release/squashfs-deku Cargo.toml out.squashfs -comp xz
#[test]
#[cfg(feature = "xz")]
fn test_03() {
    let (test_path, asset_def) = read_asset("test_03");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_04() {
    let (test_path, asset_def) = read_asset("test_04");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_05() {
    let (test_path, asset_def) = read_asset("test_05");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -always-use-fragments
#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_06() {
    let (test_path, asset_def) = read_asset("test_06");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0);
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip
#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_07() {
    let (test_path, asset_def) = read_asset("test_07");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0);
    }
}

// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz -Xbcj arm
#[test]
#[cfg(feature = "xz")]
fn test_08() {
    let (test_path, asset_def) = read_asset("test_08");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_19() {
    let (test_path, asset_def) = read_asset("test_10");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_20() {
    let (test_path, asset_def) = read_asset("test_20");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_openwrt_tplink_archera7v5() {
    let (test_path, asset_def) = read_asset("test_openwrt_tplink_archera7v5");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0x0022_5fd0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_openwrt_netgear_ex6100v2() {
    let (test_path, asset_def) = read_asset("netgear_ex6100v2");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0x002c_0080, Verify::Extract, false);
}

#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_appimage_plexamp() {
    let (test_path, asset_def) = read_asset("plexamp-4-6-1");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0x2dfe8, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0x2dfe8);
    }
}

#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_appimage_firefox() {
    let (test_path, asset_def) = read_asset("firefox-108-appimage");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0x2f4c0, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0x2f4c0);
    }
}

/// Archer\ AX1800\(US\)_V3_221016.zip from https://www.tp-link.com/us/support/download/archer-ax1800/#Firmware
/// (after ubi_extract_image)
#[test]
#[cfg(feature = "xz")]
fn test_tplink_ax1800() {
    let (test_path, asset_def) = read_asset("test_tplink_ax1800");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, false);
}

/// one /console char device
#[test]
#[cfg(feature = "xz")]
fn test_21() {
    let (test_path, asset_def) = read_asset("test_210");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_er605() {
    let (test_path, asset_def) = read_asset("test_er605_v2_2");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_re815xe() {
    let (test_path, asset_def) = read_asset("test_er605_xev160");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_slow_archlinux_iso_rootfs() {
    let (test_path, asset_def) = read_asset("test_archlinux_iso_rootfs");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}

#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_many_files() {
    let (test_path, asset_def) = read_asset("test_many_files");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0);
    }
}

#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_many_dirs() {
    let (test_path, asset_def) = read_asset("test_many_dirs");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0);
    }
}

#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_few_dirs_many_files() {
    let (test_path, asset_def) = read_asset("test_few_dirs_many_files");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0);
    }
}

#[test]
#[cfg(any(feature = "gzip", feature = "gzip-zune-inflate"))]
fn test_socket_fifo() {
    let (test_path, asset_def) = read_asset("test_socket_fifo");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    if has_gzip_feature() {
        full_test_inner(asset_defs, &file_name, &test_path, 0, Verify::Extract, true, false);
    } else {
        only_read(asset_defs, &file_name, &test_path, 0);
    }
}

#[test]
#[cfg(any(feature = "zstd"))]
fn no_qemu_test_crates_zstd() {
    let (test_path, asset_def) = read_asset("crates_io_zstd");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_slow_sparse_data_issue_623() {
    let (test_path, asset_def) = read_asset("test_slow_sparse_data_issue_623");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    full_test(asset_defs, &file_name, &test_path, 0, Verify::Extract, true);
}
