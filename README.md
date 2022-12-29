# Squashfs-deku
Library and collection of binaries for the reading, creation, and modification 
of [SquashFS](https://en.wikipedia.org/wiki/SquashFS) file systems.

| :warning: WARNING                                                                          |
|:-------------------------------------------------------------------------------------------|
| The API for this libary **isn't** complete. I will most likely break this for improvements |

## Library
See `cargo doc`, but here are some examples
### Reading + Writing Firmware
```rust
// read
let file = File::open(args.input).unwrap();
let squashfs = Squashfs::from_reader(file).unwrap();
let filesystem = squashfs.into_filesystem().unwrap();

// write
let bytes = filesystem.to_bytes().unwrap();
```

### Modifying Firmware
See the `add` binary.

## Binaries
These are currently under developement and are missing features, MR's welcome!

### unsquashfs
```
Usage: unsquashfs [OPTIONS] <INPUT> <COMMAND>

Commands:
  extract-files  Extract single file from image
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
```
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
