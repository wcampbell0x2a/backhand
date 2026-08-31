#![no_main]

use backhand::FilesystemReader;
use libfuzzer_sys::fuzz_target;

// See bytes.rs for why this takes `&[u8]` instead of `Vec<u8>`
fuzz_target!(|data: &[u8]| {
    let reader = std::io::Cursor::new(data);
    let _ = FilesystemReader::from_reader(reader);
});
