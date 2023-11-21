use backhand::NodeHeader;

/// https://github.com/wcampbell0x2a/backhand/issues/275
#[test]
#[cfg(feature = "xz")]
fn issue_275() {
    let mut writer = std::io::Cursor::new(vec![]);
    let mut fs = backhand::FilesystemWriter::default();
    fs.write(&mut writer).unwrap();
}

/// https://github.com/wcampbell0x2a/backhand/issues/359
#[test]
fn issue_359() {
    let mut writer = std::io::Cursor::new(vec![]);
    let mut fs = backhand::FilesystemWriter::default();
    let header = NodeHeader { permissions: 0, uid: 1, gid: 2, mtime: 3 };
    fs.push_dir_all("a/b/c/d/e/f/g", header).unwrap();
    fs.write(&mut writer).unwrap();
}
