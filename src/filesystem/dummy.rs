use std::io::{Read, Seek, SeekFrom};

/// Used in situations that FilesystemWriter is created without a previously
/// existing squashfs, if used, just panic.
pub struct DummyReadSeek;

impl Read for DummyReadSeek {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        unreachable!()
    }
}

impl Seek for DummyReadSeek {
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        unreachable!()
    }
}
