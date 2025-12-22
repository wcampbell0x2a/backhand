mod common;

use assert_cmd::prelude::*;

#[test]
#[cfg(feature = "xz")]
#[cfg(feature = "__test_unsquashfs")]
fn test_unsquashfs_cli() {
    common::download_asset("re815xe");
    let image_path = "test-assets/test_re815_xev160/870D97.squashfs";

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
        .args(["-s", "--quiet", "--kind", "le_v4_0", &image_path])
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

    common::download_asset("openwrt_tplink_archera7v5");
    let image_path = "test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin";

    let tmp_dir = tempdir().unwrap();
    {
        let cmd = common::get_base_command("unsquashfs-backhand")
            .env("RUST_LOG", "none")
            .args([
                "--auto-offset",
                "--kind",
                "le_v4_0",
                "-d",
                tmp_dir.path().join("squashfs-root-c").to_str().unwrap(),
                &image_path,
            ])
            .unwrap();
        cmd.assert().code(&[0] as &[i32]);
    }
}
