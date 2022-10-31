use std::collections::HashMap;
use std::io::{self, Cursor, Seek, SeekFrom, Write};

use deku::bitvec::{BitVec, Msb0};
use deku::ctx::Endian;
use deku::{DekuContainerWrite, DekuWrite};
use tracing::{info, instrument, trace};

use crate::compressor::{self, CompressionOptions, Compressor};
use crate::error::SquashfsError;
use crate::inode::Inode;
use crate::{metadata, Squashfs};

// TODO: add the option of not compressing entires
// TODO: add docs
#[derive(Debug)]
struct MetadataWriter {
    compressor: Compressor,
    compression_options: Option<CompressionOptions>,
    /// Offset from the beginning of the metadata block last written
    pub(crate) metadata_start: u32,
    // All current bytes that are uncompressed
    pub(crate) uncompressed_bytes: Vec<u8>,
    // All current bytes that are compressed
    pub(crate) compressed_bytes: Vec<Vec<u8>>,
}

impl MetadataWriter {
    #[instrument(skip_all)]
    pub fn new(compressor: Compressor, compression_options: Option<CompressionOptions>) -> Self {
        Self {
            compressor,
            compression_options,
            metadata_start: 0,
            uncompressed_bytes: vec![],
            compressed_bytes: vec![],
        }
    }

    // TODO: add docs
    #[instrument(skip_all)]
    pub fn finalize(&mut self) -> Vec<u8> {
        let mut out = vec![];
        for cb in &self.compressed_bytes {
            trace!("len: {:02x?}", cb.len());
            trace!("off: {:02x?}", out.len());
            out.write_all(&(cb.len() as u16).to_le_bytes()).unwrap();
            out.write_all(cb).unwrap();
        }

        let b = compressor::compress(
            self.uncompressed_bytes.clone(),
            self.compressor,
            &self.compression_options,
        )
        .unwrap();

        trace!("len: {:02x?}", b.len());
        trace!("off: {:02x?}", out.len());
        out.write_all(&(b.len() as u16).to_le_bytes()).unwrap();
        out.write_all(&b).unwrap();

        out
    }
}

impl Write for MetadataWriter {
    // TODO: add docs
    #[instrument(skip_all)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // add all of buf into uncompressed
        self.uncompressed_bytes.write_all(buf)?;

