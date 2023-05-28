//! Reader traits

use std::io::{BufRead, Read, Seek, SeekFrom, Write};

use deku::prelude::*;

/// Private struct containing logic to read the `Squashfs` section from a file
#[derive(Debug)]
pub(crate) struct SquashfsReaderWithOffset<R: BufReadSeek> {
    io: R,
    /// Offset from start of file to squashfs
    offset: u64,
}

impl<R: BufReadSeek> SquashfsReaderWithOffset<R> {
    pub fn new(mut io: R, offset: u64) -> std::io::Result<Self> {
        io.seek(SeekFrom::Start(offset))?;
        Ok(Self { io, offset })
    }
}

impl<R: BufReadSeek> BufRead for SquashfsReaderWithOffset<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.io.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.io.consume(amt)
    }
}

impl<R: BufReadSeek> Read for SquashfsReaderWithOffset<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.io.read(buf)
    }
}

impl<R: BufReadSeek> Seek for SquashfsReaderWithOffset<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let seek = match pos {
            SeekFrom::Start(start) => SeekFrom::Start(self.offset + start),
            seek => seek,
        };
        self.io.seek(seek).map(|x| x - self.offset)
    }
}

/// Similar to to Seek, but only require the `rewind` function
pub trait SeekRewind {
    /// Set the IO position back at the start
    fn rewind(&mut self) -> std::io::Result<()>;
}

impl<T: Seek> SeekRewind for T {
    fn rewind(&mut self) -> std::io::Result<()> {
        <Self as Seek>::rewind(self)
    }
}

/// Pseudo-Trait for Read + SeekRewind
pub trait ReadRewind: Read + SeekRewind {}
impl<T: Read + SeekRewind> ReadRewind for T {}

/// Pseudo-Trait for BufRead + SeekRewind
pub trait BufReadRewind: BufRead + SeekRewind {}
impl<T: BufRead + SeekRewind> BufReadRewind for T {}

/// Pseudo-Trait for BufRead + Seek
pub trait BufReadSeek: BufRead + Seek {}
impl<T: BufRead + Seek> BufReadSeek for T {}

/// Pseudo-Trait for Write + Seek
pub trait WriteSeek: Write + Seek {}
impl<T: Write + Seek> WriteSeek for T {}

//pub enum Reader {
//    V4_0(SquashFsReaderV4),
//}
