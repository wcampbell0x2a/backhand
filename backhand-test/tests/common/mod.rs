use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::tempdir;
use tempfile::tempdir_in;

/// test the new squashfs vs the original squashfs with squashfs-tool/unsquashfs
/// by extract
pub fn test_squashfs_tools_unsquashfs(
    control: &str,
    new: &str,
    control_offset: Option<u64>,
    assert_success: bool,
) {
    let control_dir = tempdir_in(".").unwrap();
    let mut cmd = Command::new("unsquashfs");
    let cmd = cmd.args([
        "-d",
        control_dir.path().join("squashfs-root-rust").to_str().unwrap(),
        "-o",
        &control_offset.unwrap_or(0).to_string(),
        // we don't run as root, avoid special file errors
        "-ignore-errors",
        //"-no-exit-code",
        control,
    ]);

    // For older version of squashfs-tools that the cross-rs/cross projects uses,
    // we can't using new -no-exit-code option in unsquashfs, so for the images
    // that contain /dev devices we can't assert the success of unsquashfs.
    if assert_success {
        cmd.assert().code(&[0] as &[i32]);
    } else {
        cmd.assert();
    }

    let new_dir = tempdir_in(".").unwrap();
    let mut cmd = Command::new("unsquashfs");
    let cmd = cmd.args([
        "-d",
        new_dir.path().join("squashfs-root-rust").to_str().unwrap(),
        "-o",
        &control_offset.unwrap_or(0).to_string(),
        // we don't run as root, avoid special file errors
        "-ignore-errors",
        //"-no-exit-code",
        new,
    ]);

    if assert_success {
        cmd.assert().code(&[0] as &[i32]);
    } else {
        cmd.assert();
    }

    let d = dir_diff::is_different(
        control_dir.path().join("squashfs-root-rust").to_str().unwrap(),
        new_dir.path().join("squashfs-root-rust").to_str().unwrap(),
    );
    assert!(!d.expect("couldn't compare dirs"));
}

// Test that both our unsquashfs and unsquashfs both extract to the same
pub fn test_bin_unsquashfs(file: &str, file_offset: Option<u64>, assert_success: bool) {
    let tmp_dir = tempdir().unwrap();
    // Run "our" unsquashfs against the control
    let cmd = get_base_command("unsquashfs-backhand")
        .env("RUST_LOG", "none")
        .args([
            "-d",
            tmp_dir.path().join("squashfs-root-rust").to_str().unwrap(),
            "-o",
            &file_offset.unwrap_or(0).to_string(),
            file,
        ])
        .unwrap();
    tracing::info!("{:?}", cmd);
    cmd.assert().code(&[0] as &[i32]);

    // only squashfs-tools/unsquashfs when x86_64
    #[cfg(feature = "__test_unsquashfs")]
    {
        let mut cmd = Command::new("unsquashfs");
        let cmd = cmd.args([
            "-d",
            tmp_dir.path().join("squashfs-root-c").to_str().unwrap(),
            "-o",
            &file_offset.unwrap_or(0).to_string(),
            // we don't run as root, avoid special file errors
            "-ignore-errors",
            //"-no-exit-code",
            file,
        ]);

        // For older version of squashfs-tools that the cross-rs/cross projects uses,
        // we can't using new -no-exit-code option in unsquashfs, so for the images
        // that contain /dev devices we can't assert the success of unsquashfs.
        if assert_success {
            cmd.assert().code(&[0] as &[i32]);
        } else {
            cmd.assert();
        }
        tracing::info!("{:?}", cmd);

        let d = dir_diff::is_different(
            tmp_dir.path().join("squashfs-root-rust"),
            tmp_dir.path().join("squashfs-root-c"),
        );
        // remove the followig comment to keep around tmp dirs
        // let _ = tmp_dir.into_path();
        assert!(!d.expect("couldn't compare dirs"));
    }
}

fn find_runner() -> Option<String> {
    for (key, value) in std::env::vars() {
        if key.starts_with("CARGO_TARGET_") && key.ends_with("_RUNNER") && !value.is_empty() {
            return Some(value);
        }
    }
    None
}

/// Under cargo cross (qemu), find runner
pub fn get_base_command(base: &str) -> Command {
    let path = assert_cmd::cargo::cargo_bin(base);

    let mut cmd;
    if let Some(runner) = find_runner() {
        let mut runner = runner.split_whitespace();
        cmd = Command::new(runner.next().unwrap());
        for arg in runner {
            cmd.arg(arg);
        }
        cmd.arg(path);
    } else {
        cmd = Command::new(path);
    }
    cmd
}
