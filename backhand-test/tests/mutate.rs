mod common;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor};

use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
#[allow(unused_imports)]
use common::{test_bin_unsquashfs, test_squashfs_tools_unsquashfs};
use test_log::test;

/// Before:
/// testing
/// └── a
///     └── b
///         └── c
///             └── d
///                 └── e
///                     └── first_file
/// After:
/// testing
/// ├── a
/// │   ├── b
/// │   │   └── c
/// │   │       └── d
/// │   │           └── e
/// │   │               ├── dude
/// │   │               └── first_file (modified)
/// │   └── d
/// │       └── e
/// │           └── new_file (added)
/// ├── ptr -> a/b/c/d/dude
/// └── root_file (added)
#[test]
#[cfg(feature = "xz")]
fn test_add_00() {
    common::download_asset("add_00_out");
    common::download_asset("add_00_new");
    common::download_asset("add_00_control");
    let og_path = "test-assets/test_add_00/out.squashfs";
    let new_path = std::path::Path::new(&og_path).parent().unwrap().join("bytes.squashfs");
    let new_path = new_path.to_str().unwrap();
    let file = BufReader::new(File::open(&og_path).unwrap());
    let og_filesystem = FilesystemReader::from_reader(file).unwrap();
    let mut new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();

    let h = NodeHeader { permissions: 0o755, uid: 0, gid: 0, mtime: 0 };

    //create directories
    new_filesystem.push_dir_all("a/d/e", h).unwrap();
    new_filesystem.push_dir_all("a/b/c/d/e", h).unwrap();
    // Add file
    let bytes = Cursor::new(b"this is a new file, wowo!");
    new_filesystem.push_file(bytes, "a/d/e/new_file", h).unwrap();
    // Add file
    new_filesystem.push_file(Cursor::new("i am (g)root"), "root_file", h).unwrap();

    // Add file
    new_filesystem.push_file(Cursor::new("dude"), "a/b/c/d/dude", h).unwrap();

    new_filesystem.push_symlink("a/b/c/d/dude", "ptr", h).unwrap();

    // Modify file
    new_filesystem
        .replace_file("/a/b/c/d/e/first_file", Cursor::new(b"MODIFIEDfirst file!\n"))
        .unwrap();

    // create the modified squashfs
    {
        let mut output = BufWriter::new(File::create(&new_path).unwrap());
        new_filesystem.write(&mut output).unwrap();
    }
    // force output to drop, so buffer is written

    // compare when on x86 host
    #[cfg(feature = "__test_unsquashfs")]
    {
        let control_new_path = "test-assets/test_add_00/control.squashfs";
        test_squashfs_tools_unsquashfs(&new_path, &control_new_path, None, true);
        test_bin_unsquashfs(&og_path, None, true, true);
        test_bin_unsquashfs(&new_path, None, true, true);
    }
}
