mod common;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor};

use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
use common::test_unsquashfs;
use test_assets::TestAssetDef;
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
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "8610cd350bbd51ca6c8b84c210ef24c57898845f75f5b4ae0c6d7e785efaab4f".to_string(),
            url: "https://wcampbell.dev/squashfs/testing/test_add_00/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "new.squashfs".to_string(),
            hash: "dc02848152d42b331fa0540000f68bf0942c5b00a3a44a3a6f208af34b4b6ec3".to_string(),
            url: "https://wcampbell.dev/squashfs/testing/test_add_00/new.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "control.squashfs".to_string(),
            hash: "b690b167ef3d6126ca4180e73cf0cb827f48405630278a64017208b6774b663b".to_string(),
            url: "https://wcampbell.dev/squashfs/testing/test_add_00/control.squashfs".to_string(),
        },
    ];
    const TEST_PATH: &str = "test-assets/test_add_00";
    let og_path = format!("{TEST_PATH}/out.squashfs");
    let new_path = format!("{TEST_PATH}/bytes.squashfs");

    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();
    let file = BufReader::new(File::open(og_path).unwrap());
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
        test_unsquashfs(&new_path, &control_new_path, None);
    }
}
