mod common;
use std::fs::{self, File};
use std::io::Cursor;

use backhand::filesystem::FilesystemHeader;
use backhand::Filesystem;
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
    ];
    const TEST_PATH: &str = "test-assets/test_add_00";
    let og_path = format!("{TEST_PATH}/out.squashfs");
    let new_path = format!("{TEST_PATH}/bytes.squashfs");

    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();
    let file = File::open(og_path).unwrap();
    let mut og_filesystem = Filesystem::from_reader(file).unwrap();

    // Add file
    og_filesystem
        .push_file(
            &mut Cursor::new("this is a new file, wowo!"),
            "a/d/e/new_file",
            FilesystemHeader::default(),
        )
        .unwrap();
    // Add file
    og_filesystem
        .push_file(
            &mut Cursor::new("i am (g)root"),
            "root_file",
            FilesystemHeader::default(),
        )
        .unwrap();
    // Add file
    og_filesystem
        .push_file(
            &mut Cursor::new("dude"),
            "a/b/c/d/dude",
            FilesystemHeader::default(),
        )
        .unwrap();

    // Modify file
    let file = og_filesystem.mut_file("/a/b/c/d/e/first_file").unwrap();
    file.bytes = b"MODIFIEDfirst file!\n".to_vec();

    let bytes = og_filesystem.to_bytes().unwrap();
    fs::write(new_path, bytes).unwrap();

    let new_path = format!("{TEST_PATH}/new.squashfs");
    test_unsquashfs(&new_path, &new_path, None);
}
