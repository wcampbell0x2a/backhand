use std::fs::{self, File};
use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use squashfs_deku::compressor::{CompressionOptions, Gzip};
use squashfs_deku::filesystem::{Node, SquashfsFile};
use squashfs_deku::Squashfs;
use tempfile::tempdir;
use test_assets::TestAssetDef;
use test_log::test;
use tracing::info;

fn test_unsquashfs(control: &str, new: &str, control_offset: Option<u64>) {
    let control_dir = tempdir().unwrap();
    Command::new("unsquashfs")
        .args([
            "-d",
            control_dir.path().to_str().unwrap(),
            "-o",
            &control_offset.unwrap_or(0).to_string(),
            // we don't run as root, avoid special file errors
            "-ignore-errors",
            "-no-exit-code",
            control,
        ])
        .assert()
        .code(&[0] as &[i32]);

    let new_dir = tempdir().unwrap();
    Command::new("unsquashfs")
        .args([
            "-d",
            new_dir.path().to_str().unwrap(),
            // we don't run as root, avoid special file errors
            "-ignore-errors",
            "-no-exit-code",
            new,
        ])
        .assert()
        .code(&[0] as &[i32]);

    let d = dir_diff::is_different(control_dir.path(), new_dir.path());
    assert!(!d.expect("couldn't compare dirs"));
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2 -always-use-fragments
#[test]
fn test_00() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "976c1638d8c1ba8014de6c64b196cbd70a5acf031be10a8e7f649536193c8e78".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_00/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "squashfs-deku".to_string(),
            hash: "71c1af12ec9097314e6ea64b98fec55103c5ade2d61822de3b2afcccf3263202".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_00/squashfs-deku".to_string(),
        },
    ];
    const TEST_PATH: &str = "test-assets/test_00";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    let squashfs = Squashfs::from_reader(file).unwrap();

    assert_eq!(
        squashfs.compression_options,
        Some(CompressionOptions::Gzip(Gzip {
            compression_level: 2,
            window_size: 15,
            strategies: 0
        }))
    );

    // convert to bytes
    let filesystem = squashfs.into_filesystem().unwrap();
    let bytes = filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let _ = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();

    test_unsquashfs(&og_path, &new_path, None);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -Xcompression-level 2
#[test]
fn test_01() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "9d9f5ba77b562fd4141fc725038028822673b24595e2774a8718260f4fc39710".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_01/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "squashfs-deku".to_string(),
            hash: "d500f2e3c4a8767257eb1e16b260f33b295ee4bd6c91847b49b8250c4bbbcad9".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_01/squashfs-deku".to_string(),
        },
    ];

    const TEST_PATH: &str = "test-assets/test_01";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let filesystem = squashfs.into_filesystem().unwrap();

    // convert to bytes
    let bytes = filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let _ = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();

    test_unsquashfs(&og_path, &new_path, None);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz
#[test]
fn test_02() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "c18d1b57e73740ab4804672c61f5c77f170cc16179d9a7e12dd722ba311f5623".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_02/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "squashfs-deku".to_string(),
            hash: "d500f2e3c4a8767257eb1e16b260f33b295ee4bd6c91847b49b8250c4bbbcad9".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_02/squashfs-deku".to_string(),
        },
    ];

    const TEST_PATH: &str = "test-assets/test_02";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let filesystem = squashfs.into_filesystem().unwrap();
    // convert to bytes
    let bytes = filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let _ = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();

    test_unsquashfs(&og_path, &new_path, None);
}

/// mksquashfs ./target/release/squashfs-deku Cargo.toml out.squashfs -comp xz
#[test]
fn test_03() {
    let asset_defs = [
        TestAssetDef {
            filename: "Cargo.toml".to_string(),
            hash: "30369cb0cd81b36609c459a48acc6704aa60a95275981404ff411dd5f1eb3304".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_03/Cargo.toml".to_string(),
        },
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "4171d9dd5a53f2ad841715af1c01351028a9d9df13e4ae8172f37660306c0473".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_03/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "squashfs-deku".to_string(),
            hash: "4f7e334dcf980d4d86f7b891e7ff4ad05bef2eac865f4b77063ec2e5b070b595".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_03/squashfs-deku".to_string(),
        },
    ];

    const TEST_PATH: &str = "test-assets/test_03";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let filesystem = squashfs.into_filesystem().unwrap();

    // convert to bytes
    let bytes = filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let _ = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();

    test_unsquashfs(&og_path, &new_path, None);
}

