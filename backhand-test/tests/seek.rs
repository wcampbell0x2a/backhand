mod common;

use backhand::DEFAULT_BLOCK_SIZE;
use std::io::{Cursor, Read, Seek, SeekFrom};

fn read_at(reader: &mut (impl Read + Seek), pos: u64, len: usize) -> Vec<u8> {
    reader.seek(SeekFrom::Start(pos)).unwrap();
    let mut buf = vec![0u8; len];
    let n = reader.read(&mut buf).unwrap();
    buf.truncate(n);
    buf
}

#[cfg(feature = "xz")]
mod v4 {
    use super::*;
    use backhand::{FilesystemReader, FilesystemWriter, InnerNode, NodeHeader};

    fn create_test_data() -> (Vec<u8>, Vec<u8>) {
        let size = (DEFAULT_BLOCK_SIZE as usize) * 3 + 2;
        let data: Vec<u8> =
            (0..size).map(|i| ((i ^ (i >> 8) ^ (i >> 16) ^ (i >> 24)) & 255) as u8).collect();

        let mut fs = FilesystemWriter::default();
        fs.push_file(Cursor::new(data.clone()), "test_file", NodeHeader::default()).unwrap();
        let mut out = Cursor::new(Vec::new());
        fs.write(&mut out).unwrap();

        (out.into_inner(), data)
    }

    #[test]
    fn test_seek_and_read() {
        let (squashfs, data) = create_test_data();
        let reader = FilesystemReader::from_reader(Cursor::new(&squashfs)).unwrap();
        let node = reader.files().find(|n| n.fullpath.to_str() == Some("/test_file")).unwrap();
        let file = match &node.inner {
            InnerNode::File(f) => f,
            _ => panic!("expected file"),
        };
        let mut r = reader.file(file).reader();

        // big seek forward
        let pos = (DEFAULT_BLOCK_SIZE * 2) as u64;
        assert_eq!(read_at(&mut r, pos, 4), &data[pos as usize..][..4]);

        // seek to exact block boundary (block multiple)
        let block_pos = DEFAULT_BLOCK_SIZE as u64;
        assert_eq!(read_at(&mut r, block_pos, 4), &data[block_pos as usize..][..4]);

        // reset to head
        assert_eq!(read_at(&mut r, 0, 1), &data[0..1]);

        // seek to current
        assert_eq!(read_at(&mut r, 1, 1), &data[1..2]);

        // small seeks forward and back
        assert_eq!(read_at(&mut r, 10, 4), &data[10..14]);
        assert_eq!(read_at(&mut r, 100, 4), &data[100..104]);
        assert_eq!(read_at(&mut r, 10, 4), &data[10..14]);
        assert_eq!(read_at(&mut r, 0, 4), &data[0..4]);

        // last byte in fragment, overread past EOF by one byte
        let frag_pos = (DEFAULT_BLOCK_SIZE * 3 + 1) as u64;
        assert_eq!(read_at(&mut r, frag_pos, 2), &data[frag_pos as usize..][..1]);

        // way past eof
        assert_eq!(read_at(&mut r, (DEFAULT_BLOCK_SIZE * 100) as u64, 1), &[]);
    }

