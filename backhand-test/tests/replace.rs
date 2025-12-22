mod common;

use assert_cmd::prelude::*;
use tempfile::tempdir;
use test_log::test;

#[test]
#[cfg(feature = "xz")]
fn test_replace() {
    common::download_asset("test_05");
    let image_path = "test-assets/test_05/out.squashfs";

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
