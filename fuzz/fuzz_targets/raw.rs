#![no_main]

use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Vec<u8>| {
    let header = NodeHeader { permissions: 0o755, uid: 0, gid: 0, mtime: 0 };

    let mut fs = FilesystemWriter::default();
    fs.set_time(0x634f5237);

    fs.push_dir("oh", header).unwrap();
    fs.push_dir("oh/my", header).unwrap();
    fs.push_file(std::io::Cursor::new(&data), "heyo", header).unwrap();
    fs.push_file(std::io::Cursor::new(&data), "wow", header).unwrap();
    fs.push_dir_all("this/is", header).unwrap();
    fs.push_file(std::io::Cursor::new(&data), "this/is/extreme", header).unwrap();

    let mut output = std::io::Cursor::new(vec![]);
    fs.write(&mut output).unwrap();

    // reset the position to the start so we can read this as a file
    output.set_position(0);
    // all files create using FilesystemWriter need to be valid
    let _ = FilesystemReader::from_reader(output).unwrap();
});
