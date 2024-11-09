mod common;

use assert_cmd::prelude::*;
use test_assets_ureq::TestAssetDef;

#[test]
#[cfg(feature = "xz")]
#[cfg(feature = "__test_unsquashfs")]
fn test_unsquashfs_cli() {
    const FILE_NAME: &str = "870D97.squashfs";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "a73325883568ba47eaa5379c7768ded5661d61841a81d6c987371842960ac6a2".to_string(),
        url: format!("wcampbell.dev/squashfs/testing/test_re815xev1/{FILE_NAME}"),
    }];
    const TEST_PATH: &str = "test-assets/test_re815_xev160";

    common::download_backoff(&asset_defs, TEST_PATH);
    let image_path = format!("{TEST_PATH}/{FILE_NAME}");

    // single file
    let cmd = common::get_base_command("unsquashfs-backhand")
        .env("RUST_LOG", "none")
        .args(["--path-filter", r#"/usr/bin/wget"#, "-l", "--quiet", &image_path])
        .unwrap();
    cmd.assert().stdout(
        r#"/
/usr
/usr/bin
/usr/bin/wget
"#,
    );

    // multiple file
    let cmd = common::get_base_command("unsquashfs-backhand")
        .env("RUST_LOG", "none")
        .args(["--path-filter", r#"/www/webpages/data"#, "-l", "--quiet", &image_path])
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

    // stat
    //
    // the following is squashfs-tools/unsquashfs -s
    // Found a valid SQUASHFS 4:0 superblock on test-assets/test_re815_xev160/870D97.squashfs.
    // Creation or last append time Fri Sep  2 07:26:23 2022
    // Filesystem size 19957138 bytes (19489.39 Kbytes / 19.03 Mbytes)
    // Compression xz
    // Block size 131072
    // Filesystem is exportable via NFS
    // Inodes are compressed
    // Data is compressed
    // Uids/Gids (Id table) are compressed
    // Fragments are compressed
    // Always-use-fragments option is not specified
    // Xattrs are compressed
    // Duplicates are removed
    // Number of fragments 169
    // Number of inodes 1828
    // Number of ids 1
    // Number of xattr ids 0
    let cmd = common::get_base_command("unsquashfs-backhand")
        .env("RUST_LOG", "none")
        .args(["-s", "--quiet", &image_path])
        .unwrap();
    cmd.assert().stdout(
        r#"SuperBlock {
    magic: [
        0x000068,
        0x000073,
        0x000071,
        0x000073,
    ],
    inode_count: 0x000724,
    mod_time: 0x6311e85f,
    block_size: 0x020000,
    frag_count: 0x0000a9,
    compressor: Xz,
    block_log: 0x000011,
    flags: 0x0000c0,
    id_count: 0x000001,
    version_major: 0x000004,
    version_minor: 0x000000,
    root_inode: 0x3c6e1276,
    bytes_used: 0x1308592,
    id_table: 0x130858a,
    xattr_table: 0xffffffffffffffff,
    inode_table: 0x12fec8c,
    dir_table: 0x1302d9c,
    frag_table: 0x13076e0,
    export_table: 0x1308574,
}
Compression Options: None
flag: data has been deduplicated
flag: nfs export table exists
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
    common::download_backoff(&asset_defs, TEST_PATH);
    let image_path = format!("{TEST_PATH}/{FILE_NAME}");

    let tmp_dir = tempdir().unwrap();
    {
        let cmd = common::get_base_command("unsquashfs-backhand")
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
