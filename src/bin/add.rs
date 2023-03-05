use std::fs::File;
use std::path::PathBuf;

use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
use clap::Parser;

/// tool to add files to squashfs filesystems
#[derive(Parser, Debug)]
#[command(author, version, name = "add-backhand")]
struct Args {
    /// Squashfs input image
    image: PathBuf,

    /// Path of file to read, to write into squashfs
    file: PathBuf,

    /// Path of file once inserted into squashfs
    #[clap(name = "FILE_PATH_IN_IMAGE")]
    file_path: PathBuf,

    /// Squashfs output image
    #[clap(short, long, default_value = "added.squashfs")]
    out: PathBuf,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // read of squashfs
    let file = File::open(args.image).unwrap();
    let filesystem = FilesystemReader::from_reader(file).unwrap();
    let mut filesystem = FilesystemWriter::from_fs_reader(&filesystem).unwrap();

    // create new file
    let new_file = File::open(&args.file).unwrap();
    filesystem.push_file(new_file, args.file_path, NodeHeader::default());

    // write new file
    let mut output = File::create(&args.out).unwrap();
    filesystem.write(&mut output).unwrap();
    println!("added file and wrote to {}", args.out.display());
}
