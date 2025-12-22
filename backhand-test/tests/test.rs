mod common;
use std::fs::File;
use std::io::{BufReader, BufWriter};

use backhand::{FilesystemReader, FilesystemWriter};
use common::{test_bin_unsquashfs, test_squashfs_tools_unsquashfs};
use test_log::test;
use tracing::{info, trace};

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

fn only_read(path: &str, offset: u64) {
    let file = BufReader::new(File::open(path).unwrap());
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
fn full_test(og_path: &str, offset: u64, verify: Verify, assert_success: bool) {
    full_test_inner(og_path, offset, verify, assert_success, true)
}

fn full_test_inner(
    og_path: &str,
    offset: u64,
    verify: Verify,
    assert_success: bool,
    run_squashfs_tools_unsquashfs: bool,
) {
    // Extract directory from og_path to create new_path in same directory
    let path = std::path::Path::new(og_path);
    let dir = path.parent().unwrap();
    let new_path = dir.join("bytes.squashfs");
    let new_path = new_path.to_str().unwrap();
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
#[cfg(feature = "gzip")]
fn test_00() {
    common::download_asset("test_00");
    if has_gzip_feature() {
        full_test("test-assets/test_00/out.squashfs", 0, Verify::Extract, true);
    } else {
        only_read("test-assets/test_00/out.squashfs", 0);
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2
#[test]
#[cfg(feature = "gzip")]
fn test_01() {
    common::download_asset("test_01");
    if has_gzip_feature() {
        full_test("test-assets/test_01/out.squashfs", 0, Verify::Extract, true);
    } else {
        only_read("test-assets/test_01/out.squashfs", 0);
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz
#[test]
#[cfg(feature = "xz")]
fn test_02() {
    common::download_asset("test_02");
    full_test("test-assets/test_02/out.squashfs", 0, Verify::Extract, true);
}

/// mksquashfs ./target/release/squashfs-deku Cargo.toml out.squashfs -comp xz
#[test]
#[cfg(feature = "xz")]
fn test_03() {
    common::download_asset("test_03");
    full_test("test-assets/test_03/out.squashfs", 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_04() {
    common::download_asset("test_04");
    full_test("test-assets/test_04/out.squashfs", 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_05() {
    common::download_asset("test_05");
    full_test("test-assets/test_05/out.squashfs", 0, Verify::Extract, true);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -always-use-fragments
#[test]
#[cfg(feature = "gzip")]
fn test_06() {
    common::download_asset("test_06");
    if has_gzip_feature() {
        full_test("test-assets/test_06/out.squashfs", 0, Verify::Extract, true);
    } else {
        only_read("test-assets/test_06/out.squashfs", 0);
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip
#[test]
#[cfg(feature = "gzip")]
fn test_07() {
    common::download_asset("test_07");
    if has_gzip_feature() {
        full_test("test-assets/test_07/out.squashfs", 0, Verify::Extract, true);
    } else {
        only_read("test-assets/test_07/out.squashfs", 0);
    }
}

// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz -Xbcj arm
#[test]
#[cfg(feature = "xz")]
fn test_08() {
    common::download_asset("test_08");
    full_test("test-assets/test_08/out.squashfs", 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_19() {
    common::download_asset("test_19");
    full_test("test-assets/test_19/out.squashfs", 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_20() {
    common::download_asset("test_20");
    full_test("test-assets/test_20/out.squashfs", 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "xz")]
fn test_openwrt_tplink_archera7v5() {
    common::download_asset("openwrt_tplink_archera7v5");
    full_test(
        "test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin",
        0x0022_5fd0,
        Verify::Extract,
        false,
    );
}

#[test]
#[cfg(feature = "xz")]
fn test_openwrt_netgear_ex6100v2() {
    common::download_asset("netgear_ex6100v2");
    full_test(
        "test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img",
        0x002c_0080,
        Verify::Extract,
        false,
    );
}

#[test]
#[cfg(feature = "gzip")]
fn test_appimage_plexamp() {
    common::download_asset("appimage_plexamp");
    if has_gzip_feature() {
        full_test(
            "test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage",
            0x2dfe8,
            Verify::Extract,
            true,
        );
    } else {
        only_read("test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage", 0x2dfe8);
    }
}

#[test]
#[cfg(feature = "gzip")]
fn test_appimage_firefox() {
    common::download_asset("appimage_firefox");
    if has_gzip_feature() {
        full_test(
            "test-assets/test_appimage_firefox/firefox-108.0.r20221215175817-x86_64.AppImage",
            0x2f4c0,
            Verify::Extract,
            true,
        );
    } else {
        only_read(
            "test-assets/test_appimage_firefox/firefox-108.0.r20221215175817-x86_64.AppImage",
            0x2f4c0,
        );
    }
}

/// Archer\ AX1800\(US\)_V3_221016.zip from https://www.tp-link.com/us/support/download/archer-ax1800/#Firmware
/// (after ubi_extract_image)
#[test]
#[cfg(feature = "xz")]
fn test_tplink_ax1800() {
    common::download_asset("tplink_ax1800");
    full_test(
        "test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs",
        0,
        Verify::Extract,
        false,
    );
}

/// one /console char device
#[test]
#[cfg(feature = "xz")]
fn test_21() {
    common::download_asset("test_21");
    full_test("test-assets/test_21/out.squashfs", 0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_er605() {
    common::download_asset("er605");
    full_test("test-assets/test_er605_v2_2/2611E3.squashfs", 0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_re815xe() {
    common::download_asset("re815xe");
    full_test("test-assets/test_re815_xev160/870D97.squashfs", 0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_slow_archlinux_iso_rootfs() {
    common::download_asset("archlinux_iso_rootfs");
    full_test("test-assets/test_archlinux_iso_rootfs/airootfs.sfs", 0, Verify::Extract, true);
}

#[test]
#[cfg(feature = "gzip")]
fn test_many_files() {
    common::download_asset("many_files");
    if has_gzip_feature() {
        full_test("test-assets/test_many_files/many_files.squashfs", 0, Verify::Extract, true);
    } else {
        only_read("test-assets/test_many_files/many_files.squashfs", 0);
    }
}

#[test]
#[cfg(feature = "gzip")]
fn test_many_dirs() {
    common::download_asset("many_dirs");
    if has_gzip_feature() {
        full_test("test-assets/test_many_dirs/many_dirs.squashfs", 0, Verify::Extract, true);
    } else {
        only_read("test-assets/test_many_dirs/many_dirs.squashfs", 0);
    }
}

#[test]
#[cfg(feature = "gzip")]
fn test_few_dirs_many_files() {
    common::download_asset("few_dirs_many_files");
    if has_gzip_feature() {
        full_test(
            "test-assets/test_few_dirs_many_files/few_dirs_many_files.squashfs",
            0,
            Verify::Extract,
            true,
        );
    } else {
        only_read("test-assets/test_few_dirs_many_files/few_dirs_many_files.squashfs", 0);
    }
}

#[test]
#[cfg(feature = "gzip")]
fn test_socket_fifo() {
    common::download_asset("socket_fifo");
    if has_gzip_feature() {
        full_test_inner(
            "test-assets/socket_fifo/squashfs_v4.specfile.bin",
            0,
            Verify::Extract,
            true,
            false,
        );
    } else {
        only_read("test-assets/socket_fifo/squashfs_v4.specfile.bin", 0);
    }
}

#[test]
#[cfg(feature = "zstd")]
fn no_qemu_test_crates_zstd() {
    common::download_asset("crates_io_zstd");
    trace!("downloaing test");
    trace!("starting test");
    full_test("test-assets/crates_io_zstd/crates-io.squashfs", 0, Verify::Extract, false);
}

#[test]
#[cfg(feature = "xz")]
fn test_slow_sparse_data_issue_623() {
    common::download_asset("sparse_data_issue_623");
    full_test(
        "test-assets/test_sparse_data_issue_623/aosc-os_buildkit_20251206_amd64.squashfs",
        0,
        Verify::Extract,
        true,
    );
}

#[test]
#[cfg(feature = "lz4")]
fn test_lz4_write_read() {
    common::download_asset("lz4_write_read");
    full_test("test-assets/test_lz4_write_read/testing.lz4.squash", 0, Verify::Extract, true);
}
