# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Dependencies
- Bump `actions/upload-artifact` from 4.1.0 to 4.2.0 ([#435](https://github.com/wcampbell0x2a/backhand/pull/435))

## [v0.14.2] - 2024-01-16
### `backhand`
- Enable overflow-checks ([#421](https://github.com/wcampbell0x2a/backhand/pull/421))
- Add feature `gzip-zune-inflate` to add a decompress only option with speed improvements ([#419](https://github.com/wcampbell0x2a/backhand/pull/419))
- Remove allocation for `impl From<BackhandError> for io::Error {` ([#425](https://github.com/wcampbell0x2a/backhand/pull/425))

### `backhand-cli`
- Enable overflow-checks for dist builds ([#421](https://github.com/wcampbell0x2a/backhand/pull/421))

#### unsquashfs 
- Use feature `gzip-zune-inflate` for dist build and speed improvements ([#419](https://github.com/wcampbell0x2a/backhand/pull/419))
- Updated benchmarks to show improvement ([#419](https://github.com/wcampbell0x2a/backhand/pull/419))

### Dependencies
- Bump `clap` from 4.4.12 to 4.4.17 ([#417](https://github.com/wcampbell0x2a/backhand/pull/417), [#424](https://github.com/wcampbell0x2a/backhand/pull/424))
- Bump `thiserror` from 1.0.53 to 1.0.56 ([#404](https://github.com/wcampbell0x2a/backhand/pull/404))
- Bump `actions/upload-artifact` from 4.0.0 to 4.1.0 ([#423](https://github.com/wcampbell0x2a/backhand/pull/423))
- Bump `libc` from 0.2.151 to 0.2.152 ([#408](https://github.com/wcampbell0x2a/backhand/pull/408))
- Bump `clap_complete` from 4.4.5 to 4.4.7 ([#426](https://github.com/wcampbell0x2a/backhand/pull/426))
- Bump `assert_cmd` from 2.0.12 to 2.0.13 ([#422](https://github.com/wcampbell0x2a/backhand/pull/422))
- Bump `console` from 0.15.7 to 0.15.8 ([#413](https://github.com/wcampbell0x2a/backhand/pull/413))

## [v0.14.1] - 2024-01-13
### `backhand`
#### Changes
- Fix path to project `README.md` for `crates.io` ([#420](https://github.com/wcampbell0x2a/backhand/pull/420))

## [v0.14.0] - 2024-01-13
Major changes were made to the organization of this repo, with the library `backhand` now being separated from
the `backhand-cli` package, which is used to install `unsquashfs`, `replace`, and `add`.
### `backhand`
#### Changes
- Following changes were done to allow multi-threaded applications ([#278](https://github.com/wcampbell0x2a/backhand/pull/278))
  - Change `RefCell<Box<T>>` into `Arc<Mutex<T>>`
  - Change `RefCell<T>` into `Mutex<T>`
  - Change `Rc<T>` into `Arc<T>`
  - Change `dyn CompressionAction` to `dyn CompressionAction + Send + Sync` for `Kind` uses
  - Change `BufReadSeek: BufRead + Seek {}` to `BufReadSeek: BufRead + Seek + Send {}`
- Allow user provided read/write files to not be static ([@rbran](https://github.com/rbran)) ([#285](https://github.com/wcampbell0x2a/backhand/pull/285))
- Bump MSRV to `1.67.1`
- Allow creating and reading uncompressed files ([@rbran](https://github.com/rbran)) ([#365](https://github.com/wcampbell0x2a/backhand/pull/365))
- Allow calling `FilesystemWriter::write` with Owned and RefMut writer ([@rbran](https://github.com/rbran)) ([#361](https://github.com/wcampbell0x2a/backhand/pull/361))
- Push dir, file, etc, with lifetimes unrelated to reader from `from_fs_reader` ([@rbran](https://github.com/rbran)) ([#361](https://github.com/wcampbell0x2a/backhand/pull/361))
For example, the following is now allowed:
```diff
-   let mut output = File::create(&args.out).unwrap();
-   if let Err(e) = filesystem.write(&mut output) {
+   let output = File::create(&args.out).unwrap();
+   if let Err(e) = filesystem.write(output) {
````

#### Bug Fix
- When creating an empty image using `FilesystemWriter::default()`, correctly create the ID table for UID and GID entries. Reported: ([@hwittenborn](https://github.com/hwittenborn)) ([!250](https://github.com/wcampbell0x2a/backhand/issues/275)), Fixed: ([#275](https://github.com/wcampbell0x2a/backhand/pull/275))
- Remove manual `Clone` impl for `FilesystemReaderFile` ([#277](https://github.com/wcampbell0x2a/backhand/pull/277))
- Increase `DirectoryIndex::name_size` length from 100 to 255. ([@eatradish](https://github.com/eatradish)) ([!282](https://github.com/wcampbell0x2a/backhand/issues/282)), Fixed: ([#283](https://github.com/wcampbell0x2a/backhand/pull/283))
- Prevent `push_file` "file within file", will now return `InvalidFilePath` ([@rbran](https://github.com/rbran)) ([#364](https://github.com/wcampbell0x2a/backhand/pull/364))
- Fix `gid` and `uid` for `push_dir_all(..)` ([#360](https://github.com/wcampbell0x2a/backhand/pull/360))

#### Security
- Only allow root and simple filenames into `DirEntry` ([@rbran](https://github.com/rbran)) ([#271](https://github.com/wcampbell0x2a/backhand/pull/271))

### `backhand-cli`
#### Changes to All
- `strip` and `LTO` are enabled for release binaries
- Fix macOS builds ([#260](https://github.com/wcampbell0x2a/backhand/pull/260))
- Bump MSRV to `1.73.0` to use now stabilized `std::os::unix::fs::lchown`
- Add color styling to help output ([#387](https://github.com/wcampbell0x2a/backhand/pull/387))

#### unsquashfs 
- Changed name to `unsquashfs-backhand` ([#356](https://github.com/wcampbell0x2a/backhand/pull/356))
- Add progress bar for a cleaner output when extracting files ([#272](https://github.com/wcampbell0x2a/backhand/pull/272))
- Add `--quiet` for not displaying progress bar and RUST_LOG output ([#272](https://github.com/wcampbell0x2a/backhand/pull/272))
- Add multiple threads for extracing files, giving us the same performance in most cases as `squashfs-tools/unsquashfs`! ([#278](https://github.com/wcampbell0x2a/backhand/pull/278))

#### add
- Changed name to `add-backhand` ([#356](https://github.com/wcampbell0x2a/backhand/pull/356))

#### replace
- Changed name to `replace-backhand` ([#356](https://github.com/wcampbell0x2a/backhand/pull/356))

### ci
- Add testing and release binaries for the following platforms:([#259](https://github.com/wcampbell0x2a/backhand/pull/259))
   - `aarch64-unknown-linux-musl`
   - `arm-unknown-linux-musleabi`
   - `x86_64-unknown-linux-musl` (previously already release supported)
- Testing and release binaries were not added for macOS, support was tested on that platform.
### testing
- Replace curl in test dependency `test-assets` with ureq ([#264](https://github.com/wcampbell0x2a/backhand/pull/264))
- Replace `zune-inflate` with `libdeflater` for custom decompression testing for reliability ([#325](https://github.com/wcampbell0x2a/backhand/pull/325))

### Dependencies
- Bump `flate2` from 1.0.26 to 1.0.28 ([#307](https://github.com/wcampbell0x2a/backhand/pull/307))
- Bump `jemallocator` from 0.5.0 to 0.5.4 ([#305](https://github.com/wcampbell0x2a/backhand/pull/305))
- Bump `env_logger` from 0.10.0 to 0.10.1 ([#341](https://github.com/wcampbell0x2a/backhand/pull/341))
- Bump `clap` from 4.4.7 to 4.4.12 ([#340](https://github.com/wcampbell0x2a/backhand/pull/340), [#371](https://github.com/wcampbell0x2a/backhand/pull/371), [#376](https://github.com/wcampbell0x2a/backhand/pull/376), [#399](https://github.com/wcampbell0x2a/backhand/pull/399))
- Bump `dangoslen/dependabot-changelog-helper` from 3.5.0 to 3.7.0 ([#342](https://github.com/wcampbell0x2a/backhand/pull/342), [#369](https://github.com/wcampbell0x2a/backhand/pull/369))
- Bump `tracing-subscriber` from 0.3.17 to 0.3.18 ([#347](https://github.com/wcampbell0x2a/backhand/pull/347))
- Bump `byte-unit` from 4.0.19 to 5.0.3 ([#367](https://github.com/wcampbell0x2a/backhand/pull/367))
- Bump `actions/labeler` from 4 to 5 ([#377](https://github.com/wcampbell0x2a/backhand/pull/377))
- Bump `test-log` from 0.2.13 to 0.2.14 ([#378](https://github.com/wcampbell0x2a/backhand/pull/378))
- Bump `clap_complete` from 4.4.4 to 4.4.5 ([#393](https://github.com/wcampbell0x2a/backhand/pull/393))
- Bump `thiserror` from 1.0.51 to 1.0.53 ([#391](https://github.com/wcampbell0x2a/backhand/pull/391), [#401](https://github.com/wcampbell0x2a/backhand/pull/401))
- Bump `actions/upload-artifact` from 3.1.3 to 4.0.0 ([#380](https://github.com/wcampbell0x2a/backhand/pull/380))
- Bump `tempfile` from 3.8.1 to 3.9.0 ([#398](https://github.com/wcampbell0x2a/backhand/pull/398))
- Bump `document-features` from 0.2.7 to 0.2.8 ([#400](https://github.com/wcampbell0x2a/backhand/pull/400))

## [v0.13.0] - 2023-06-18
### backhand
#### Changes
- Decrease in memory usage for file reader and write ([#255](https://github.com/wcampbell0x2a/backhand/pull/255))
- Remove unnecessary deconstruction/reconstruction of Vec when reading inodes ([@rbran](https://github.com/rbran)) ([#251](https://github.com/wcampbell0x2a/backhand/pull/251))
- Only store file data compressed if it results in smaller size ([@rbran](https://github.com/rbran)) ([#250](https://github.com/wcampbell0x2a/backhand/pull/250))
- Remove `lzo` being a default feature because of GPL license ([#240](https://github.com/wcampbell0x2a/backhand/pull/240))
- Add support for OpenWRT compression options ([#239](https://github.com/wcampbell0x2a/backhand/pull/239))
- Bump MSRV to `1.65.0` for latest `clap` requirements ([#253](https://github.com/wcampbell0x2a/backhand/pull/253))
#### Bug Fix
- Fix bug in generating Uid and Gid's with `FilesystemWriter`. All internal representation of Gid and Uid are changed to u32 ([#254](https://github.com/wcampbell0x2a/backhand/pull/254))
- Remove case where invalid filesystem root_inode_offset would cause invalid bounds read panic. Found by fuzzer ([#245](https://github.com/wcampbell0x2a/backhand/pull/245))

#### Complete API Updates
```
$ cargo public-api -ss diff v0.12.0..HEAD
```

<details>
<summary>Click to expand</summary>

```diff
Removed items from the public API
=================================
(none)

Changed items in the public API
===============================
-pub fn backhand::FilesystemWriter<'a>::set_root_gid(&mut self, gid: u16)
+pub fn backhand::FilesystemWriter<'a>::set_root_gid(&mut self, gid: u32)
-pub fn backhand::FilesystemWriter<'a>::set_root_uid(&mut self, uid: u16)
+pub fn backhand::FilesystemWriter<'a>::set_root_uid(&mut self, uid: u32)
-pub backhand::NodeHeader::gid: u16
+pub backhand::NodeHeader::gid: u32
-pub backhand::NodeHeader::uid: u16
+pub backhand::NodeHeader::uid: u32
-pub fn backhand::NodeHeader::new(permissions: u16, uid: u16, gid: u16, mtime: u32) -> Self
+pub fn backhand::NodeHeader::new(permissions: u16, uid: u32, gid: u32, mtime: u32) -> Self

Added items to the public API
=============================
+pub backhand::compression::Xz::bit_opts: core::option::Option<u16>
+pub backhand::compression::Xz::fb: core::option::Option<u16>
+pub fn backhand::kind::Kind::magic(&self) -> [u8; 4]
+impl backhand::NodeHeader
+pub fn backhand::NodeHeader::from_inode(inode_header: InodeHeader, id_table: &[backhand::Id]) -> Self
```

</details>

### All binaries
#### Changes
- jemalloc is now used for `-musl` release targets for performance reasons ([#254](https://github.com/wcampbell0x2a/backhand/pull/254))
- `HAVE_DECODER_ARM`, `HAVE_DECODER_ARM64`, and `HAVE_DECODER_ARMTHUMB` filter flags are now defined for xz2. This only effects static build created in our CI. ([#254](https://github.com/wcampbell0x2a/backhand/pull/248))
- Add `RUST_LOG` and available Decompressors to `--help` of all binaries ([#242](https://github.com/wcampbell0x2a/backhand/pull/242))

### add
#### Changes
- Add `--dir` to create a empty directory ([#242](https://github.com/wcampbell0x2a/backhand/pull/242))
#### Bug Fix
- Add correctly reading new file metadata from `--file`, force other arguments for `--dir` ([#254](https://github.com/wcampbell0x2a/backhand/pull/254))

### unsquashfs
#### Changes
- Add `--auto-offset` for automatic finding of initial SquashFS offset in image ([#241](https://github.com/wcampbell0x2a/backhand/pull/241))
- Add possible `kind` values to `--help` output ([#236](https://github.com/wcampbell0x2a/backhand/pull/236))
- Add `--path-filter` to limit file extraction to a path ([#237](https://github.com/wcampbell0x2a/backhand/pull/237))

## [v0.12.0] - 2023-05-07
Thanks [@rbran](https://github.com/rbran/) for the contributions!

### backhand
- `Kind` has been extended to take an `CompressionAction` to have a custom compression and decompression
  algorithm. This defaults to the `DefaultCompressor` in most situations to be like the Linux kernel
  squashfs code. This should allow an ever greater array of custom vendor Squashfs image support.
  Many API changes were done to support this, Most of the following changes focus on the Public API that
  we expect the normal developer to be using.
- Added method to allow creating image without padding: `FilesystemWriter::set_no_padding()`
- Added method to allow modification to Compression options: `FilesystemCompressor::options(&mut self, options: CompressionOptions)`
- Added `FilesytemWriter::push_dir_all`, following behavior of `std::fs::create_dir_all` and create required parent directories
- Added `FilesystemReader::files()` and `file()` as the new method of reading files from an image.
This change also reduced allocations in use when reading.
```diff
- for node in &filesystem.nodes {
+ for node in filesystem.files() {
```
- Compression Options are now written to the image during `FilesystemWriter.write(..)`
- Removed non-used allocation in `SquashFsReader`. No change in public API.
- Changed  `SquashfsReadFile::reader(..)` to reduce the amount of allocation when extracting a file.
This required adding `alloc_read_buffers` to initialize the re-used buffers.
```diff, rust
+// alloc required space for file data readers
+let (mut buf_read, mut buf_decompress) = filesystem.alloc_read_buffers();

-let mut reader = filesystem.file(&file.basic).reader();
+let mut reader = filesystem
+    .file(&file.basic)
+    .reader(&mut buf_read, &mut buf_decompress);
```
- Removed `FilesystemReader::read_file`
- Changed `FilesytemWriter::push_file<P: Into<PathBuf>>(` into `push_file<P: AsRef<Path>>(`.
NOTE: The function will no longer create parent directories! Instead use new `FilesytemWriter::push_dir_all`
- Removed `SquashfsFileSource`
- Changed `FilesystemWriter::push_*()` functions to now return `BackhandError` to avoid duplicate files and invalid file paths.
The following `BackhandError`s were added to support this: `DuplicatedFileName`, `UndefineFileName`, and `InvalidFilePath`.
- Changed `FilesystemWriter::push_block_device<P: Into<PathBuf>>()` into `P: AsRef<Path>`
- Changed `FilesystemWriter::push_block_device<P: Into<PathBuf>>()` into `P: AsRef<Path>`
- Changed `FilesystemWriter::write_with_offset()` to now take `&mut self`
- Changed `FilesystemWriter::write()` to now take `&mut self`
- Removed trait bound from `FilesystemReader`, `FilesystemReaderFile`, and `FilesystemWriter`:
```diff
-pub struct backhand::FilesystemReader<R: backhand::ReadSeek>
+pub struct backhand::FilesystemReader
-pub struct backhand::FilesystemReaderFile<'a, R: backhand::ReadSeek>
+pub struct backhand::FilesystemReaderFile<'a>
-pub struct backhand::FilesystemWriter<'a, R: backhand::ReadSeek>
+pub struct backhand::FilesystemWriter<'a>
```
- Changed public fields in `FilesystemReader`:
```diff
-pub root_inode: SquashfsDir,
-pub nodes: Vec<Node<SquashfsFileReader>>,
+pub root: Nodes<SquashfsFileReader>,
```
- `FilesystemReader::from_reader_*()` functions now take `BufReadSeek` for an increase in performance during reading for some images.

#### Detailed Changed/Added/Removed

```
$ cargo public-api -ss diff v0.11.0..HEAD
```

<details>
<summary>Click to expand</summary>

```diff
Removed items from the public API
=================================
-pub fn backhand::kind::Kind::new() -> Self
-impl core::default::Default for backhand::kind::Kind
-pub fn backhand::kind::Kind::default() -> Self
-impl deku::DekuRead<'_, backhand::kind::Kind> for backhand::Export
-impl deku::DekuRead<'_, backhand::kind::Kind> for backhand::Export
-pub fn backhand::kind::Kind::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-pub fn backhand::kind::Kind::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-pub fn backhand::kind::Kind::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-pub fn backhand::kind::Kind::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-impl deku::DekuRead<'_, backhand::kind::Kind> for backhand::Fragment
-impl deku::DekuRead<'_, backhand::kind::Kind> for backhand::Fragment
-impl deku::DekuRead<'_, backhand::kind::Kind> for backhand::Id
-impl deku::DekuRead<'_, backhand::kind::Kind> for backhand::Id
-impl deku::DekuRead<'_, backhand::kind::Kind> for backhand::SuperBlock
-impl deku::DekuRead<'_, backhand::kind::Kind> for backhand::SuperBlock
-impl deku::DekuWrite<backhand::kind::Kind> for backhand::Export
-impl deku::DekuWrite<backhand::kind::Kind> for backhand::Export
-pub fn backhand::kind::Kind::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(), deku::error::DekuError>
-pub fn backhand::kind::Kind::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(), deku::error::DekuError>
-pub fn backhand::kind::Kind::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(), deku::error::DekuError>
-pub fn backhand::kind::Kind::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(), deku::error::DekuError>
-impl deku::DekuWrite<backhand::kind::Kind> for backhand::Fragment
-impl deku::DekuWrite<backhand::kind::Kind> for backhand::Fragment
-impl deku::DekuWrite<backhand::kind::Kind> for backhand::Id
-impl deku::DekuWrite<backhand::kind::Kind> for backhand::Id
-impl deku::DekuWrite<backhand::kind::Kind> for backhand::SuperBlock
-impl deku::DekuWrite<backhand::kind::Kind> for backhand::SuperBlock
-impl core::clone::Clone for backhand::kind::Kind
-pub fn backhand::kind::Kind::clone(&self) -> backhand::kind::Kind
-impl core::cmp::Eq for backhand::kind::Kind
-impl core::cmp::PartialEq<backhand::kind::Kind> for backhand::kind::Kind
-pub fn backhand::kind::Kind::eq(&self, other: &backhand::kind::Kind) -> bool
-impl core::marker::Copy for backhand::kind::Kind
-impl core::marker::StructuralEq for backhand::kind::Kind
-impl core::marker::StructuralPartialEq for backhand::kind::Kind
-pub enum backhand::SquashfsFileSource<'a, R: backhand::ReadSeek>
-pub backhand::SquashfsFileSource::SquashfsFile(backhand::FilesystemReaderFile<'a, R>)
-pub backhand::SquashfsFileSource::UserDefined(core::cell::RefCell<alloc::boxed::Box<(dyn std::io::Read + 'a)>>)
-pub fn backhand::Export::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-pub fn backhand::Export::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(), deku::error::DekuError>
-pub backhand::FilesystemReader::nodes: alloc::vec::Vec<backhand::Node<backhand::SquashfsFileReader>>
-pub backhand::FilesystemReader::root_inode: backhand::SquashfsDir
-impl<R: backhand::ReadSeek> backhand::FilesystemReader<R>
-impl<R: backhand::ReadSeek> backhand::FilesystemReader<R>
-pub fn backhand::FilesystemReader::file<'a>(&'a self, basic_file: &'a backhand::BasicFile) -> backhand::FilesystemReaderFile<'a, R>
-pub fn backhand::FilesystemReader::read_file(&self, basic_file: &backhand::BasicFile) -> core::result::Result<alloc::vec::Vec<u8>, backhand::BackhandError>
-pub fn backhand::FilesystemReader::from_reader(reader: R) -> core::result::Result<Self, backhand::BackhandError>
-impl<R: backhand::ReadSeek> backhand::FilesystemReader<SquashfsReaderWithOffset<R>>
-pub fn backhand::FilesystemReader::from_reader_with_offset(reader: R, offset: u64) -> core::result::Result<Self, backhand::BackhandError>
-pub fn backhand::FilesystemReader::from_reader_with_offset_and_kind(reader: R, offset: u64, kind: backhand::kind::Kind) -> core::result::Result<Self, backhand::BackhandError>
-impl<R: core::fmt::Debug + backhand::ReadSeek> core::fmt::Debug for backhand::FilesystemReader<R>
-pub fn backhand::FilesystemReader::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
-impl<'a, R: backhand::ReadSeek> backhand::FilesystemReaderFile<'a, R>
-pub fn backhand::FilesystemReaderFile::new(system: &'a backhand::FilesystemReader<R>, basic: &'a backhand::BasicFile) -> Self
-pub fn backhand::FilesystemReaderFile::reader(&self) -> SquashfsReadFile<'a, R>
-impl<'a, R: backhand::ReadSeek> core::clone::Clone for backhand::FilesystemReaderFile<'a, R>
-impl<'a, R: backhand::ReadSeek> core::iter::traits::collect::IntoIterator for backhand::FilesystemReaderFile<'a, R>
-impl<'a, R: core::marker::Copy + backhand::ReadSeek> core::marker::Copy for backhand::FilesystemReaderFile<'a, R>
-impl<'a, R: backhand::ReadSeek> backhand::FilesystemWriter<'a, R>
-pub fn backhand::FilesystemWriter::from_fs_reader(reader: &'a backhand::FilesystemReader<R>) -> core::result::Result<Self, backhand::BackhandError>
-pub fn backhand::FilesystemWriter::mut_file<S: core::convert::Into<std::path::PathBuf>>(&mut self, find_path: S) -> core::option::Option<&mut backhand::SquashfsFileWriter<'a, R>>
-pub fn backhand::FilesystemWriter::push_block_device<P: core::convert::Into<std::path::PathBuf>>(&mut self, device_number: u32, path: P, header: backhand::NodeHeader)
-pub fn backhand::FilesystemWriter::push_char_device<P: core::convert::Into<std::path::PathBuf>>(&mut self, device_number: u32, path: P, header: backhand::NodeHeader)
-pub fn backhand::FilesystemWriter::push_dir<P: core::convert::Into<std::path::PathBuf>>(&mut self, path: P, header: backhand::NodeHeader)
-pub fn backhand::FilesystemWriter::push_file<P: core::convert::Into<std::path::PathBuf>>(&mut self, reader: impl std::io::Read + 'a, path: P, header: backhand::NodeHeader)
-pub fn backhand::FilesystemWriter::push_symlink<P: core::convert::Into<std::path::PathBuf>, S: core::convert::Into<std::path::PathBuf>>(&mut self, link: S, path: P, header: backhand::NodeHeader)
-pub fn backhand::FilesystemWriter::replace_file<S: core::convert::Into<std::path::PathBuf>>(&mut self, find_path: S, reader: impl std::io::Read + 'a) -> core::result::Result<(), backhand::BackhandError>
-pub fn backhand::FilesystemWriter::write<W: std::io::Write + std::io::Seek>(&self, w: &mut W) -> core::result::Result<(backhand::SuperBlock, u64), backhand::BackhandError>
-pub fn backhand::FilesystemWriter::write_with_offset<W: std::io::Write + std::io::Seek>(&self, w: &mut W, offset: u64) -> core::result::Result<(backhand::SuperBlock, u64), backhand::BackhandError>
-impl core::default::Default for backhand::FilesystemWriter<'_>
-impl<'a, R: core::fmt::Debug + backhand::ReadSeek> core::fmt::Debug for backhand::FilesystemWriter<'a, R>
-pub fn backhand::Fragment::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-pub fn backhand::Fragment::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(), deku::error::DekuError>
-pub fn backhand::Id::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-pub fn backhand::Id::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(), deku::error::DekuError>
-impl deku::DekuRead<'_, (u64, u32, u16, backhand::kind::Kind)> for backhand::Inode
-pub fn backhand::Inode::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, (bytes_used, block_size, block_log, kind): (u64, u32, u16, backhand::kind::Kind)) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-impl deku::DekuWrite<(u64, u32, u16, backhand::kind::Kind)> for backhand::Inode
-pub fn backhand::Inode::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, (bytes_used, block_size, block_log, kind): (u64, u32, u16, backhand::kind::Kind)) -> core::result::Result<(), deku::error::DekuError>
-pub backhand::Node::path: std::path::PathBuf
-pub fn backhand::Node::new(path: std::path::PathBuf, inner: backhand::InnerNode<T>) -> Self
-impl<T: core::cmp::Eq> core::cmp::Eq for backhand::Node<T>
-impl<T: core::cmp::PartialEq> core::cmp::PartialEq<backhand::Node<T>> for backhand::Node<T>
-pub fn backhand::Node::eq(&self, other: &backhand::Node<T>) -> bool
-impl<T> core::marker::StructuralEq for backhand::Node<T>
-impl<T> core::marker::StructuralPartialEq for backhand::Node<T>
-pub backhand::Squashfs::data_and_fragments: alloc::vec::Vec<u8>
-impl<R: backhand::ReadSeek> backhand::Squashfs<R>
-impl<R: backhand::ReadSeek> backhand::Squashfs<R>
-pub fn backhand::Squashfs::from_reader(reader: R) -> core::result::Result<backhand::Squashfs<R>, backhand::BackhandError>
-pub fn backhand::Squashfs::into_filesystem_reader(self) -> core::result::Result<backhand::FilesystemReader<R>, backhand::BackhandError>
-impl<R: backhand::ReadSeek> backhand::Squashfs<SquashfsReaderWithOffset<R>>
-pub fn backhand::Squashfs::from_reader_with_offset(reader: R, offset: u64) -> core::result::Result<backhand::Squashfs<SquashfsReaderWithOffset<R>>, backhand::BackhandError>
-pub fn backhand::Squashfs::from_reader_with_offset_and_kind(reader: R, offset: u64, kind: backhand::kind::Kind) -> core::result::Result<backhand::Squashfs<SquashfsReaderWithOffset<R>>, backhand::BackhandError>
-pub backhand::SquashfsBlockDevice::header: backhand::NodeHeader
-pub backhand::SquashfsCharacterDevice::header: backhand::NodeHeader
-pub backhand::SquashfsDir::header: backhand::NodeHeader
-pub backhand::SquashfsFileReader::header: backhand::NodeHeader
-pub struct backhand::SquashfsFileWriter<'a, R: backhand::ReadSeek>
-pub backhand::SquashfsFileWriter::header: backhand::NodeHeader
-pub backhand::SquashfsFileWriter::reader: backhand::SquashfsFileSource<'a, R>
-impl<'a, R: backhand::ReadSeek> core::fmt::Debug for backhand::SquashfsFileWriter<'a, R>
-pub backhand::SquashfsSymlink::header: backhand::NodeHeader
-pub const backhand::SuperBlock::NOT_SET: u64
-pub fn backhand::SuperBlock::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
-pub fn backhand::SuperBlock::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, kind: backhand::kind::Kind) -> core::result::Result<(), deku::error::DekuError>
-pub trait backhand::ReadSeek: std::io::Read + std::io::Seek
-impl<T: std::io::Read + std::io::Seek> backhand::ReadSeek for T

Changed items in the public API
===============================
-pub struct backhand::Export(pub u64)
+pub struct backhand::Export
-pub struct backhand::FilesystemReader<R: backhand::ReadSeek>
+pub struct backhand::FilesystemReader
-pub struct backhand::FilesystemReaderFile<'a, R: backhand::ReadSeek>
+pub struct backhand::FilesystemReaderFile<'a>
-pub struct backhand::FilesystemWriter<'a, R: backhand::ReadSeek>
+pub struct backhand::FilesystemWriter<'a>
-pub struct backhand::Id(pub u32)
+pub struct backhand::Id
-pub struct backhand::Squashfs<R: backhand::ReadSeek>
+pub struct backhand::Squashfs

Added items to the public API
=============================
+pub struct backhand::compression::DefaultCompressor
+impl backhand::compression::CompressionAction for backhand::compression::DefaultCompressor
+impl backhand::compression::CompressionAction for backhand::compression::DefaultCompressor
+pub fn backhand::compression::DefaultCompressor::compress(&self, bytes: &[u8], fc: backhand::FilesystemCompressor, block_size: u32) -> core::result::Result<alloc::vec::Vec<u8>, backhand::BackhandError>
+pub fn backhand::compression::DefaultCompressor::decompress(&self, bytes: &[u8], out: &mut alloc::vec::Vec<u8>, compressor: backhand::compression::Compressor) -> core::result::Result<(), backhand::BackhandError>
+impl core::clone::Clone for backhand::compression::DefaultCompressor
+pub fn backhand::compression::DefaultCompressor::clone(&self) -> backhand::compression::DefaultCompressor
+impl core::marker::Copy for backhand::compression::DefaultCompressor
+pub trait backhand::compression::CompressionAction
+pub fn backhand::compression::CompressionAction::compress(&self, bytes: &[u8], fc: backhand::FilesystemCompressor, block_size: u32) -> core::result::Result<alloc::vec::Vec<u8>, backhand::BackhandError>
+pub fn backhand::compression::CompressionAction::compress(&self, bytes: &[u8], fc: backhand::FilesystemCompressor, block_size: u32) -> core::result::Result<alloc::vec::Vec<u8>, backhand::BackhandError>
+pub fn backhand::compression::CompressionAction::decompress(&self, bytes: &[u8], out: &mut alloc::vec::Vec<u8>, compressor: backhand::compression::Compressor) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::compression::CompressionAction::decompress(&self, bytes: &[u8], out: &mut alloc::vec::Vec<u8>, compressor: backhand::compression::Compressor) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::kind::Kind::from_const(inner: InnerKind<dyn backhand::compression::CompressionAction>) -> core::result::Result<backhand::kind::Kind, alloc::string::String>
+pub fn backhand::kind::Kind::from_kind(kind: &backhand::kind::Kind) -> backhand::kind::Kind
+pub fn backhand::kind::Kind::from_target(s: &str) -> core::result::Result<backhand::kind::Kind, alloc::string::String>
+pub fn backhand::kind::Kind::new<C: backhand::compression::CompressionAction>(compressor: &'static C) -> Self
+pub fn backhand::kind::Kind::new_with_const<C: backhand::compression::CompressionAction>(compressor: &'static C, c: InnerKind<dyn backhand::compression::CompressionAction>) -> Self
+pub backhand::BackhandError::DuplicatedFileName
+pub backhand::BackhandError::InvalidFilePath
+pub backhand::BackhandError::UndefineFileName
+pub enum backhand::SquashfsFileWriter<'a>
+pub backhand::SquashfsFileWriter::Consumed(usize, Added)
+pub backhand::SquashfsFileWriter::SquashfsFile(backhand::FilesystemReaderFile<'a>)
+pub backhand::SquashfsFileWriter::UserDefined(core::cell::RefCell<alloc::boxed::Box<(dyn std::io::Read + 'a)>>)
+impl<'a> core::fmt::Debug for backhand::SquashfsFileWriter<'a>
+pub backhand::Export::num: u64
+impl deku::DekuRead<'_, deku::ctx::Endian> for backhand::Export
+pub fn backhand::Export::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, type_endian: deku::ctx::Endian) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
+impl deku::DekuWrite<deku::ctx::Endian> for backhand::Export
+pub fn backhand::Export::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, type_endian: deku::ctx::Endian) -> core::result::Result<(), deku::error::DekuError>
+pub fn backhand::FilesystemCompressor::options(&mut self, options: backhand::compression::CompressionOptions) -> core::result::Result<(), backhand::BackhandError>
+pub backhand::FilesystemReader::root: Nodes<backhand::SquashfsFileReader>
+impl backhand::FilesystemReader
+pub fn backhand::FilesystemReader::alloc_read_buffers(&self) -> (alloc::vec::Vec<u8>, alloc::vec::Vec<u8>)
+pub fn backhand::FilesystemReader::file<'a>(&'a self, basic_file: &'a backhand::BasicFile) -> backhand::FilesystemReaderFile<'_>
+pub fn backhand::FilesystemReader::files(&self) -> impl core::iter::traits::iterator::Iterator<Item = &backhand::Node<backhand::SquashfsFileReader>>
+pub fn backhand::FilesystemReader::from_reader<R: backhand::BufReadSeek + 'static>(reader: R) -> core::result::Result<Self, backhand::BackhandError>
+pub fn backhand::FilesystemReader::from_reader_with_offset<R: backhand::BufReadSeek + 'static>(reader: R, offset: u64) -> core::result::Result<Self, backhand::BackhandError>
+pub fn backhand::FilesystemReader::from_reader_with_offset_and_kind<R: backhand::BufReadSeek + 'static>(reader: R, offset: u64, kind: backhand::kind::Kind) -> core::result::Result<Self, backhand::BackhandError>
+impl<'a> backhand::FilesystemReaderFile<'a>
+pub fn backhand::FilesystemReaderFile::new(system: &'a backhand::FilesystemReader, basic: &'a backhand::BasicFile) -> Self
+pub fn backhand::FilesystemReaderFile::reader(&self, buf_read: &'a mut alloc::vec::Vec<u8>, buf_decompress: &'a mut alloc::vec::Vec<u8>) -> backhand::SquashfsReadFile<'_>
+impl<'a> core::clone::Clone for backhand::FilesystemReaderFile<'a>
+impl<'a> core::iter::traits::collect::IntoIterator for backhand::FilesystemReaderFile<'a>
+impl<'a> core::marker::Copy for backhand::FilesystemReaderFile<'a>
+impl<'a> backhand::FilesystemWriter<'a>
+pub fn backhand::FilesystemWriter::from_fs_reader(reader: &'a backhand::FilesystemReader) -> core::result::Result<Self, backhand::BackhandError>
+pub fn backhand::FilesystemWriter::mut_file<S: core::convert::AsRef<std::path::Path>>(&mut self, find_path: S) -> core::option::Option<&mut backhand::SquashfsFileWriter<'a>>
+pub fn backhand::FilesystemWriter::push_block_device<P: core::convert::AsRef<std::path::Path>>(&mut self, device_number: u32, path: P, header: backhand::NodeHeader) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::FilesystemWriter::push_char_device<P: core::convert::AsRef<std::path::Path>>(&mut self, device_number: u32, path: P, header: backhand::NodeHeader) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::FilesystemWriter::push_dir<P: core::convert::AsRef<std::path::Path>>(&mut self, path: P, header: backhand::NodeHeader) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::FilesystemWriter::push_dir_all<P: core::convert::AsRef<std::path::Path>>(&mut self, path: P, header: backhand::NodeHeader) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::FilesystemWriter::push_file<P: core::convert::AsRef<std::path::Path>>(&mut self, reader: impl std::io::Read + 'a, path: P, header: backhand::NodeHeader) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::FilesystemWriter::push_symlink<P: core::convert::AsRef<std::path::Path>, S: core::convert::Into<std::path::PathBuf>>(&mut self, link: S, path: P, header: backhand::NodeHeader) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::FilesystemWriter::replace_file<S: core::convert::AsRef<std::path::Path>>(&mut self, find_path: S, reader: impl std::io::Read + 'a) -> core::result::Result<(), backhand::BackhandError>
+pub fn backhand::FilesystemWriter::set_no_padding(&mut self)
+pub fn backhand::FilesystemWriter::write<W: std::io::Write + std::io::Seek>(&mut self, w: &mut W) -> core::result::Result<(backhand::SuperBlock, u64), backhand::BackhandError>
+pub fn backhand::FilesystemWriter::write_with_offset<W: std::io::Write + std::io::Seek>(&mut self, w: &mut W, offset: u64) -> core::result::Result<(backhand::SuperBlock, u64), backhand::BackhandError>
+impl<'a> core::default::Default for backhand::FilesystemWriter<'a>
+impl<'a> core::fmt::Debug for backhand::FilesystemWriter<'a>
+impl backhand::Fragment
+pub fn backhand::Fragment::new(start: u64, size: backhand::DataSize, unused: u32) -> Self
+impl deku::DekuRead<'_, deku::ctx::Endian> for backhand::Fragment
+pub fn backhand::Fragment::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, type_endian: deku::ctx::Endian) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
+impl deku::DekuWrite<deku::ctx::Endian> for backhand::Fragment
+pub fn backhand::Fragment::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, type_endian: deku::ctx::Endian) -> core::result::Result<(), deku::error::DekuError>
+pub backhand::Id::num: u32
+pub const backhand::Id::SIZE: usize
+pub fn backhand::Id::new(num: u32) -> backhand::Id
+impl deku::DekuRead<'_, deku::ctx::Endian> for backhand::Id
+pub fn backhand::Id::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, type_endian: deku::ctx::Endian) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
+impl deku::DekuWrite<deku::ctx::Endian> for backhand::Id
+pub fn backhand::Id::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, type_endian: deku::ctx::Endian) -> core::result::Result<(), deku::error::DekuError>
+impl backhand::Inode
+pub fn backhand::Inode::new(id: InodeId, header: InodeHeader, inner: InodeInner) -> Self
+impl deku::DekuRead<'_, (u64, u32, u16, deku::ctx::Endian)> for backhand::Inode
+pub fn backhand::Inode::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, (bytes_used, block_size, block_log, type_endian): (u64, u32, u16, deku::ctx::Endian)) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
+impl deku::DekuWrite<(u64, u32, u16, deku::ctx::Endian)> for backhand::Inode
+pub fn backhand::Inode::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, (bytes_used, block_size, block_log, type_endian): (u64, u32, u16, deku::ctx::Endian)) -> core::result::Result<(), deku::error::DekuError>
+pub backhand::Node::fullpath: std::path::PathBuf
+pub backhand::Node::header: backhand::NodeHeader
+pub fn backhand::Node::new_root(header: backhand::NodeHeader) -> Self
+impl<T> core::cmp::Eq for backhand::Node<T>
+impl<T> core::cmp::Ord for backhand::Node<T>
+pub fn backhand::Node::cmp(&self, other: &Self) -> core::cmp::Ordering
+impl<T> core::cmp::PartialEq<backhand::Node<T>> for backhand::Node<T>
+pub fn backhand::Node::eq(&self, other: &Self) -> bool
+impl<T> core::cmp::PartialOrd<backhand::Node<T>> for backhand::Node<T>
+pub fn backhand::Node::partial_cmp(&self, other: &Self) -> core::option::Option<core::cmp::Ordering>
+impl<T: core::clone::Clone> core::clone::Clone for backhand::Node<T>
+pub fn backhand::Node::clone(&self) -> backhand::Node<T>
+impl backhand::Squashfs
+pub fn backhand::Squashfs::from_reader(reader: impl backhand::BufReadSeek + 'static) -> core::result::Result<backhand::Squashfs, backhand::BackhandError>
+pub fn backhand::Squashfs::from_reader_with_offset(reader: impl backhand::BufReadSeek + 'static, offset: u64) -> core::result::Result<backhand::Squashfs, backhand::BackhandError>
+pub fn backhand::Squashfs::from_reader_with_offset_and_kind(reader: impl backhand::BufReadSeek + 'static, offset: u64, kind: backhand::kind::Kind) -> core::result::Result<backhand::Squashfs, backhand::BackhandError>
+pub fn backhand::Squashfs::into_filesystem_reader(self) -> core::result::Result<backhand::FilesystemReader, backhand::BackhandError>
+pub fn backhand::Squashfs::superblock_and_compression_options(reader: &mut alloc::boxed::Box<dyn backhand::BufReadSeek>, kind: &backhand::kind::Kind) -> core::result::Result<(backhand::SuperBlock, core::option::Option<backhand::compression::CompressionOptions>), backhand::BackhandError>
+impl core::marker::Copy for backhand::SquashfsBlockDevice
+impl core::marker::Copy for backhand::SquashfsCharacterDevice
+impl core::default::Default for backhand::SquashfsDir
+pub fn backhand::SquashfsDir::default() -> backhand::SquashfsDir
+impl core::marker::Copy for backhand::SquashfsDir
+pub struct backhand::SquashfsReadFile<'a>
+impl<'a> std::io::Read for backhand::SquashfsReadFile<'a>
+pub fn backhand::SquashfsReadFile::read(&mut self, buf: &mut [u8]) -> std::io::error::Result<usize>
+impl backhand::SuperBlock
+impl deku::DekuRead<'_, ([u8; 4], u16, u16, deku::ctx::Endian)> for backhand::SuperBlock
+pub fn backhand::SuperBlock::read(__deku_input_bits: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, (ctx_magic, ctx_version_major, ctx_version_minor, ctx_type_endian): ([u8; 4], u16, u16, deku::ctx::Endian)) -> core::result::Result<(&bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, Self), deku::error::DekuError>
+impl deku::DekuWrite<([u8; 4], u16, u16, deku::ctx::Endian)> for backhand::SuperBlock
+pub fn backhand::SuperBlock::write(&self, __deku_output: &mut bitvec::vec::BitVec<u8, bitvec::order::Msb0>, (ctx_magic, ctx_version_major, ctx_version_minor, ctx_type_endian): ([u8; 4], u16, u16, deku::ctx::Endian)) -> core::result::Result<(), deku::error::DekuError>
+pub trait backhand::BufReadSeek: std::io::BufRead + std::io::Seek
+impl<T: std::io::BufRead + std::io::Seek> backhand::BufReadSeek for T
```

</details>

### `unsquashfs`
- Added `--kind` for custom squashfs type image extraction
```
  -k, --kind <KIND>      Kind(type of image) to parse [default: le_v4_0] [possible values: be_v4_0, le_v4_0, amv_be_v4_0]
```
- Added `--completions` for the generation of shell completions scripts

#### Performance
See https://github.com/wcampbell0x2a/backhand/discussions/145 for more details.
These are benchmarked against the SquashFS image from `TP-Link AXE5400 Mesh Wi-Fi 6E Range Extender`.

##### Speed
For single threaded mode `squashfs-tools/unsquashfs-v4.6.1`, testing on my machine lets me know that 
our `backhand/unsquashfs` is around the same speed performance with a single thread.

##### Allocations
Only testing single threaded mode, peak heap memory consumption for `squashfs-tools/unsquashfs-v4.6.1`
is 74.8MB, while our `backhand/unsquashfs` only uses 18.1MB.

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

### [v0.7.0] - 2023-01-23
#### Added
- Use `block_size` as XZ default `dict_size` when compressing data
- Add `Filesystem::push_symlink(..)`
- Add `Filesystem::push_dir(..)`
- Add `Filesystem::push_char_device(..)`
- Add `Filesystem::push_block_device(..)`

#### Fixed
- Correctly choose between storing uncompressed and compressed data on which takes the least space

#### Changed
- Improve `unsquashfs` and `add` cli args to match `squashfs-tools/unsquashfs` cli
- `Filesystem::push_file(..)` now takes for bytes anything that is `into Read` instead of `into Vec<u8>`
- `Node::Path` renamed to `Node::Dir`
- `SquashfsPath` renamed to `SquashfsDir`
- `Filesystem::from_reader(..)`, `R` now takes `Read + Seek` instead our own `ReadSeek`
- `Filesystem::from_reader_with_offset(..)`, `R` now takes `Read + Seek` instead our own `ReadSeek`
- `Filesystem::push_symlink(..)` now only needs `path` and `link`

### [v0.6.0] - 2023-01-10
- Fix bug in our filesystem tree causing directory header information (gui, uid, permissions)
  to not be saved in resulting filesystem when calling `Filesystem::to_bytes(..)`.
- Rework `filesystem::Node` to be a struct containing the path and `InnerNode`.
  This cleans up the inner implementation of the file system tree.
- Make more types public that are useful for Squashfs detailed introspection
- Improve documentation

### [v0.5.0] - 2023-01-08
- Fix warning when compression options isn't standard size
- In `from_reader(..)`, show info about flags used
- Add `Filesystem::from_reader(..)` and `Filesystem::from_reader_with_offset(..)`
  which calls `Squashfs::from_reader(..)` and `Squashfs::from_reader_with_offset(..)` and `Squashfs::into_filesystem(..)`.
- 5% Performance increases due to using `Vec::with_capacity(..)` for `fragment_bytes`
- Add Block and Char Device support for Reading and Writing
- Fix error with `inode_offset` mis-calculation
- Fix tail-end fragment support for reading image
- Fix `unsquashfs` file path extraction

### [v0.4.0] - 2023-01-04
- Add `mod_time` from `Squashfs` to `Filesystem` used in creation of new image with `to_bytes(..)`

### [v0.3.0] - 2023-01-03
- Restrict public API
- Improve docs
- Add `Filesystem::push_file(..)` for adding a file, as well as the dirs for the path
- Add `Filesystem::mut_file(..)` for mutating a file at a path already in the filesystem

### [v0.2.1] - 2023-01-02
- Fix Cargo.toml issues

### [v0.2.0] - 2023-01-02
- Add `block_size` and `block_log` to Filesystem. Automatically taken from `Squashfs` when using `into_filesystem()`
- Add support for data fragments for `filesystem::to_bytes()`
- `DirEntry` uses `InodeId` instead of `u8`

### [v0.1.0] - 2023-01-01
- Initial Release
