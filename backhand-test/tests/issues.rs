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
#[cfg(feature = "xz")]
fn issue_359() {
    let mut writer = std::io::Cursor::new(vec![]);
    let mut fs = backhand::FilesystemWriter::default();
    let header = backhand::NodeHeader { permissions: 0, uid: 1, gid: 2, mtime: 3 };
    fs.push_dir_all("a/b/c/d/e/f/g", header).unwrap();
    fs.write(&mut writer).unwrap();
}

/// https://github.com/wcampbell0x2a/backhand/issues/363
#[test]
#[cfg(feature = "xz")]
fn issue_363() {
    let dummy_file = std::io::Cursor::new(&[]);
    let dummy_header = backhand::NodeHeader::default();
    let mut fs = backhand::FilesystemWriter::default();
    // create a files
    fs.push_file(dummy_file.clone(), "a", dummy_header).unwrap();
    // try to put a file inside the first file
    match fs.push_file(dummy_file, "a/b", dummy_header) {
        // correct result: InvalidFilePath (or equivalent error?)
        Err(e) => {
            // Should get InvalidFilePath or equivalent error
            println!("Got expected error: {:?}", e);
        }
        Ok(_) => panic!("Invalid result"),
    };
}
