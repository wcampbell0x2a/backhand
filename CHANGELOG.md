# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
- Fix bug in our filesystem tree causing directory header information (gui, uid, permissions)
  to not be saved in resulting filesystem when calling `Filesystem::to_bytes(..)`.

## [v0.5.0] - 2021-01-08
- Fix warning when compression options isn't standard size
- In `from_reader(..)`, show info about flags used
- Add `Filesystem::from_reader(..)` and `Filesystem::from_reader_with_offset(..)`
  which calls `Squashfs::from_reader(..)` and `Squashfs::from_reader_with_offset(..)` and `Squashfs::into_filesystem(..)`.
- 5% Performance increases due to using `Vec::with_capacity(..)` for `fragment_bytes`
- Add Block and Char Device support for Reading and Writing
- Fix error with `inode_offset` mis-calculation
- Fix tail-end fragment support for reading image
- Fix `unsquashfs` file path extraction

## [v0.4.0] - 2021-01-04
- Add `mod_time` from `Squashfs` to `Filesystem` used in creation of new image with `to_bytes(..)`

## [v0.3.0] - 2021-01-03
- Restrict public API
- Improve docs
- Add `Filesystem::push_file(..)` for adding a file, as well as the dirs for the path
- Add `Filesystem::mut_file(..)` for mutating a file at a path already in the filesystem

## [v0.2.1] - 2021-01-02
- Fix Cargo.toml issues

## [v0.2.0] - 2021-01-02
- Add `block_size` and `block_log` to Filesystem. Automatically taken from `Squashfs` when using `into_filesystem()`
- Add support for data fragments for `filesystem::to_bytes()`
- `DirEntry` uses `InodeId` instead of `u8`

## [v0.1.0] - 2021-01-01
- Initial Release
