use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use backhand::{FilesystemReader, FilesystemWriter};
use backhand_cli::{after_help, styles};
use clap::Parser;

// -musl malloc is slow, use jemalloc
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

/// tool to replace files in squashfs filesystems
#[derive(Parser, Debug)]
#[command(author,
          version,
          name = "replace-backhand",
          after_help = after_help(false),
          max_term_width = 98,
          styles = styles(),
)]
struct Args {
    /// Squashfs input image
    image: PathBuf,

    /// Path of file to read, to write into squashfs
    file: PathBuf,

    /// Path of file replaced in image
    #[clap(name = "FILE_PATH_IN_IMAGE")]
    file_path: PathBuf,

    /// Squashfs output image
    #[clap(short, long, default_value = "replaced.squashfs")]
    out: PathBuf,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // read of squashfs
    let file = BufReader::new(File::open(args.image).unwrap());
    let filesystem = FilesystemReader::from_reader(file).unwrap();
    let mut filesystem = FilesystemWriter::from_fs_reader(&filesystem).unwrap();

    // Modify file
    let new_file = File::open(&args.file).unwrap();
    if let Err(e) = filesystem.replace_file(args.file_path, new_file) {
        println!("[!] {e}");
        return ExitCode::FAILURE;
    }

    // write new file
    let mut output = File::create(&args.out).unwrap();
    filesystem.write(&mut output).unwrap();
    println!("replaced file and wrote to {}", args.out.display());

    ExitCode::SUCCESS
}