        if self.uncompressed_bytes.len() > 0x2000 {
            trace!("time to compress");
            // "Write" the to the saved metablock
            let b = compressor::compress(
                // TODO use split_at?
                self.uncompressed_bytes[..0x2000].to_vec(),
                self.compressor,
                &self.compression_options,
            )
            .unwrap();

            // Metadata len + bytes + last metadata_start
            self.metadata_start += 2 + b.len() as u32;
            trace!("new metadata start: {:#02x?}", self.metadata_start);
            self.uncompressed_bytes = self.uncompressed_bytes[0x2000..].to_vec();
            self.compressed_bytes.push(b);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Squashfs {
    /// Serialize `Squashfs` to bytes
    ///
    /// Write all fields of `Squashfs`, while updating the following fields w.r.t the new locations
    /// within the image: `superblock`, `compression_options`, `inodes`, `root_inode`,
    /// `dir_blocks`, `fragments`, and `id`. The export table is not written to the image.
    // TODO: this function is a nightmare, each section needs to be a function, and we should move
    // this to a new module
    // TODO: support non-compression for some parts
    #[instrument(skip_all)]
    pub fn to_bytes(&self) -> Result<Vec<u8>, SquashfsError> {
        let mut c = Cursor::new(vec![]);

        // copy of the superblock to write the new positions, but we don't mutate the one stored in
        // Squashfs
        let mut write_superblock = self.superblock;

        c.write_all(&[0x00; 96])?;
        // Compression Options
        info!("Writing compressions options");
        if self.compression_options.is_some() {
            //TODO: make correct by writing the length and uncompressed Metadata
            c.write_all(&[0x08, 0x80])?;
            let mut bv: BitVec<Msb0, u8> = BitVec::new();
            self.compression_options
                .write(&mut bv, (Endian::Little, self.superblock.compressor))?;
            c.write_all(bv.as_raw_slice())?;
        }

        // Data and Fragment Bytes
        c.write_all(
            &self.data_and_fragments
                [96 + self.superblock.compression_options_size().unwrap_or(0)..],
        )?;

        // Inode Bytes
        info!("Writing Inodes");
        write_superblock.inode_table = c.position();

        let mut inode_writer =
            MetadataWriter::new(self.superblock.compressor, self.compression_options);
        let mut dir_writer =
            MetadataWriter::new(self.superblock.compressor, self.compression_options);

        let mut inode_pos = HashMap::new();

        let inodes = self.inodes.clone();
        for mut inode in inodes {
            match &mut inode {
                // If directories, we need to write the directory to `dir_bytes` and record the
                // position
                Inode::BasicDirectory(basic_dir) => {
                    //tracing::trace!("{:#02x?}", basic_dir);
                    inode_pos.insert(
                        basic_dir.header.inode_number,
                        (
                            inode_writer.metadata_start,
                            inode_writer.uncompressed_bytes.len(),
                        ),
                    );
                    if &self.root_inode == basic_dir {
                        let start = inode_writer.metadata_start;
                        let offset = inode_writer.uncompressed_bytes.len();
                        trace!("start: {start:02x?}, offset: {offset:02x?}");
                        write_superblock.root_inode = ((start << 16) as u64) | offset as u64;
                    }
                    // get the dirs from the dir_table bytes, this only works if the dir_bytes
                    // haven't been changed from initial read
                    trace!("{basic_dir:02x?}");
                    if let Some(dirs) = self.dir_from_index(
                        basic_dir.block_index as u64,
                        basic_dir.file_size as u32,
                        basic_dir.block_offset as usize,
                    )? {
                        // Mutate Inode
                        basic_dir.block_index = dir_writer.metadata_start;
                        basic_dir.block_offset = dir_writer.uncompressed_bytes.len() as u16;

                        // Mutate Dir
                        let dirs = dirs.clone();
                        for mut dir in dirs {
                            // Update the location of the inode that this Dir points at
                            //tracing::trace!("{:#02x?}", dir);
                            dir.start = inode_writer.metadata_start;
                            for entry in &mut dir.dir_entries {
                                //tracing::trace!(
                                //    "inode: {:#02x?} {:02x?}",
                                //    dir.inode_num,
                                //    entry.inode_offset
                                //);
                                let search = dir.inode_num as i16 + entry.inode_offset;
                                //tracing::trace!("inode: {:#02x?}", search);
                                let (start, un_len) = inode_pos[&(search as u32)];
                                // TODO: both starts should agree?
                                // !!!
                                dir.start = start;
                                // !!!
                                entry.offset = un_len as u16;
                            }
                            let span = tracing::span!(tracing::Level::TRACE, "dir");
                            let _enter = span.enter();
                            dir_writer.write_all(&dir.to_bytes()?)?;
                        }
                    } else {
                        //panic!("didn't find dirs");
                    }
                },
                Inode::ExtendedDirectory(extended_dir) => {
                    //tracing::trace!("{:#02x?}", basic_dir);
                    inode_pos.insert(
                        extended_dir.header.inode_number,
                        (
                            inode_writer.metadata_start,
                            inode_writer.uncompressed_bytes.len(),
                        ),
                    );

                    // get the dirs from the dir_table bytes, this only works if the dir_bytes
                    // haven't been changed from initial read
                    trace!("{extended_dir:02x?}");
                    if let Some(dirs) = self.dir_from_index(
                        extended_dir.block_index as u64,
                        extended_dir.file_size as u32,
                        extended_dir.block_offset as usize,
                    )? {
                        // Mutate Inode
                        extended_dir.block_index = dir_writer.metadata_start;
                        extended_dir.block_offset = dir_writer.uncompressed_bytes.len() as u16;

                        // Mutate Dir
                        let mut dirs = dirs.clone();
                        for dir in &mut dirs {
                            // Update the location of the inode that this Dir points at
                            dir.start = inode_writer.metadata_start;
                            for entry in &mut dir.dir_entries {
                                //tracing::trace!(
                                //    "inode: {:#02x?} {:02x?}",
                                //    dir.inode_num,
                                //    entry.inode_offset
                                //);
                                let search = dir.inode_num as i16 + entry.inode_offset;
                                //tracing::trace!("inode: {:#02x?}", search);
                                let (start, un_len) = inode_pos[&(search as u32)];
                                // TODO: both starts should agree?
                                // !!!
                                dir.start = start;
                                // !!!
                                entry.offset = un_len as u16;
                            }
                            let span = tracing::span!(tracing::Level::TRACE, "dir");
                            let _enter = span.enter();
                            dir_writer.write_all(&dir.to_bytes()?)?;
                        }
                    } else {
                        //panic!("didn't find dirs");
                    }
                },
                Inode::BasicFile(basic_file) => {
                    inode_pos.insert(
                        basic_file.header.inode_number,
                        (
                            inode_writer.metadata_start,
                            inode_writer.uncompressed_bytes.len(),
                        ),
                    );
                    //tracing::trace!("{:#02x?}", basic_file);
                },
                Inode::ExtendedFile(_) => todo!(),
                Inode::BasicSymlink(basic_symlink) => {
                    inode_pos.insert(
                        basic_symlink.header.inode_number,
                        (
                            inode_writer.metadata_start,
                            inode_writer.uncompressed_bytes.len(),
                        ),
                    );
                    //tracing::trace!("{:#02x?}", basic_symlink);
                },
                Inode::BasicBlockDevice(basic_block) => {
                    inode_pos.insert(
                        basic_block.header.inode_number,
                        (
                            inode_writer.metadata_start,
                            inode_writer.uncompressed_bytes.len(),
                        ),
                    );
                    //tracing::trace!("{:#02x?}", basic_block);
                },
                Inode::BasicCharacterDevice(basic_char) => {
                    inode_pos.insert(
                        basic_char.header.inode_number,
                        (
                            inode_writer.metadata_start,
                            inode_writer.uncompressed_bytes.len(),
                        ),
                    );
                    //tracing::trace!("{:#02x?}", basic_char);
                },
            }

            // Convert inode to bytes
            let mut v = BitVec::<Msb0, u8>::new();
            inode.write(&mut v, (0, 0)).unwrap();
            let bytes = v.as_raw_slice().to_vec();

            let span = tracing::span!(tracing::Level::TRACE, "inode");
            let _enter = span.enter();
            trace!("{:02x?}", inode_writer.uncompressed_bytes.len());
            inode_writer.write_all(&bytes)?;
        }

        // Write Inodes
        info!("Writing Inodes");
        write_superblock.inode_table = c.position();
        c.write_all(&inode_writer.finalize())?;

        // Write Dir table
        info!("Writing Dirs");
        write_superblock.dir_table = c.position();
        c.write_all(&dir_writer.finalize())?;

        // Fragment Lookup Table Bytes
        info!("Writing Fragment Lookup Table");
        if let Some(fragments) = &self.fragments {
            let fragment_table_dat = c.position();
            let bytes: Vec<u8> = fragments
                .iter()
                .flat_map(|a| a.to_bytes().unwrap())
                .collect();
            let metadata_len = metadata::set_if_uncompressed(bytes.len() as u16).to_le_bytes();
            c.write_all(&metadata_len)?;
            c.write_all(&bytes)?;
            write_superblock.frag_table = c.position();
            c.write_all(&fragment_table_dat.to_le_bytes())?;
        }

        // Export Lookup Table
        info!("Writing Export Lookup Table");
        write_superblock.export_table = 0xffffffffffffffff;
        //if let Some(export) = &self.export {
        //    let export_table_dat = c.position();
        //    let bytes: Vec<u8> = export.iter().flat_map(|a| a.to_bytes().unwrap()).collect();
        //    let metadata_len = metadata::set_if_uncompressed(bytes.len() as u16).to_le_bytes();
        //    c.write_all(&metadata_len)?;
        //    c.write_all(&bytes)?;
        //    write_superblock.export_table = c.position();
        //    c.write_all(&export_table_dat.to_le_bytes())?;
        //}

        // Export Id Table
        info!("Writing Export Id Table");
        if let Some(id) = &self.id {
            let id_table_dat = c.position();
            let bytes: Vec<u8> = id.iter().flat_map(|a| a.to_bytes().unwrap()).collect();
            let metadata_len = metadata::set_if_uncompressed(bytes.len() as u16).to_le_bytes();
            c.write_all(&metadata_len)?;
            c.write_all(&bytes)?;
            write_superblock.id_table = c.position();
            c.write_all(&id_table_dat.to_le_bytes())?;
        }

        // Pad out block_size
        info!("Writing Padding");
        write_superblock.bytes_used = c.position();
        let blocks_used = write_superblock.bytes_used as u32 / 0x1000;
        let pad_len = (blocks_used + 1) * 0x1000;
        let pad_len = pad_len - write_superblock.bytes_used as u32;
        c.write_all(&vec![0x00; pad_len as usize])?;

        // Seek back the beginning and write the superblock
        info!("Writing Superblock");
        trace!("{:#02x?}", write_superblock);
        c.seek(SeekFrom::Start(0))?;
        c.write_all(&write_superblock.to_bytes().unwrap())?;

        info!("Writing Finished");
        Ok(c.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mwriter() {
        let bytes = [0xffu8; 0x2000 - 3];

        let mut mwriter = MetadataWriter::new(Compressor::Xz, None);

        mwriter.write_all(&bytes).unwrap();
        assert_eq!(0, mwriter.metadata_start);
        assert_eq!(bytes, &*mwriter.uncompressed_bytes);
        assert!(mwriter.compressed_bytes.is_empty());

        let bytes = [0x11u8; 6];

        mwriter.write_all(&bytes).unwrap();
        assert_eq!(0x6e, mwriter.metadata_start);
        assert_eq!(bytes[3..], mwriter.uncompressed_bytes);
        assert_eq!(mwriter.compressed_bytes[0].len(), 0x6c);
    }
}