#[test]
fn test_04() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "bfb3424bf3b744b8c7a156c9c538310c49fbe8a57f336864f00210e6f356f2c3".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_04/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "03".to_string(),
            hash: "90117dea9028cf65911c2024f11aa3fcc555b847cb5e44e93e7bd890d79cfb88".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_04/testing/03".to_string(),
        },
        TestAssetDef {
            filename: "04".to_string(),
            hash: "784636e0b138cf6182fc9af9b39ff9c38ae3ffd0b6b78381a55ba595ffc78a1c".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_04/testing/what/04".to_string(),
        },
        TestAssetDef {
            filename: "01".to_string(),
            hash: "7c1527ba3e29054d348279f66592bc7d7ad4441bf18e5478906f918793d3562c".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_04/testing/what/yikes/01".to_string(),
        },
        TestAssetDef {
            filename: "02".to_string(),
            hash: "e4818e2fdfafe27b1b42ee15fdd6494194e534ecc5667acddfbaa3ac9311df31".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_04/testing/what/yikes/02".to_string(),
        },
        TestAssetDef {
            filename: "05".to_string(),
            hash: "4a2c0fe812a83a3a906c1f3c0ee55f9fad520610d361b6afd5c3dedeaa287a39".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_04/testing/woah/05".to_string(),
        },
    ];

    const TEST_PATH: &str = "test-assets/test_04";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    info!("{:?}", file);
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let filesystem = squashfs.into_filesystem().unwrap();
    for u in &filesystem.nodes {
        if let Node::File(SquashfsFile { path, bytes, .. }) = u {
            let filepath = Path::new(TEST_PATH).join(path.file_name().unwrap());
            let expected_bytes = fs::read(filepath).unwrap();
            assert_eq!(bytes, &*expected_bytes);
        }
    }
    // convert to bytes
    let bytes = filesystem.to_bytes().unwrap();

    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let new_squashfs = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();
    tracing::trace!("{:#02x?}", new_squashfs.inodes);

    test_unsquashfs(&og_path, &new_path, None);
}

#[test]
fn test_05() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "6195e4d8d14c63dffa9691d36efa1eda2ee975b476bb95d4a0b59638fd9973cb".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_05/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "d".to_string(),
            hash: "0641203cb2bbb7d75bcc537f38627caa301f3df01a2cea539b34274d6bbef7f1".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_05/a/b/c/d".to_string(),
        },
    ];

    const TEST_PATH: &str = "test-assets/test_05";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    info!("{file:?}");
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let filesystem = squashfs.into_filesystem().unwrap();
    for u in &filesystem.nodes {
        if let Node::File(SquashfsFile { path, bytes, .. }) = u {
            let filepath = Path::new(TEST_PATH).join(path.file_name().unwrap());
            let expected_bytes = fs::read(filepath).unwrap();
            assert_eq!(bytes, &*expected_bytes);
        }
    }

    // convert to bytes
    let bytes = filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let _ = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();

    test_unsquashfs(&og_path, &new_path, None);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip -always-use-fragments
#[test]
fn test_06() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "3c5db6e8c59a4e1291a016f736fbf76ddc1e07fa4bc8940eac1754975b4c617b".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_06/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "squashfs-deku".to_string(),
            hash: "71c1af12ec9097314e6ea64b98fec55103c5ade2d61822de3b2afcccf3263202".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_06/squashfs-deku".to_string(),
        },
    ];

    const TEST_PATH: &str = "test-assets/test_06";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    info!("{file:?}");
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);
    let filesystem = squashfs.into_filesystem().unwrap();

    // convert to bytes
    let bytes = filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let _ = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();

    test_unsquashfs(&og_path, &new_path, None);
}

