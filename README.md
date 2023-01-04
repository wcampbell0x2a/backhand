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
```toml, ignore
[dependencies]
backhand = "0.3.0"
```
### Reading + Writing Firmware
```rust, ignore
use std::fs::File;
use std::env::args;
use backhand::Squashfs;

// read
let file = File::open("file.squashfs").unwrap();
let squashfs = Squashfs::from_reader(file).unwrap();
let filesystem = squashfs.into_filesystem().unwrap();

// write
let bytes = filesystem.to_bytes().unwrap();
```

### Modifying Firmware
```rust, ignore
// add files
let d = FilesystemHeader::default();
filesystem.push_file("Fear is the mind-killer.", "a/d/e/new_file", d);
filesystem.push_file("It is by will alone I set my mind in motion.", "root_file", d);

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

### unsquashfs
```console
Usage: unsquashfs [OPTIONS] <INPUT> <COMMAND>

Commands:
  extract-all    Extract all files(Symlink/Files/Dirs) from image
  help           Print this message or the help of the given subcommand(s)

Arguments:
  <INPUT>  squashfs file

Options:
  -o, --offset <OFFSET>  [default: 0]
  -h, --help             Print help information
  -V, --version          Print version information
```
### add
```console
Binary to add file to squashfs filesystem

Usage: add <INPUT> <FILE> <FILE_PATH>

Arguments:
  <INPUT>      Squashfs file
  <FILE>
  <FILE_PATH>

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```
