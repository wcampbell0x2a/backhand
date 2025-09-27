use std::io::{Cursor, Seek, SeekFrom};

use backhand::{FilesystemReader, FilesystemWriter, InnerNode, Node, NodeHeader, SquashfsDir};

mod common;

/// Removing a file should work as expected
#[test]
fn test_remove_single_file() {
    let mut writer = FilesystemWriter::default();
    let dummy_file = Cursor::new(&[1, 2, 3]);
    let dummy_header = NodeHeader::new(0, 0, 0, 0);
    writer.push_dir("/test", dummy_header).unwrap();
    writer.push_file(dummy_file, "/test/file", dummy_header).unwrap();

    writer.remove_entry("/test/file").unwrap();
    let mut out_buffer = Cursor::new(vec![]);
    writer.write(&mut out_buffer).unwrap();
    drop(writer);

    out_buffer.seek(std::io::SeekFrom::Start(0)).unwrap();
    let reader = FilesystemReader::from_reader(out_buffer).unwrap();
    assert_eq!(
        reader.root.nodes,
        vec![
            Node::new_root(dummy_header),
            Node {
                fullpath: "/test".into(),
                header: dummy_header,
                inner: InnerNode::Dir(SquashfsDir {})
            }
        ]
    );
}

/// Removing a directory should also remove its children
#[test]
fn test_remove_children() {
    let mut writer = FilesystemWriter::default();
    let mut dummy_file = Cursor::new(&[1, 2, 3]);
    let dummy_header = NodeHeader::new(0, 0, 0, 0);
    writer.push_dir_all("/test/deeper", dummy_header).unwrap();
    writer.push_file(dummy_file.clone(), "/test/deeper/file", dummy_header).unwrap();
    dummy_file.seek(SeekFrom::Start(0)).unwrap();
    writer.push_file(dummy_file, "/test/deeper/file2", dummy_header).unwrap();

    writer.remove_entry("/test/deeper").unwrap();
    let mut out_buffer = Cursor::new(vec![]);
    writer.write(&mut out_buffer).unwrap();
    drop(writer);

    out_buffer.seek(SeekFrom::Start(0)).unwrap();
    let reader = FilesystemReader::from_reader(out_buffer).unwrap();
    assert_eq!(
        reader.root.nodes,
        vec![
            Node::new_root(dummy_header),
            Node {
                fullpath: "/test".into(),
                header: dummy_header,
                inner: InnerNode::Dir(SquashfsDir {})
            }
        ]
    );
}
