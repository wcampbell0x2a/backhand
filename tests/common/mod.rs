use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::tempdir;

pub fn test_unsquashfs(control: &str, new: &str, control_offset: Option<u64>) {
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

pub fn test_unsquashfs_list(control: &str, new: &str, control_offset: Option<u64>) {
    let output_control = Command::new("unsquashfs")
        .args([
            "-l",
            "-o",
            &control_offset.unwrap_or(0).to_string(),
            // we don't run as root, avoid special file errors
            "-ignore-errors",
            "-no-exit-code",
            control,
        ])
        .output()
        .unwrap();

    let output = Command::new("unsquashfs")
        .args([
            "-l",
            // we don't run as root, avoid special file errors
            "-ignore-errors",
            "-no-exit-code",
            new,
        ])
        .output()
        .unwrap();

    assert_eq!(output_control, output);
}
