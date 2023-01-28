mod common;
use std::cell::RefCell;
use std::fs::{self, File};
use std::io::Cursor;

use backhand::filesystem::{FilesystemHeader, FilesystemReader};
use backhand::FilesystemWriter;
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
/// └── root_file (added)
#[test]
fn test_add_00() {
    let asset_defs = [
        TestAssetDef {
            filename: "out.squashfs".to_string(),
            hash: "8610cd350bbd51ca6c8b84c210ef24c57898845f75f5b4ae0c6d7e785efaab4f".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_add_00/out.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "new.squashfs".to_string(),
            hash: "dc02848152d42b331fa0540000f68bf0942c5b00a3a44a3a6f208af34b4b6ec3".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_add_00/new.squashfs".to_string(),
        },
        TestAssetDef {
            filename: "control.squashfs".to_string(),
            hash: "a227c214be3efbd9b6958918e23d13f4c98de7a1fde64c2a5ede1c4c69938930".to_string(),
            url: "wcampbell.dev/squashfs/testing/test_add_00/control.squashfs".to_string(),
        },
    ];
    const TEST_PATH: &str = "test-assets/test_add_00";
    let og_path = format!("{TEST_PATH}/out.squashfs");
    let new_path = format!("{TEST_PATH}/bytes.squashfs");

    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();
    let file = File::open(og_path).unwrap();
    let og_filesystem = FilesystemReader::from_reader(file).unwrap();
    let mut new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();

    let h = FilesystemHeader {
        permissions: 0o755,
        uid: 0,
        gid: 0,
        mtime: 0,
    };

    // Add file
    let bytes = Cursor::new(b"this is a new file, wowo!");
    new_filesystem
        .push_file(bytes, "a/d/e/new_file", h)
        .unwrap();
    // Add file
    new_filesystem
        .push_file(Cursor::new("i am (g)root"), "root_file", h)
        .unwrap();
    // Add file
    new_filesystem
        .push_file(Cursor::new("dude"), "a/b/c/d/dude", h)
        .unwrap();

    // Modify file
    let file = new_filesystem.mut_file("/a/b/c/d/e/first_file").unwrap();
    file.reader = RefCell::new(Box::new(Cursor::new(b"MODIFIEDfirst file!\n")));

    let bytes = new_filesystem.to_bytes().unwrap();
    fs::write(&new_path, bytes).unwrap();

    let control_new_path = format!("{TEST_PATH}/control.squashfs");
    test_unsquashfs(&new_path, &control_new_path, None);
}
