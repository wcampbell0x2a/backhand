mod common;

use assert_cmd::prelude::*;
use test_assets::TestAssetDef;

#[test]
#[cfg(feature = "xz")]
#[cfg(feature = "__test_unsquashfs")]
fn test_unsquashfs_cli_path_filter() {
    const FILE_NAME: &str = "870D97.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "a73325883568ba47eaa5379c7768ded5661d61841a81d6c987371842960ac6a2".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_re815xev1/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_re815_xev160";

    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();
    let image_path = format!("{TEST_PATH}/{FILE_NAME}");

    // single file
    let cmd = common::get_base_command("unsquashfs")
        .env("RUST_LOG", "none")
        .args([
            "--path-filter",
            r#"/usr/bin/wget"#,
            "-l",
            "--quiet",
            &image_path,
        ])
        .unwrap();
    cmd.assert().stdout(
        r#"/
/usr
/usr/bin
/usr/bin/wget
"#,
    );

    // multiple file
    let cmd = common::get_base_command("unsquashfs")
        .env("RUST_LOG", "none")
        .args([
            "--path-filter",
            r#"/www/webpages/data"#,
            "-l",
            "--quiet",
            &image_path,
        ])
        .unwrap();
    cmd.assert().stdout(
        r#"/
/www
/www/webpages
/www/webpages/data
/www/webpages/data/region.json
/www/webpages/data/timezone.json
"#,
    );
}

#[test]
#[cfg(feature = "xz")]
fn test_unsquashfs_cli_auto_offset() {
    use tempfile::tempdir;

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
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();
    let image_path = format!("{TEST_PATH}/{FILE_NAME}");

    let tmp_dir = tempdir().unwrap();
    {
        let cmd = common::get_base_command("unsquashfs")
            .env("RUST_LOG", "none")
            .args([
                "--auto-offset",
                "-d",
                tmp_dir.path().join("squashfs-root-c").to_str().unwrap(),
                &image_path,
            ])
            .unwrap();
        cmd.assert().code(&[0] as &[i32]);
    }
}

#[test]
#[cfg(feature = "gzip")]
fn test_v3_be() {
    use std::fs;
    use tempfile::tempdir;

    const FILE_NAME: &str = "squashfs_v3_be.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "4e9493d9c6f868005dea8f11992129c3399e0bcb8f3a966c750cb925989ca97c".to_string(),
        url: format!("https://github.com/onekey-sec/unblob/raw/main/tests/integration/filesystem/squashfs/squashfs_v3/big_endian/__input__/{FILE_NAME}"),
    }];

    const TEST_PATH: &str = "test-assets/squashfs_v3_be.bin";

    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();
    let image_path = format!("{TEST_PATH}/{FILE_NAME}");

    let tmp_dir = tempdir().unwrap();
    {
        let cmd = common::get_base_command("unsquashfs")
            .env("RUST_LOG", "none")
            .args([
                "--auto-offset",
                "--kind",
                "be_v3_0",
                "-d",
                tmp_dir.path().join("squashfs-root-c").to_str().unwrap(),
                &image_path,
            ])
            .unwrap();
        cmd.assert().code(&[0] as &[i32]);
    }

    assert_eq!(
        "apple\n",
        fs::read_to_string(tmp_dir.path().join("squashfs-root-c").join("apple.txt")).unwrap()
    );
    assert_eq!(
        "cherry\n",
        fs::read_to_string(tmp_dir.path().join("squashfs-root-c").join("cherry.txt")).unwrap()
    );
}

#[test]
#[cfg(feature = "gzip")]
fn test_v3_le() {
    use std::fs;
    use tempfile::tempdir;

    const FILE_NAME: &str = "squashfs_v3_le.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "0161351caec8e9da6e3e5ac7b046fd11d832efb18eb09e33011e6d19d50cd1f7".to_string(),
        url: format!("https://github.com/onekey-sec/unblob/raw/main/tests/integration/filesystem/squashfs/squashfs_v3/little_endian/__input__/{FILE_NAME}"),
    }];

    const TEST_PATH: &str = "test-assets/squashfs_v3_le.bin";

    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();
    let image_path = format!("{TEST_PATH}/{FILE_NAME}");

    let tmp_dir = tempdir().unwrap();
    {
        let cmd = common::get_base_command("unsquashfs")
            .env("RUST_LOG", "none")
            .args([
                "--auto-offset",
                "--kind",
                "le_v3_0",
                "-d",
                tmp_dir.path().join("squashfs-root-c").to_str().unwrap(),
                &image_path,
            ])
            .unwrap();
        cmd.assert().code(&[0] as &[i32]);
    }

    assert_eq!(
        "apple\n",
        fs::read_to_string(tmp_dir.path().join("squashfs-root-c").join("apple.txt")).unwrap()
    );
    assert_eq!(
        "cherry\n",
        fs::read_to_string(tmp_dir.path().join("squashfs-root-c").join("cherry.txt")).unwrap()
    );
}
