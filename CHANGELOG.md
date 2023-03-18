# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
### Added
- Append no padding to image: `FilesystemWriter::set_no_padding()`
- Modify FilesystemCompressor options: `FilesystemCompressor::options(&mut self, options: CompressionOptions)`

## [v0.11.0] - 2023-03-14
### Added
- Support for Read/Write of non-standard custom squashfs images:
    - `LE_V4_0`: (linux kernel) Little-Endian default official v4.0
    - `BE_V4_0`: Big-Endian v4.0
    - `AVM_BE_V4_0`: AVM Fritz!OS firmware support.
- `FilesystemWriter`: Builder pattern used when mutating an image. This includes multiple functions
   for the public API. Supporting both raw images and modification made to images that already exist.
- `FilesytemCompressor`: `.compressor` is now `FilesystemCompressor`,
   which holds the Id as well as options stored in the image as well as extra options only used when
   compressing when creating a new image.
- Add error `InvalidCompressionOption`
- Change default XZ compression level to 6
- Support custom XZ filters for `FilesystemWriter`
- Return `(Superblock, bytes_written)` for `FilesystemWriter::write()`
- Update deku to 0.16.0
- `add`: now reads file details to derive the details when the file is added the image
- `add`: `--mtime`, `--uid`, `--gid` and `--permission` to override file details derived from file
- `unsquashfs`: now correctly extracts ownership and permission details
### Fixed
- `ID` now supports multiple IDs for GUI and UID in the table
- `id_table` is now properly a u64 pointer
- Data is now *not* copied when during the use of a `FilesystemWriter` you decide to change the compression used.
  Thanks [@rbran](https://github.com/rbran/)
### Changed
- Renamed `SquashfsError` to `BackhandError`

## [v0.10.1] - 2023-02-22
### Added
- Zstd compression support 

### Fixed
- `FilesystemWriter` Debug impl now works
- `FilesystemReader::from_reader_with_offset(..)` now properly respects given offsets
- `FilesystemWriter::write_with_offset(..)` now properly respects given offsets

## [v0.10.0] - 2023-02-20
### Added
- Fuzz testing with `cargo fuzz`. Mostly fuzz bytes as bytes/image input into this library.
- `unsquashfs`: Add `-o, --out <OUT>` flag for output squashfs image destination
- `replace`: Add binary to replace file in squashfs filesystems
- Add support for Lzo compression, and feature `lzo`

### Fixed
- Fixed many issues found with fuzz testing related to legal images.
  Checks are now added at every stop possible to prevent many soundness issues.
- Fixed `Compressor` id values for Lzo and Lzma

### Changed
- Pass internal raw data by reference, improving `only_read` benchmarks by ~9%.
- Invalid `Superblock.block_size` is now checked against MiB(1) instead of MB(1)

## [v0.9.1] - 2023-02-16
### Fixed
- Fix `unsquashfs` extracting wrong file data

## [v0.9.0] - 2023-02-13
### Fixed
- `FilesystemWriter::push_file(..)` correctly enters file into filesystem
### Changed
- Remove Result return type from `FilesystemWriter::{push_file(..), push_dir(..), push_symlink(..), push_char_device(..) and push_block_devivce(..)`.
- Remove unused errors: `FieldNotInitialized` and `OsStringToStr`.

## [v0.8.1] - 2023-02-11
- Fix `src/lib.rs` version for docs.rs

## [v0.8.0] - 2023-02-11
### Added
- unsquashfs: Add `--stat`, `--force`, `--info` flags.
- unsquashfs: Add support for Char and Block device file creation when superuser.
- features: `xz` and `gzip`. By default both are enabled, but conditionally you may compile only one type of decompressor.
- `SquashfsError::Unreachable`, `SquashfsError::UnexpectedInode`, `SquashfsError::UnsupportedInode`.
  These are all returned by the public API of filesystem and more panics were removed.

### Fixed
- `inode_count` is fixed, previously was +1 the actual inode count.

### Changed
- The Public API of the library has been condensed, lmk if you have lost access to a required struct/field/enum.
- Add `FilesystemReader` and `FilesystemWriter` for lazy-reading the files only when required.
  This significantly speeds up the initial read of the filesystem and splits the reading of the filesystem and the writing of the filesystem.
  The following diff will cover most common API upgrades from `v0.7.0`
  ```diff
  -let squashfs = Squashfs::from_reader(file).unwrap();
  -let mut filesystem = squashfs.into_filesystem().unwrap();
  +let filesystem = FilesystemReader::from_reader(file).unwrap();
  +let mut filesystem = FilesystemWriter::from_fs_reader(&filesystem).unwrap();
  ```
  ```diff
  -let filesystem = Filesystem::from_reader(file).unwrap();
  +let filesystem = FilesystemReader::from_reader(file).unwrap();
  +let mut filesystem = FilesystemWriter::from_fs_reader(&filesystem).unwrap();
  ```

  ```diff
  -FilesystemHeader
  +NodeHeader
  ```

### Performance
This releases allows massive performance improvements by only reading files from disk when required 
and reducing the amount of memory required to read and write an image.

Thanks [@rbran](https://github.com/rbran/) for the incredible work on the performance of the library.

Before:
```
read/write/netgear_ax6100v2
                        time:   [2.3553 s 2.3667 s 2.3775 s]
read/write/tplink_ax1800
                        time:   [17.996 s 18.068 s 18.140 s]
```

After:
```
write_read/netgear_ax6100v2
                        time:   [1.2291 s 1.2363 s 1.2433 s]
write_read/tplink_ax1800
                        time:   [6.7506 s 6.8287 s 6.9349 s]
only_read/netgear_ax6100v2
                        time:   [5.1153 ms 5.1234 ms 5.1305 ms]
only_read/tplink_ax1800 
                        time:   [22.383 ms 22.398 ms 22.415 ms]
```

## [v0.7.0] - 2023-01-23
### Added
- Use `block_size` as XZ default `dict_size` when compressing data
- Add `Filesystem::push_symlink(..)`
- Add `Filesystem::push_dir(..)`
- Add `Filesystem::push_char_device(..)`
- Add `Filesystem::push_block_device(..)`

### Fixed
- Correctly choose between storing uncompressed and compressed data on which takes the least space

### Changed
- Improve `unsquashfs` and `add` cli args to match `squashfs-tools/unsquashfs` cli
- `Filesystem::push_file(..)` now takes for bytes anything that is `into Read` instead of `into Vec<u8>`
- `Node::Path` renamed to `Node::Dir`
- `SquashfsPath` renamed to `SquashfsDir`
- `Filesystem::from_reader(..)`, `R` now takes `Read + Seek` instead our own `ReadSeek`
- `Filesystem::from_reader_with_offset(..)`, `R` now takes `Read + Seek` instead our own `ReadSeek`
- `Filesystem::push_symlink(..)` now only needs `path` and `link`

## [v0.6.0] - 2023-01-10
- Fix bug in our filesystem tree causing directory header information (gui, uid, permissions)
  to not be saved in resulting filesystem when calling `Filesystem::to_bytes(..)`.
- Rework `filesystem::Node` to be a struct containing the path and `InnerNode`.
  This cleans up the inner implementation of the file system tree.
- Make more types public that are useful for Squashfs detailed introspection
- Improve documentation

## [v0.5.0] - 2023-01-08
- Fix warning when compression options isn't standard size
- In `from_reader(..)`, show info about flags used
- Add `Filesystem::from_reader(..)` and `Filesystem::from_reader_with_offset(..)`
  which calls `Squashfs::from_reader(..)` and `Squashfs::from_reader_with_offset(..)` and `Squashfs::into_filesystem(..)`.
- 5% Performance increases due to using `Vec::with_capacity(..)` for `fragment_bytes`
- Add Block and Char Device support for Reading and Writing
- Fix error with `inode_offset` mis-calculation
- Fix tail-end fragment support for reading image
- Fix `unsquashfs` file path extraction

## [v0.4.0] - 2023-01-04
- Add `mod_time` from `Squashfs` to `Filesystem` used in creation of new image with `to_bytes(..)`

## [v0.3.0] - 2023-01-03
- Restrict public API
- Improve docs
- Add `Filesystem::push_file(..)` for adding a file, as well as the dirs for the path
- Add `Filesystem::mut_file(..)` for mutating a file at a path already in the filesystem

## [v0.2.1] - 2023-01-02
- Fix Cargo.toml issues

## [v0.2.0] - 2023-01-02
- Add `block_size` and `block_log` to Filesystem. Automatically taken from `Squashfs` when using `into_filesystem()`
- Add support for data fragments for `filesystem::to_bytes()`
- `DirEntry` uses `InodeId` instead of `u8`

## [v0.1.0] - 2023-01-01
- Initial Release
