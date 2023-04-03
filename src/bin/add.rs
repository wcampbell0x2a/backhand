use std::fs::File;
use std::os::unix::fs::MetadataExt;
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

    /// Overide mode read from <FILE>
    #[clap(long)]
    mode: Option<u16>,

    /// Overide uid read from <FILE>
    #[clap(long)]
    uid: Option<u16>,

    /// Overide gid read from <FILE>
    #[clap(long)]
    gid: Option<u16>,

    /// Overide mtime read from <FILE>
    #[clap(long)]
    mtime: Option<u32>,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // read of squashfs
    let file = File::open(args.image).unwrap();
    let meta = file.metadata().unwrap();

    let mode = args.mode.unwrap_or(meta.mode() as u16) & 0xfff;
    let uid = args.uid.unwrap_or(meta.uid() as u16);
    let gid = args.gid.unwrap_or(meta.gid() as u16);
    let mtime = args.mtime.unwrap_or(meta.mtime() as u32);
    let node = NodeHeader::new(mode, uid, gid, mtime);

    let filesystem = FilesystemReader::from_reader(file).unwrap();
    let mut filesystem = FilesystemWriter::from_fs_reader(&filesystem).unwrap();

    // create new file
    let new_file = File::open(&args.file).unwrap();
    filesystem
        .push_file(new_file, args.file_path, node)
        .unwrap();

    // write new file
    let mut output = File::create(&args.out).unwrap();
    filesystem.write(&mut output).unwrap();
    println!("added file and wrote to {}", args.out.display());
}
