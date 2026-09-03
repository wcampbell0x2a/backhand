#![no_main]

use backhand::create_squashfs_from_kind;
use backhand::kind::{Kind, LE_V1_0, LE_V2_0};
use libfuzzer_sys::fuzz_target;

// Read arbitrary bytes as a v1 and as a v2 image. These versions address inodes
// by position inside a decompressed table, so this target checks that a
// reference past the end of that table reports an error rather than panicking.
fuzz_target!(|data: &[u8]| {
    for inner in [LE_V1_0, LE_V2_0] {
        let reader = std::io::Cursor::new(data);
        let kind = Kind::from_const(inner).unwrap();
        let _ = create_squashfs_from_kind(reader, 0, kind);
    }
});
