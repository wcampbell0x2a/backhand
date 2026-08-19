/// Regression test: zero-length files must be encoded with NO fragment
/// reference (frag_index 0xffffffff). A dangling zero-byte fragment entry
/// makes the Linux kernel squashfs driver reject the inode with EINVAL on
/// stat/open, even though userspace readers tolerate it.
use std::io::Cursor;

use backhand::v4::filesystem::node::InnerNode;
use backhand::SquashfsFileReader;
use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
use test_log::test;

#[test]
fn test_empty_file_has_no_fragment_ref() {
    let mut fs = FilesystemWriter::default();
    fs.push_file(Cursor::new(vec![]), "empty", NodeHeader::default()).unwrap();
    fs.push_file(Cursor::new(b"data".to_vec()), "full", NodeHeader::default()).unwrap();

    let mut image = Cursor::new(vec![]);
    fs.write(&mut image).unwrap();
    image.set_position(0);

    let fs = FilesystemReader::from_reader(image).unwrap();
    let node = fs.files().find(|n| n.fullpath.to_string_lossy() == "/empty").unwrap();
    let InnerNode::File(SquashfsFileReader::Basic(basic)) = &node.inner else {
        panic!("expected basic file inode");
    };
    assert_eq!(basic.file_size, 0);
    assert_eq!(basic.frag_index, 0xffffffff, "empty file must not reference a fragment");
}
