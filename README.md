backhand
===============================

[<img alt="github" src="https://img.shields.io/badge/github-wcampbell0x2a/backhand-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/wcampbell0x2a/backhand)
[<img alt="crates.io" src="https://img.shields.io/crates/v/backhand.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/backhand)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-backhand-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/backhand)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/wcampbell0x2a/backhand/main.yml?branch=master&style=for-the-badge" height="20">](https://github.com/wcampbell0x2a/backhand/actions?query=branch%3Amaster)
[<img alt="Codecov" src="https://img.shields.io/codecov/c/github/wcampbell0x2a/backhand?style=for-the-badge" height="20">](https://app.codecov.io/gh/wcampbell0x2a/backhand)

Library and binaries for the reading, creating, and modification
of [SquashFS](https://en.wikipedia.org/wiki/SquashFS) file systems.

- **Library** — Backhand provides an easy way for programmatic analysis of Squashfs 4.0 images,
including the extraction and modification of images.
- **Feature Flags** — Supported compression and decompression are feature flagged, so your final binary (or `unsquashfs`)
only needs to include code to extract one type of image.
- **Unconventional Support** — As well as supporting normal linux kernel SquashFS 4.0, we also support
the "wonderful world of vendor formats" with a `Kind` struct.
This allows changing the magic bytes, custom compression algorithms, and the Endian-ness of either the Data or Metadata fields.


## Library
*Compiler support: requires rustc 1.75+*

Add the following to your `Cargo.toml` file:
```toml
[dependencies]
backhand = "0.20.0"
```

#### Target Support
Although additional targets may be supported, only the following have been fully tested or confirmed to build successfully.

| Target                                 | `build` | `test` |
|----------------------------------------|:-------:|:------:|
| `x86_64-unknown-linux-musl`            | ✓       | ✓      |
| `aarch64-unknown-linux-musl`           | ✓       | ✓      |
| `arm-unknown-linux-musleabi`           | ✓       | ✓      |
| `armv7-unknown-linux-musleabi`         | ✓       | ✓      |
| `aarch64-unknown-linux-musl`           | ✓       | ✓      |
| `x86_64-apple-darwin`                  | ✓       | ✓      |
| `aarch64-apple-darwin`                  | ✓       | ✓      |
| `x86_64-pc-windows-gnu`                | ✓       |        |

### Reading/Writing/Modifying Firmware
```rust,no_run
use std::fs::File;
use std::io::{Cursor, BufReader};
use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};

// read
let file = BufReader::new(File::open("file.squashfs").unwrap());
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

## Binaries
*Compiler support: requires rustc 1.77+*

These are currently under development and are missing features, MR's welcome!

To install, run `cargo install backhand-cli --locked`, or download from the
[latest github release](https://github.com/wcampbell0x2a/backhand/releases/latest).

See ``--help`` for more information.

#### Target Support
Although additional targets may be supported, only the following have been tested and included in our GitHub releases.

| Target                                 | `test`    | `release` |
|----------------------------------------|:---------:|:---------:|
| `x86_64-unknown-linux-musl`            | ✓         | ✓         |
| `aarch64-unknown-linux-musl`           | ✓         | ✓         |
| `arm-unknown-linux-musleabi`           | ✓         | ✓         |
| `armv7-unknown-linux-musleabi`         | ✓         | ✓         |
| `aarch64-unknown-linux-musl`           | ✓         | ✓         |
| `x86_64-apple-darwin`                  | ✓         | ✓         |
| `aarch64-apple-darwin`                 | ✓         | ✓         |

### unsquashfs-backhand
```no_test
tool to uncompress, extract and list squashfs filesystems

Usage: unsquashfs-backhand [OPTIONS] [FILESYSTEM]

Arguments:
  [FILESYSTEM]  Squashfs file

Options:
  -o, --offset <BYTES>             Skip BYTES at the start of FILESYSTEM [default: 0]
  -a, --auto-offset                Find first instance of squashfs --kind magic
  -l, --list                       List filesystem, do not write to DEST (ignores --quiet)
  -d, --dest <PATHNAME>            Extract to [PATHNAME] [default: squashfs-root]
  -i, --info                       Print files as they are extracted
      --path-filter <PATH_FILTER>  Limit filesystem extraction [default: /]
  -f, --force                      If file already exists then overwrite
  -s, --stat                       Display filesystem superblock information (ignores --quiet)
  -k, --kind <KIND>                Kind(type of image) to parse [default: le_v4_0] [possible
                                   values: be_v4_0, le_v4_0, avm_be_v4_0]
      --completions <COMPLETIONS>  Emit shell completion scripts [possible values: bash, elvish,
                                   fish, powershell, zsh]
      --quiet                      Silence all progress bar and RUST_LOG output
  -h, --help                       Print help (see more with '--help')
  -V, --version                    Print version
```

### add-backhand
```no_test
tool to add a file or directory to squashfs filesystems

Usage: add-backhand [OPTIONS] <INPUT_IMAGE> <FILE_PATH_IN_IMAGE> <OUTPUT_IMAGE>

Arguments:
  <INPUT_IMAGE>         Squashfs input image
  <FILE_PATH_IN_IMAGE>  Path of file once inserted into squashfs
  <OUTPUT_IMAGE>        Squashfs output image path

Options:
  -d, --dir                     Create empty directory
  -f, --file <FILE>             Path of file to read, to write into squashfs
      --mode <MODE>             Override mode read from <FILE>
      --uid <UID>               Override uid read from <FILE>
      --gid <GID>               Override gid read from <FILE>
      --mtime <MTIME>           Override mtime read from <FILE>
      --pad-len <PAD_LEN>       Custom KiB padding length
      --no-compression-options  Don't emit compression options
  -h, --help                    Print help
  -V, --version                 Print version
```

### replace-backhand
```no_test
tool to replace files in squashfs filesystems

Usage: replace-backhand [OPTIONS] <INPUT_IMAGE> <FILE> <FILE_PATH_IN_IMAGE> <OUTPUT_IMAGE>

Arguments:
  <INPUT_IMAGE>         Squashfs input image
  <FILE>                Path of file to read, to write into squashfs
  <FILE_PATH_IN_IMAGE>  Path of file replaced in image
  <OUTPUT_IMAGE>        Squashfs output image

Options:
      --pad-len <PAD_LEN>       Custom KiB padding length
      --no-compression-options  Don't emit compression options
  -h, --help                    Print help
  -V, --version                 Print version
```

## Performance
See [BENCHMARK.md](BENCHMARK.md).

## Testing
See [backhand-test](backhand-test/README.md).
