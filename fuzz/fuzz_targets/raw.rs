#![no_main]

use backhand::{SquashfsDir, NodeHeader, FilesystemWriter, FilesystemReader};
use backhand::compression::Compressor::Xz;
use backhand::internal::Id;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Vec<u8>| {
    let header = NodeHeader {
        permissions: 0o755,
        uid: 0,
        gid: 0,
        mtime: 0,
    };

    let mut fs: FilesystemWriter = FilesystemWriter {
        id_table: Some(vec![Id(0)]),
        mod_time: 0x634f5237,
        block_size: 0x040000,
        compressor: Xz,
        compression_options: None,
        block_log: 0x000012,
        root_inode: SquashfsDir { header },
        nodes: vec![],
    };

    fs.push_dir("oh", header).unwrap();
    fs.push_dir("oh/my", header).unwrap();
    fs.push_file(std::io::Cursor::new(data), "heyo", header).unwrap();
    fs.push_file(std::io::Cursor::new(data), "wow", header).unwrap();
    fs.push_file(std::io::Cursor::new(data), "this/is/extreme", header).unwrap();

    let mut output = std::io::Cursor::new(vec![]);
    fs.write(&mut output).unwrap();

    let _ = FilesystemReader::from_reader(output);
});
