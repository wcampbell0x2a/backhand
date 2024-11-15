mod common;

use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::tempdir;
use test_assets_ureq::TestAssetDef;
use test_log::test;

use crate::common::read_asset;

#[test]
#[cfg(feature = "xz")]
fn test_replace() {
    let (test_path, asset_def) = read_asset("test_05");
    let asset_defs = &[asset_def];
    let file_name = &asset_defs[0].filename;

    common::download_backoff(asset_defs, &test_path);
    let image_path = format!("{test_path}/{file_name}");

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
