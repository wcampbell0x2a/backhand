use std::fs::File;
use std::io::BufReader;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process::ExitCode;

use backhand::{FilesystemReader, FilesystemWriter, NodeHeader};
use backhand_cli::after_help_unsquashfs;
use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

// -musl malloc is slow, use jemalloc
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

/// tool to add a file or directory to squashfs filesystems
#[derive(Parser, Debug)]
#[command(author,
          version,
          name = "add-backhand",
          after_help = after_help_unsquashfs(false),
          max_term_width = 98,
          styles = clap_cargo::style::CLAP_STYLING,
)]
struct Args {
    /// Squashfs input image
    input_image: PathBuf,

    /// Create empty directory
    #[clap(short, long)]
    dir: bool,

    /// Path of file to read, to write into squashfs
    #[clap(short, long)]
    #[clap(required_unless_present = "dir")]
    file: Option<PathBuf>,

    /// Path of file once inserted into squashfs
    #[clap(name = "FILE_PATH_IN_IMAGE")]
    path: PathBuf,

    /// Squashfs output image path
    output_image: PathBuf,

    /// Override mode read from <FILE>
    #[clap(long, required_if_eq("dir", "true"))]
    mode: Option<u16>,

    /// Override uid read from <FILE>
    #[clap(long, required_if_eq("dir", "true"))]
    uid: Option<u32>,

    /// Override gid read from <FILE>
    #[clap(long, required_if_eq("dir", "true"))]
    gid: Option<u32>,

    /// Override mtime read from <FILE>
    #[clap(long, required_if_eq("dir", "true"))]
    mtime: Option<u32>,

    /// Custom KiB padding length
    #[clap(long)]
    pad_len: Option<u32>,

    /// Don't emit compression options
    #[clap(long)]
    no_compression_options: bool,
}

fn main() -> ExitCode {
    // setup tracing to RUST_LOG or just info
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("add=info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let args = Args::parse();

    // read of squashfs
    let file = File::open(args.input_image).unwrap();
    let file = BufReader::new(file);

    let filesystem = FilesystemReader::from_reader(file).unwrap();
    let mut filesystem = FilesystemWriter::from_fs_reader(&filesystem).unwrap();

    // create new file
    if let Some(file) = args.file {
        let new_file = File::open(&file).unwrap();

        // if metadata isn't already defined, use from file
        let meta = file.metadata().unwrap();

        let mode = args.mode.unwrap_or(meta.mode() as u16) & 0xfff;
        let uid = args.uid.unwrap_or(meta.uid());
        let gid = args.gid.unwrap_or(meta.gid());
        let mtime = args.mtime.unwrap_or(meta.mtime() as u32);
        let node = NodeHeader::new(mode, uid, gid, mtime);

        if let Err(e) = filesystem.push_file(new_file, args.path, node) {
            error!("{e}");
            return ExitCode::FAILURE;
        }
    } else if args.dir {
        // use file meta from args
        let node = NodeHeader::new(
            args.mode.unwrap(),
            args.uid.unwrap(),
            args.gid.unwrap(),
            args.mtime.unwrap(),
        );
        if let Err(e) = filesystem.push_dir(args.path, node) {
            error!("{e}");
            return ExitCode::FAILURE;
        }
    }

    if let Some(pad_len) = args.pad_len {
        filesystem.set_kib_padding(pad_len)
    }

    if args.no_compression_options {
        filesystem.set_emit_compression_options(false);
    }

    // write new file
    let Ok(output) = File::create_new(&args.output_image) else {
        error!("failed to open {}", args.output_image.display());
        return ExitCode::FAILURE;
    };
    if let Err(e) = filesystem.write(output) {
        error!("{e}");
    }
    info!("added file and wrote to {}", args.output_image.display());

    ExitCode::SUCCESS
}
