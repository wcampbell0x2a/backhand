mod common;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor};

use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
use common::{test_bin_unsquashfs, test_squashfs_tools_unsquashfs};
use test_assets_ureq::TestAssetDef;
use test_log::test;

use crate::common::read_asset;

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
    let (test_path, asset_def_01) = read_asset("mutate_01");
    let (test_path, asset_def_02) = read_asset("mutate_02");
    let (test_path, asset_def_03) = read_asset("mutate_03");
    let asset_defs = &[asset_def_01, asset_def_02, asset_def_03];
    let file_name = &asset_defs[0].filename;
    let og_path = format!("{test_path}/out.squashfs");
    let new_path = format!("{test_path}/bytes.squashfs");

    common::download_backoff(asset_defs, &test_path);
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
        let control_new_path = format!("{TEST_PATH}/control.squashfs");
        test_squashfs_tools_unsquashfs(&new_path, &control_new_path, None, true);
        test_bin_unsquashfs(&og_path, None, true, true);
        test_bin_unsquashfs(&new_path, None, true, true);
    }
}
