use squashfs_deku::{Dir, FileSystem, Frag, Inode};

use std::fs::File;
use std::io::Read;

use deku::prelude::*;
use hxdmp::hexdump;

fn main() {
    let mut file = File::open("out.squashfs").unwrap();
    let mut buf = vec![];
    file.read_to_end(&mut buf).unwrap();

    let file = FileSystem::from_bytes((&buf, 0)).unwrap().1;
    println!("File: {:#02x?}", file);

    println!("Inode Metadata");
    let mut inode_bytes = file.decompress(&file.inode_metadata.data);
    let mut buffer = Vec::new();
    hexdump(&inode_bytes, &mut buffer);
    println!("{}", String::from_utf8_lossy(&buffer));
    while !inode_bytes.is_empty() {
        let ((rest, _), inode) = Inode::from_bytes((&inode_bytes, 0)).unwrap();
        println!("Inode: {:02x?}", inode);
        inode_bytes = rest.to_vec();
    }

    println!("Dir Metadata");
    let mut dir_bytes = file.decompress(&file.dir_metadata.data);
    let mut buffer = Vec::new();
    hexdump(&dir_bytes, &mut buffer);
    println!("{}", String::from_utf8_lossy(&buffer));
    let (_, dir) = Dir::from_bytes((&dir_bytes, 0)).unwrap();
    println!("Dir: {:#02x?}", dir);

    println!("Frag Metadata");
    let mut frag_bytes = file.frag_metadata.data;
    while !frag_bytes.is_empty() {
        let ((rest, _), frag) = Frag::from_bytes((&frag_bytes, 0)).unwrap();
        // TODO: 1 << 24 == uncompressed
        println!("Frag: {:02x?}", frag);
        frag_bytes = rest.to_vec();
    }
}
