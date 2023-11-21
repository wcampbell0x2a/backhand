mod common;

use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::tempdir;
use test_assets::TestAssetDef;
use test_log::test;

#[test]
#[cfg(feature = "xz")]
fn test_replace() {
    const FILE_NAME: &str = "out.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "6195e4d8d14c63dffa9691d36efa1eda2ee975b476bb95d4a0b59638fd9973cb".to_string(),
        url: format!("https://wcampbell.dev/squashfs/testing/test_05/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_05";

    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();
    let image_path = format!("{TEST_PATH}/{FILE_NAME}");

    // extract single file
    let tmp_dir = tempdir().unwrap();
    let cmd = common::get_base_command("unsquashfs-backhand")
        .env("RUST_LOG", "none")
        .args([
            "--path-filter",
            r#"/b/c/d"#,
            "-i",
            &image_path,
            "-d",
            tmp_dir.path().join("squashfs-root-rust").to_str().unwrap(),
        ])
        .unwrap();
    cmd.assert().code(0);

    // edit that file
    let text = b"The mystery of life isn't a problem to solve, but a reality to experience.";
    std::fs::write(tmp_dir.path().join("squashfs-root-rust/b/c/d").to_str().unwrap(), text)
        .unwrap();

    // replace that file
    let cmd = common::get_base_command("replace-backhand")
        .env("RUST_LOG", "none")
        .args([
            &image_path,
            tmp_dir.path().join("squashfs-root-rust/b/c/d").to_str().unwrap(),
            "/b/c/d",
            "-o",
            tmp_dir.path().join("replaced").to_str().unwrap(),
        ])
        .unwrap();
    cmd.assert().code(0);

    // extract
    {
        let cmd = common::get_base_command("unsquashfs-backhand")
            .env("RUST_LOG", "none")
            .args([
                "--path-filter",
                r#"/b/c/d"#,
                "-i",
                tmp_dir.path().join("replaced").to_str().unwrap(),
                "-d",
                tmp_dir.path().join("squashfs-root-rust2").to_str().unwrap(),
            ])
            .unwrap();
        cmd.assert().code(0);

        // assert the text changed!
        let bytes =
            std::fs::read(tmp_dir.path().join("squashfs-root-rust2/b/c/d").to_str().unwrap())
                .unwrap();
        assert_eq!(bytes, text);
    }
}
