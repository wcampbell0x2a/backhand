mod common;

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::tempdir;
use test_log::test;

#[test]
#[cfg(feature = "xz")]
fn test_add() {
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::prelude::PermissionsExt;

    use backhand::DEFAULT_BLOCK_SIZE;
    use nix::sys::stat::utimes;
    use nix::sys::time::TimeVal;

    common::download_asset("test_05");
    let image_path = "test-assets/test_05/out.squashfs";

    // Add /test dir
    // ./target/release/add test-assets/test_05/out.squashfs /test --dir --gid 4242 --mtime 1 --uid 2 --mode 511 -o $tmp/out
    let tmp_dir = tempdir().unwrap();
    let cmd = common::get_base_command("add-backhand")
        .env("RUST_LOG", "none")
        .args([
            &image_path,
            "/test",
            tmp_dir.path().join("out").to_str().unwrap(),
            "--dir",
            "--gid",
            "4242",
            "--mtime",
            "60",
            "--uid",
            "2",
            "--mode",
            "777",
        ])
        .unwrap();
    cmd.assert().code(0);

    let mut file = File::create(tmp_dir.path().join("file").to_str().unwrap()).unwrap();
    file.write_all(b"nice").unwrap();
    let mut file = File::create(tmp_dir.path().join("big_file").to_str().unwrap()).unwrap();
    file.write_all(&[b'a'; DEFAULT_BLOCK_SIZE as usize * 2]).unwrap();

    let metadata = file.metadata().unwrap();
    let mut permissions = metadata.permissions();

    permissions.set_mode(0o644);
    let timeval = TimeVal::new(60 * 2, 0);
    utimes(tmp_dir.path().join("file").to_str().unwrap(), &timeval, &timeval).unwrap();

    // We can't really test gid and uid, just trust me it works reading from the --file

    let cmd = common::get_base_command("add-backhand")
        .env("RUST_LOG", "none")
        .args([
            tmp_dir.path().join("out").to_str().unwrap(),
            "/test/new",
            tmp_dir.path().join("out1").to_str().unwrap(),
            "--file",
            tmp_dir.path().join("file").to_str().unwrap(),
            "--gid",
            "2",
            "--uid",
            "4242",
            //"--mtime",
            //"120",
        ])
        .unwrap();
    cmd.assert().code(0);

    let cmd = common::get_base_command("add-backhand")
        .env("RUST_LOG", "none")
        .args([
            tmp_dir.path().join("out1").to_str().unwrap(),
            "/test/big_file",
            tmp_dir.path().join("out2").to_str().unwrap(),
            "--file",
            tmp_dir.path().join("big_file").to_str().unwrap(),
            "--gid",
            "2",
            "--uid",
            "4242",
            "--mtime",
            "120",
        ])
        .unwrap();
    cmd.assert().code(0);

    #[cfg(feature = "__test_unsquashfs")]
    {
        let dir = tmp_dir.path().join("out2");
        let output = Command::new("unsquashfs")
            .args(["-lln", "-UTC", dir.to_str().unwrap()])
            .output()
            .unwrap();
        let expected = r#"drwxr-xr-x 1000/1000                36 2022-10-14 03:02 squashfs-root
drwxr-xr-x 1000/1000                24 2022-10-14 03:02 squashfs-root/b
drwxr-xr-x 1000/1000                24 2022-10-14 03:03 squashfs-root/b/c
-rw-r--r-- 1000/1000                39 2022-10-14 03:03 squashfs-root/b/c/d
dr----x--t 2/4242                   42 1970-01-01 00:01 squashfs-root/test
-rw-r--r-- 4242/2               262144 1970-01-01 00:02 squashfs-root/test/big_file
-rw-r--r-- 4242/2                    4 1970-01-01 00:02 squashfs-root/test/new
"#;

        // using contains here, the output of squashfs varies between versions
        assert_eq!(std::str::from_utf8(&output.stdout).unwrap(), expected);
    }
}

#[test]
#[cfg(feature = "xz")]
fn test_dont_emit_compression_options() {
    use std::fs::File;
    use std::io::Write;

    common::download_asset("test_08");
    let image_path = "test-assets/test_08/out.squashfs";
    let tmp_dir = tempdir().unwrap();

    let mut file = File::create(tmp_dir.path().join("file").to_str().unwrap()).unwrap();
    file.write_all(b"nice").unwrap();

    // with compression option
    let out_image = tmp_dir.path().join("out-comp-options").display().to_string();
    let cmd = common::get_base_command("add-backhand")
        .env("RUST_LOG", "none")
        .args([
            &image_path,
            "/new",
            &out_image,
            "--file",
            tmp_dir.path().join("file").to_str().unwrap(),
            "--no-compression-options",
        ])
        .unwrap();
    cmd.assert().code(0);

    let cmd = common::get_base_command("unsquashfs-backhand")
        .env("RUST_LOG", "none")
        .args(["-s", "--quiet", "--kind", "le_v4_0", &out_image])
        .unwrap();
    let stdout = std::str::from_utf8(&cmd.stdout).unwrap();
    stdout.contains("Compression Options: None");

    // with no compression option
    let out_image = tmp_dir.path().join("out-no-comp-options").display().to_string();
    let cmd = common::get_base_command("add-backhand")
        .env("RUST_LOG", "none")
        .args([
            &image_path,
            "/new",
            &out_image,
            "--file",
            tmp_dir.path().join("file").to_str().unwrap(),
        ])
        .unwrap();
    cmd.assert().code(0);

    let cmd = common::get_base_command("unsquashfs-backhand")
        .env("RUST_LOG", "none")
        .args(["-s", "--quiet", "--kind", "le_v4_0", &out_image])
        .unwrap();
    let stdout = std::str::from_utf8(&cmd.stdout).unwrap();
    stdout.contains("Compression Options: Some");
}
