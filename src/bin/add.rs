use std::fs::File;
use std::path::PathBuf;

use backhand::filesystem::{FilesystemHeader, Node, SquashfsFile};
use backhand::Squashfs;
use clap::Parser;

/// Binary to add file to squashfs filesystem
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Squashfs file
    input: PathBuf,

    // Path of file to read, to write into squashfs
    file: PathBuf,

    // Path of file inserted into squashfs
    file_path: PathBuf,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // read of squashfs
    let file = File::open(args.input).unwrap();
    let squashfs = Squashfs::from_reader(file).unwrap();
    let mut filesystem = squashfs.into_filesystem().unwrap();

    // create new file
    let bytes = std::fs::read(&args.file).unwrap();
    let new_file = SquashfsFile {
        header: FilesystemHeader::default(),
        path: args.file_path,
        bytes,
    };

    // insert new file
    filesystem.nodes.push(Node::File(new_file));

    // write new file
    let bytes = filesystem.to_bytes().unwrap();
    std::fs::write("added.squashfs", bytes).unwrap();
    println!("added file and wrote to added.squashfs");
}