/// mksquashfs ./target/release/squashfs-deku out.squashfs -comp gzip
#[test]
fn test_07() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "6bc1571d82473e74a55cfd2d07ce21d9150ea4ad5941d2345ea429507d812671".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_07/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "squashfs-deku".to_string(),
            hash: "71c1af12ec9097314e6ea64b98fec55103c5ade2d61822de3b2afcccf3263202".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_07/squashfs-deku".to_string(),
        },
    ];

    const TEST_PATH: &str = "test-assets/test_07";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    info!("{file:?}");
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let filesystem = squashfs.into_filesystem().unwrap();

    // convert to bytes
    let bytes = filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let _ = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();

    test_unsquashfs(&og_path, &new_path, None);
}

// mksquashfs ./target/release/squashfs-deku out.squashfs -comp xz -Xbcj arm
#[test]
fn test_08() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "debe0986658b276be78c3836779d20464a03d9ba0a40903e6e8e947e434f4d67".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_08/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "squashfs-deku".to_string(),
            hash: "18b5a0c441b8b2ab7b3f2dd2ae869897e0a963cd2b33ce0dfebb22bcaadc0897".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_08/squashfs-deku".to_string(),
        },
    ];

    const TEST_PATH: &str = "test-assets/test_08";
    let og_path: String = format!("{TEST_PATH}/out.squashfs");
    let new_path: String = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let file = File::open(&og_path).unwrap();
    info!("{file:?}");
    let squashfs = Squashfs::from_reader(file).unwrap();
    info!("{:02x?}", squashfs.superblock);

    let filesystem = squashfs.into_filesystem().unwrap();

    // convert to bytes
    let bytes = filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    let _ = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();

    test_unsquashfs(&og_path, &new_path, None);
}

fn factory_test(assets_defs: &[TestAssetDef], filepath: &str, test_path: &str, offset: u64) {
    test_assets::download_test_files(assets_defs, test_path, true).unwrap();

    let og_path = format!("{test_path}/{filepath}");
    let new_path = format!("{test_path}/bytes.squashfs");
    let file = File::open(&og_path).unwrap();
    info!("calling from_reader");
    let squashfs = Squashfs::from_reader_with_offset(file, offset).unwrap();

    // convert to bytes
    info!("calling into_filesystem");
    let og_filesystem = squashfs.into_filesystem().unwrap();
    info!("calling to_bytes");
    let bytes = og_filesystem.to_bytes().unwrap();
    fs::write(&new_path, &bytes).unwrap();

    // assert that our library can atleast read the output, use unsquashfs to really assert this
    info!("calling from_reader");
    let new_squashfs = Squashfs::from_reader(std::io::Cursor::new(bytes)).unwrap();
    info!("calling into_filesystem");
    let _new_filesystem = new_squashfs.into_filesystem().unwrap();

    info!("starting unsquashfs test");
    test_unsquashfs(&og_path, &new_path, Some(offset));
}

#[test]
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
    factory_test(&asset_defs, FILE_NAME, TEST_PATH, 0x225fd0);
}

#[test]
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
    factory_test(&asset_defs, FILE_NAME, TEST_PATH, 0x2c0080);
}

#[test]
fn test_appimage_plexamp() {
    const FILE_NAME: &str = "Plexamp-4.6.1.AppImage";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "6d2a3fba571da54e6869c2f7e1f7e6ca22f380a9a6f7a44a5ac675d1c656b584".to_string(),
        url: format!("https://plexamp.plex.tv/plexamp.plex.tv/desktop/{FILE_NAME}"),
    }];

    const TEST_PATH: &str = "test-assets/test_appimage_plexamp";
    factory_test(&asset_defs, FILE_NAME, TEST_PATH, 0x2dfe8);
}

#[test]
fn test_appimage_firefox() {
    const FILE_NAME: &str = "firefox-108.0.r20221215175817-x86_64.AppImage";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "b635ffdd24ecde8991cbc829773ca049994145cb2194c7c3987a87591153e58b".to_string(),
        url: format!(
            "https://github.com/srevinsaju/Firefox-Appimage/releases/download/firefox/{FILE_NAME}"
        ),
    }];

    const TEST_PATH: &str = "test-assets/test_appimage_firefox";
    factory_test(&asset_defs, FILE_NAME, TEST_PATH, 0x2f4c0);
}
