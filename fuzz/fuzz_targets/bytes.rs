#![no_main]

use backhand::Squashfs;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data:Vec<u8>| {
    let reader = std::io::Cursor::new(data);
    let _ = Squashfs::from_reader(reader);
});
