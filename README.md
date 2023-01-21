backhand
===============================

[<img alt="github" src="https://img.shields.io/badge/github-wcampbell0x2a/backhand-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/wcampbell0x2a/backhand)
[<img alt="crates.io" src="https://img.shields.io/crates/v/backhand.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/backhand)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-backhand-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/backhand)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/wcampbell0x2a/backhand/main.yml?branch=master&style=for-the-badge" height="20">](https://github.com/wcampbell0x2a/backhand/actions?query=branch%3Amaster)

Library and binaries for the reading, creating, and modification
of [SquashFS](https://en.wikipedia.org/wiki/SquashFS) file systems.

| :warning: WARNING                                                                          |
|:-------------------------------------------------------------------------------------------|
| The API for this library **isn't** complete. I will most likely break this for improvements |

## Library
Add the following to your `Cargo.toml` file:
```toml
[dependencies]
backhand = "0.6.0"
```
### Reading/Writing/Modifying Firmware
```rust, ignore
use std::fs::File;
use backhand::{Filesystem, FilesystemHeader};

// read
let file = File::open("file.squashfs").unwrap();
let mut filesystem = Filesystem::from_reader(file).unwrap();

// add file with data from bytes
let d = FilesystemHeader::default();
let mut bytes = Cursor::new("Fear is the mind-killer.");
filesystem.push_file(&mut bytes, "a/d/e/new_file", d);

// add file with data from file
let mut new_file = File::open("dune").unwrap();
filesystem.push_file(&mut new_file, "/root/dune", d);

// modify file
let file = filesystem.mut_file("/a/b/c/d/e/first_file").unwrap();
file.bytes = b"The sleeper must awaken.\n".to_vec();

// write
let bytes = filesystem.to_bytes().unwrap();
```

## Testing
This library is tested with unpacking and packing SquashFS firmwares and testing that result with `unsquashfs`.
`openwrt` binaries are primarily being tested.

## Binaries
These are currently under development and are missing features, MR's welcome!

To install, run `cargo install backhand`.

### unsquashfs
```console
tool to uncompress, extract and list squashfs filesystems

Usage: unsquashfs [OPTIONS] <FILESYSTEM>

Arguments:
  <FILESYSTEM>  Squashfs file

Options:
  -o, --offset <BYTES>   Skip BYTES at the start of FILESYSTEM [default: 0]
  -l, --list             List filesystem, do not write to DEST
  -d, --dest <PATHNAME>  Extract to [PATHNAME] [default: squashfs-root]
  -h, --help             Print help information
  -V, --version          Print version information
```
### add
```console
tool to add files to squashfs filesystems

Usage: add <FILESYSTEM> <FILE> <FILE_PATH>

Arguments:
  <FILESYSTEM>  Squashfs file
  <FILE>
  <FILE_PATH>

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```
