# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
### Added
### Fixed
- `inode_count` is fixed, previously was +1 the actual inode count.
### Changed
- Add `FilesystemReader` and `FilesystemWriter` for lazy-reading the files only when required.
  This speeds up the initial read of the filesystem and splits the reading of the filesystem and the writing of the filesystem.
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

  Thanks [@rbran](https://github.com/rbran/) for the MR!

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
