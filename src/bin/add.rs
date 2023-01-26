use std::fs::File;
use std::path::PathBuf;

use backhand::filesystem::{FilesystemHeader, FilesystemReader, FilesystemWriter};
use clap::Parser;

/// tool to add files to squashfs filesystems
#[derive(Parser, Debug)]
#[command(author, version)]
struct Args {
    /// Squashfs file
    filesystem: PathBuf,

    // Path of file to read, to write into squashfs
    file: PathBuf,

    // Path of file inserted into squashfs
    file_path: PathBuf,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // read of squashfs
    let file = File::open(args.filesystem).unwrap();
    let filesystem = FilesystemReader::from_reader(file).unwrap();
    let mut filesystem = FilesystemWriter::from_fs_reader(&filesystem).unwrap();

    // create new file
    let new_file = File::open(&args.file).unwrap();
    filesystem
        .push_file(
            Box::new(new_file),
            args.file_path,
            FilesystemHeader::default(),
        )
        .unwrap();

    // write new file
    let bytes = filesystem.to_bytes().unwrap();
    std::fs::write("added.squashfs", bytes).unwrap();
    println!("added file and wrote to added.squashfs");
}
