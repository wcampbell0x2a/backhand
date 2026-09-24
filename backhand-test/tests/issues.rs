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

/// https://github.com/wcampbell0x2a/backhand/issues/803
///
/// An empty file must have no fragment, or the kernel returns EINVAL on stat and open
#[test]
#[cfg(feature = "xz")]
fn issue_803() {
    use backhand::{FilesystemReader, FilesystemWriter, InnerNode, SquashfsFileReader};
    use std::io::Cursor;

    fn assert_no_fragment(image: &[u8]) {
        let reader = FilesystemReader::from_reader(Cursor::new(image)).unwrap();
        let node = reader.files().find(|node| node.fullpath.ends_with("empty")).unwrap();
        let InnerNode::File(SquashfsFileReader::Basic(file)) = &node.inner else {
            panic!("expected a basic file: {:?}", node.inner);
        };
        assert_eq!(file.file_size, 0);
        assert_eq!(file.frag_index, 0xffffffff);
        assert!(file.block_sizes.is_empty());
    }

    let mut fs = FilesystemWriter::default();
    fs.push_file(Cursor::new(vec![]), "empty", backhand::NodeHeader::default()).unwrap();
    // A small file before the empty file, so that the fragment table is not empty
    fs.push_file(Cursor::new(vec![1; 10]), "small", backhand::NodeHeader::default()).unwrap();
    let mut image = Cursor::new(vec![]);
    fs.write(&mut image).unwrap();
    let image = image.into_inner();
    assert_no_fragment(&image);

    let reader = FilesystemReader::from_reader(Cursor::new(&image)).unwrap();
    let mut copy = Cursor::new(vec![]);
    FilesystemWriter::from_fs_reader(&reader).unwrap().write(&mut copy).unwrap();
    assert_no_fragment(&copy.into_inner());
}
