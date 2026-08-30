#![no_main]

use backhand::FilesystemReader;
use backhand::kind::{Kind, LE_V4_0_LZMA};
use libfuzzer_sys::fuzz_target;

// Read arbitrary bytes as an LZMA image. This kind finds its decompression
// parameters by trying candidates, so this target checks that the search ends
// on input that decompresses with none of them.
fuzz_target!(|data: &[u8]| {
    let reader = std::io::Cursor::new(data);
    let kind = Kind::from_const(LE_V4_0_LZMA).unwrap();
    let _ = FilesystemReader::from_reader_with_offset_and_kind(reader, 0, kind);
});
