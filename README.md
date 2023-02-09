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
backhand = "0.7.0"
```
### Reading/Writing/Modifying Firmware
```rust,no_run
use std::fs::File;
use std::io::Cursor;
use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};

// read
let file = File::open("file.squashfs").unwrap();
let read_filesystem = FilesystemReader::from_reader(file).unwrap();

// convert to writer
let mut write_filesystem = FilesystemWriter::from_fs_reader(&read_filesystem).unwrap();

// add file with data from slice
let d = NodeHeader::default();
let bytes = Cursor::new(b"Fear is the mind-killer.");
write_filesystem.push_file(bytes, "a/d/e/new_file", d);

// add file with data from file
let new_file = File::open("dune").unwrap();
write_filesystem.push_file(new_file, "/root/dune", d);

// modify file
let bytes = Cursor::new(b"The sleeper must awaken.\n");
write_filesystem.replace_file("/a/b/c/d/e/first_file", bytes).unwrap();

// write into a new file
let mut output = File::create("modified.squashfs").unwrap();
write_filesystem.write(&mut output).unwrap();
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
  -i, --info             Print files as they are extracted
  -f, --force            If file already exists then overwrite
  -s, --stat             Display filesystem superblock information
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
