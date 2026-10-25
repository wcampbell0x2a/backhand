//! Hard links in images read by [`FilesystemReader`]
//!
//! All names of a hard-linked file point at one inode. [`Node::inode_number`] shows which nodes
//! share an inode, while files that mksquashfs only deduplicated keep inodes of their own.

#![cfg(feature = "xz")]

use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;

use backhand::FilesystemReader;

#[test]
#[cfg(feature = "__test_unsquashfs")]
fn test_mksquashfs_hard_links() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(src.join("dir")).unwrap();
    std::fs::write(src.join("a"), b"linked").unwrap();
    std::fs::hard_link(src.join("a"), src.join("dir/b")).unwrap();
    // same content, but not a link
    std::fs::write(src.join("copy"), b"linked").unwrap();

    let image = tmp.path().join("image.squashfs");
    let output = Command::new("mksquashfs")
        .arg(&src)
        .arg(&image)
        .args(["-comp", "xz", "-noappend", "-no-xattrs", "-quiet"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let image = std::fs::read(&image).unwrap();
    let reader = FilesystemReader::from_reader(Cursor::new(image)).unwrap();
    let inodes: HashMap<PathBuf, u32> =
        reader.files().map(|node| (node.fullpath.clone(), node.inode_number.unwrap())).collect();
    let inode = |path: &str| inodes[&PathBuf::from(path)];

    assert_eq!(inode("/a"), inode("/dir/b"));
    assert_ne!(inode("/a"), inode("/copy"));
    assert_ne!(inode("/"), inode("/dir"));
}