    #[test]
    fn test_seek_positions() {
        let (squashfs, _) = create_test_data();
        let reader = FilesystemReader::from_reader(Cursor::new(&squashfs)).unwrap();
        let node = reader.files().find(|n| n.fullpath.to_str() == Some("/test_file")).unwrap();
        let file = match &node.inner {
            InnerNode::File(f) => f,
            _ => panic!("expected file"),
        };
        let mut r = reader.file(file).reader();

        let superhuge = 1_000_000_000_000_000u64;
        let isuperhuge = 1_000_000_000_000_000i64;
        let len = r.seek(SeekFrom::End(0)).unwrap();
        let ilen = len as i64;
        assert_eq!(len, (DEFAULT_BLOCK_SIZE * 3 + 2) as u64);

        assert_eq!(r.seek(SeekFrom::Start(100)).unwrap(), 100);
        assert_eq!(r.seek(SeekFrom::Current(0)).unwrap(), 100);
        assert_eq!(r.seek(SeekFrom::Current(-100)).unwrap(), 0);
        assert_eq!(r.seek(SeekFrom::End(-ilen + 1)).unwrap(), 1);
        assert_eq!(r.seek(SeekFrom::End(-ilen)).unwrap(), 0);
        assert!(r.seek(SeekFrom::End(-ilen - 1)).is_err());
        assert_eq!(r.seek(SeekFrom::Start(superhuge)).unwrap(), superhuge);
        assert_eq!(r.seek(SeekFrom::End(superhuge as i64)).unwrap(), len + superhuge);
        assert_eq!(r.seek(SeekFrom::Current(0)).unwrap(), len + superhuge);
        assert_eq!(r.seek(SeekFrom::Current(1)).unwrap(), len + superhuge + 1);
        assert_eq!(r.seek(SeekFrom::Current(-isuperhuge)).unwrap(), len + 1);
        assert!(r.seek(SeekFrom::Current(-isuperhuge)).is_err());
        assert!(r.seek(SeekFrom::End(-isuperhuge)).is_err());
    }
}

// v3 tests
#[cfg(feature = "v3")]
mod v3 {
    use super::common;
    use super::*;
    use backhand::kind::{Kind, LE_V3_0};
    use backhand::v3::filesystem::node::InnerNode;
    use backhand::v3::filesystem::reader::FilesystemReader;
    use std::fs::File;
    use std::io::BufReader;

    #[test]
    fn test_seek_and_read() {
        // reusing assets from backhand-test/tests/v3.rs
        common::download_asset("v3_le_more");
        let file = BufReader::new(File::open("test-assets/test_v3_more/test_v3.sqfs").unwrap());
        let reader = FilesystemReader::from_reader_with_offset_and_kind(
            file,
            0,
            Kind::from_const(LE_V3_0).unwrap(),
        )
        .unwrap();

        let node = reader
            .files()
            .find(|n| matches!(&n.inner, InnerNode::File(f) if f.file_len() > 0))
            .unwrap();
        let file_ref = match &node.inner {
            InnerNode::File(f) => f,
            _ => unreachable!(),
        };

        let mut full_data = Vec::new();
        reader.file(file_ref).reader().read_to_end(&mut full_data).unwrap();
        let file_len = full_data.len();
        assert!(file_len > 8 && file_len < 999_999_999);

        let mut r = reader.file(file_ref).reader();

        assert_eq!(read_at(&mut r, 0, 4), &full_data[0..4]);
        assert_eq!(read_at(&mut r, file_len as u64 - 1, 4), &full_data[file_len - 1..]);
        assert_eq!(read_at(&mut r, file_len as u64 + 1000, 4), &[]);
        assert_eq!(read_at(&mut r, 2, 4), &full_data[2..6]);
        assert_eq!(read_at(&mut r, 0, file_len), &full_data[..]);

        let len = r.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(len, file_len as u64);
        assert_eq!(r.seek(SeekFrom::End(1000)).unwrap(), file_len as u64 + 1000);
        assert_eq!(r.seek(SeekFrom::Current(-1)).unwrap(), file_len as u64 + 999);
        assert_eq!(r.seek(SeekFrom::Start(1)).unwrap(), 1);
        assert_eq!(r.seek(SeekFrom::Start(1_000_000_000)).unwrap(), 1_000_000_000);
        assert_eq!(r.seek(SeekFrom::Current(-1)).unwrap(), 999_999_999);
        assert_eq!(r.seek(SeekFrom::Current(-999_999_999)).unwrap(), 0);
        assert!(r.seek(SeekFrom::Current(-1)).is_err());
        assert_eq!(r.seek(SeekFrom::Current(0)).unwrap(), 0);
        assert!(r.seek(SeekFrom::End(-(file_len as i64) - 1)).is_err());
        assert_eq!(r.seek(SeekFrom::End(-(file_len as i64) + 1)).unwrap(), 1);
        assert_eq!(r.seek(SeekFrom::Current(0)).unwrap(), 1);
    }
}
