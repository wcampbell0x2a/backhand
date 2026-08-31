#![no_main]

use std::io::{Cursor, Read};

use backhand::{FilesystemReader, InnerNode};
use libfuzzer_sys::fuzz_target;

// See bytes.rs for why this takes `&[u8]` instead of `Vec<u8>`.
//
// bytes.rs and filesystem.rs stop once the metadata parses; everything past that point -
// the block reader, the fragment lookup, and the decompressors - is only reached by
// actually pulling a file's contents out. That is where a corrupt image gets to pick block
// sizes, fragment offsets and compressed lengths, so it is worth a target of its own.

/// A corrupt image can claim a file far larger than the image itself, and a decompressor
/// handed a hostile block can expand it by orders of magnitude. Cap both what one file may
/// yield and what the whole image may yield, so a small input cannot turn into an OOM that
/// says nothing about the parser.
const MAX_FILE_BYTES: u64 = 8 << 20;
const MAX_TOTAL_BYTES: u64 = 64 << 20;

fuzz_target!(|data: &[u8]| {
    let Ok(fs) = FilesystemReader::from_reader(Cursor::new(data)) else {
        return;
    };

    let mut budget = MAX_TOTAL_BYTES;
    let mut buf = Vec::new();
    for node in fs.files() {
        match &node.inner {
            InnerNode::File(file) => {
                // Both entry points: `reader` treats an out-of-range fragment index as "no
                // fragment", `reader_checked` reports it. They take different paths through
                // the fragment table, so exercise each.
                let _ = fs.file(file).reader_checked();

                buf.clear();
                let limit = MAX_FILE_BYTES.min(budget);
                let mut reader = fs.file(file).reader().take(limit);
                // A truncated or corrupt block is an error, not a bug: only a panic,
                // a hang, or a memory-safety fault is a finding here.
                let _ = reader.read_to_end(&mut buf);
                budget = budget.saturating_sub(buf.len() as u64);
                if budget == 0 {
                    break;
                }
            }
            // Touch the other node payloads so a bogus one is at least walked over
            InnerNode::Symlink(link) => {
                let _ = link.link.as_os_str().len();
            }
            InnerNode::CharacterDevice(dev) => {
                let _ = dev.device_number;
            }
            InnerNode::BlockDevice(dev) => {
                let _ = dev.device_number;
            }
            InnerNode::Dir(_) | InnerNode::NamedPipe | InnerNode::Socket => {}
        }
    }
});
