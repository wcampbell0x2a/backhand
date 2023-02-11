mod common;
use std::fs::File;

use backhand::{FilesystemReader, FilesystemWriter};
use common::{test_unsquashfs, test_unsquashfs_list};
use test_assets::TestAssetDef;
use test_log::test;
use tracing::info;

enum Verify {
    Extract,
    List,
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
) {
    test_assets::download_test_files(assets_defs, test_path, true).unwrap();

    let og_path = format!("{test_path}/{filepath}");
    let new_path = format!("{test_path}/bytes.squashfs");
    let file = File::open(&og_path).unwrap();
    info!("calling from_reader");
    let og_filesystem = FilesystemReader::from_reader_with_offset(file, offset).unwrap();
    let new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();

    // convert to bytes
    info!("calling to_bytes");
    let mut output = File::create(&new_path).unwrap();
    new_filesystem.write(&mut output).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    info!("calling from_reader");
    let created_file = File::open(&new_path).unwrap();
    let _new_filesystem = FilesystemReader::from_reader(created_file).unwrap();

    info!("starting unsquashfs test");
    match verify {
        Verify::Extract => {
            test_unsquashfs(&og_path, &new_path, Some(offset));
        },
        Verify::List => {
            test_unsquashfs_list(&og_path, &new_path, Some(offset));
        },
    }
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2 -always-use-fragments
#[test]
#[cfg(feature = "gzip")]
fn test_00() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "976c1638d8c1ba8014de6c64b196cbd70a5acf031be10a8e7f649536193c8e78".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_00/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_00";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2
#[test]
#[cfg(feature = "gzip")]
fn test_01() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "9d9f5ba77b562fd4141fc725038028822673b24595e2774a8718260f4fc39710".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_01/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_01";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz
#[test]
#[cfg(feature = "xz")]
fn test_02() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "c18d1b57e73740ab4804672c61f5c77f170cc16179d9a7e12dd722ba311f5623".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_02/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_02";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

/// mksquashfs ./target/release/squashfs-deku Cargo.toml out.squashfs -comp xz
#[test]
#[cfg(feature = "xz")]
fn test_03() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "4171d9dd5a53f2ad841715af1c01351028a9d9df13e4ae8172f37660306c0473".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_03/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_03";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

#[test]
#[cfg(feature = "xz")]
fn test_04() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "bfb3424bf3b744b8c7a156c9c538310c49fbe8a57f336864f00210e6f356f2c3".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_04/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_04";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

#[test]
#[cfg(feature = "xz")]
fn test_05() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "6195e4d8d14c63dffa9691d36efa1eda2ee975b476bb95d4a0b59638fd9973cb".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_05/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_05";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -always-use-fragments
#[test]
#[cfg(feature = "gzip")]
fn test_06() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "3c5db6e8c59a4e1291a016f736fbf76ddc1e07fa4bc8940eac1754975b4c617b".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_06/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_06";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip
#[test]
#[cfg(feature = "gzip")]
fn test_07() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "6bc1571d82473e74a55cfd2d07ce21d9150ea4ad5941d2345ea429507d812671".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_07/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_07";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz -Xbcj arm
#[test]
#[cfg(feature = "xz")]
fn test_08() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "ce0bfab79550885cb7ced388caaaa9bd454852bf1f9c34789abc498eb6c74df6".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_08/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_08";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

#[test]
#[cfg(feature = "xz")]
fn test_19() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "4dc83c3eea0d7ae2a23c891798d485ba0eded862db5e1528a984e08b35255b0f".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_19/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_19";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

#[test]
#[cfg(feature = "xz")]
fn test_20() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "4f00c0debb2d40ecb45f8d5d176a97699a8e07727713883899e6720331d67078".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_20/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_20";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

#[test]
#[cfg(feature = "xz")]
fn test_openwrt_tplink_archera7v5() {
    const FILE_NAME: &str =
        "openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "ce0bfab79550885cb7ced388caaaa9bd454852bf1f9c34789abc498eb6c74df6".to_string(),
        url: format!(
            "https://downloads.openwrt.org/releases/22.03.2/targets/ath79/generic/{FILE_NAME}"
        ),
    }];
    const TEST_PATH: &str = "test-assets/test_openwrt_tplink_archera7v5";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0x225fd0, Verify::Extract);
}

#[test]
#[cfg(feature = "xz")]
#[cfg(not(feature = "xz-static"))]
fn test_openwrt_netgear_ex6100v2() {
    const FILE_NAME: &str = "openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img";

    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "9608a6cb558f1a4aa9659257f7c0b401f94343d10ec6e964fc4a452b4f91bea4".to_string(),
        url: format!(
            "https://downloads.openwrt.org/releases/22.03.2/targets/ipq40xx/generic/{FILE_NAME}"
        ),
    }];
    const TEST_PATH: &str = "test-assets/test_openwrt_netgear_ex6100v2";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0x2c0080, Verify::Extract);
}

#[test]
#[cfg(feature = "gzip")]
fn test_appimage_plexamp() {
    const FILE_NAME: &str = "Plexamp-4.6.1.AppImage";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "6d2a3fba571da54e6869c2f7e1f7e6ca22f380a9a6f7a44a5ac675d1c656b584".to_string(),
        url: format!("https://plexamp.plex.tv/plexamp.plex.tv/desktop/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_appimage_plexamp";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0x2dfe8, Verify::Extract);
}

#[test]
#[cfg(feature = "gzip")]
fn test_appimage_firefox() {
    const FILE_NAME: &str = "firefox-108.0.r20221215175817-x86_64.AppImage";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "b635ffdd24ecde8991cbc829773ca049994145cb2194c7c3987a87591153e58b".to_string(),
        url: "https://github.com/srevinsaju/Firefox-Appimage/releases/download/firefox-v108.0.r20221215175817/firefox-108.0.r20221215175817-x86_64.AppImage".to_string(),
    }];
    const TEST_PATH: &str = "test-assets/test_appimage_firefox";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0x2f4c0, Verify::Extract);
}

/// Archer\ AX1800\(US\)_V3_221016.zip from https://www.tp-link.com/us/support/download/archer-ax1800/#Firmware
/// (after ubi_extract_image)
#[test]
#[cfg(feature = "xz")]
#[cfg(not(feature = "xz-static"))]
fn test_tplink_ax1800() {
    const FILE_NAME: &str = "img-1571203182_vol-ubi_rootfs.ubifs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "e6adbea10615a8ed9f88e403e2478010696f421f4d69a790d37d97fe8921aa81".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_tplink1800/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_tplink_ax1800";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::Extract);
}

/// one /console char device
#[test]
#[cfg(feature = "xz")]
fn test_21() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "8fe23229be6c3e24b9565007f9f9a25e8e796270cf7ce8518da131e95bb90bad".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_21/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_21";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::List);
}

/// This has a char device (/dev/console), so we can only test the list
#[test]
#[cfg(feature = "xz")]
fn test_er605() {
    const FILE_NAME: &str = "2611E3.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "8f69958e5e25a7b9162342739305361dcd6b5a56970e342d85060f9f3be6313c".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_er605_v2_2.0.1/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_er605_v2_2";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::List);
}

/// This has a char device (/dev/console), so we can only test the list
#[test]
#[cfg(feature = "xz")]
fn test_re815xe() {
    const FILE_NAME: &str = "870D97.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "a73325883568ba47eaa5379c7768ded5661d61841a81d6c987371842960ac6a2".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_re815xev1.60/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_re815_xev160";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, Verify::List);
}
