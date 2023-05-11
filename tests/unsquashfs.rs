mod bin;
mod common;

use std::process::Command;

use assert_cmd::prelude::*;
use test_assets::TestAssetDef;

#[test]
#[cfg(feature = "xz")]
fn test_unsquashfs_cli() {
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
    let cmd = Command::cargo_bin("unsquashfs")
        .unwrap()
        .env("RUST_LOG", "none")
        .args(["--path-filter", r#"/usr/bin/wget"#, "-l", &image_path])
        .unwrap();
    cmd.assert().stdout(
        r#"/
/usr
/usr/bin
/usr/bin/wget
"#,
    );

    // multiple file
    let cmd = Command::cargo_bin("unsquashfs")
        .unwrap()
        .env("RUST_LOG", "none")
        .args(["--path-filter", r#"/www/webpages/data"#, "-l", &image_path])
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
