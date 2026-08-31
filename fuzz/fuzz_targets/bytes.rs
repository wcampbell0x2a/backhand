#![no_main]

use backhand::Squashfs;
use libfuzzer_sys::fuzz_target;

// `&[u8]` and not `Vec<u8>`: the typed form goes through `Arbitrary`, which reads a
// continuation byte before every element, so the image would only ever reach the parser
// mangled (and empty for a zero-padded one). `&[u8]` is handed the input verbatim.
fuzz_target!(|data: &[u8]| {
    let reader = std::io::Cursor::new(data);
    let _ = Squashfs::from_reader(reader);
});
