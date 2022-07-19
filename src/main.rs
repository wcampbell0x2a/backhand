use squashfs_deku::{FileSystem, Inode};

use std::fs::File;
use std::io::Read;

use deku::prelude::*;
use hexy::hexydump;

fn main() {
    let mut file = File::open("blah.squashfs").unwrap();
    let mut buf = vec![];
    file.read_to_end(&mut buf).unwrap();

    let file = FileSystem::from_bytes((&buf, 0)).unwrap().1;
    println!("{:02x?}", file.inode_table);
    println!("{:02x?}", file.dir_table);

    let (len, inode_metadata) = FileSystem::parse_metadata(&file.inode_metadata);
    let inode = Inode::from_bytes((inode_metadata, 0)).unwrap();
    println!("{:02x?}", len);
    println!("{:02x?}", inode);

    print!("\nInode Metadata");
    hexydump(inode_metadata, 0, inode_metadata.len());

    print!("\nDir Metadata");
    hexydump(&file.dir_metadata, 0, file.dir_metadata.len());

    print!("\nFrag Metadata");
    hexydump(&file.frag_metadata, 0, file.frag_metadata.len());
}
