#![no_main]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use backhand::FilesystemReader;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Vec<u8>| {
    let reader = std::io::Cursor::new(data);
    let _ = FilesystemReader::from_reader(reader);
});
