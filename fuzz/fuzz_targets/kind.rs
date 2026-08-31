#![no_main]

use std::io::{Cursor, Write};

use backhand::kind::Kind;
use backhand::traits::filesystem::BackhandInnerNode;
use backhand::create_squashfs_from_kind;
use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;

// bytes.rs, filesystem.rs and extract.rs all parse as `LE_V4_0` at offset 0, which is the
// only shape `FilesystemReader::from_reader` can produce. That leaves three things
// unfuzzed: the v3 readers, the big-endian and vendor v4 layouts, and the "image starts
// partway into the file" path firmware dumps need. `create_squashfs_from_kind` is the
// entry point that reaches all of them.

/// Every kind the library knows. `Kind::from_target` is feature-gated, so a name the build
/// does not support simply drops out of the table below rather than failing to compile.
const KINDS: &[&str] = &[
    "le_v4_0",
    "be_v4_0",
    "avm_be_v4_0",
    "be_v3_0",
    "le_v3_0",
    "le_v3_0_lzma",
    "be_v3_0_lzma",
    "netgear_be_v3_0_lzma",
    "netgear_be_v3_0_lzma_standard",
    "le_v3_1_lzma_swap",
    "be_v3_1_lzma_swap",
];

/// See extract.rs: a corrupt image can claim far more data than it holds, so cap what the
/// whole image is allowed to yield.
const MAX_TOTAL_BYTES: u64 = 64 << 20;

/// Discards what it is given and reports "no space left" once `budget` is spent, so a file
/// claiming an absurd size ends the read instead of the process.
struct Capped {
    budget: u64,
}

impl Write for Capped {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.budget = match self.budget.checked_sub(buf.len() as u64) {
            Some(left) => left,
            None => return Err(std::io::ErrorKind::WriteZero.into()),
        };
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Arbitrary)]
struct Input<'a> {
    kind: u8,
    /// Images are embedded at an offset in firmware dumps; small values keep the offset
    /// inside the input often enough to be reachable.
    offset: u8,
    /// Last field, so `Arbitrary` hands it the rest of the input verbatim rather than
    /// mangling it the way a `Vec<u8>` would.
    image: &'a [u8],
}

fuzz_target!(|input: Input| {
    let name = KINDS[usize::from(input.kind) % KINDS.len()];
    let Ok(kind) = Kind::from_target(name) else {
        // not built with the feature this kind needs
        return;
    };

    let reader = Cursor::new(input.image);
    let Ok(fs) = create_squashfs_from_kind(reader, u64::from(input.offset), kind) else {
        return;
    };

    let mut sink = Capped { budget: MAX_TOTAL_BYTES };
    for node in fs.files() {
        if let BackhandInnerNode::File(file) = &node.inner {
            // A truncated or corrupt block is an error, not a bug: only a panic, a hang,
            // or a memory-safety fault is a finding here.
            if fs.file_data_to_writer(file, &mut sink).is_err() && sink.budget == 0 {
                break;
            }
        }
    }
});
