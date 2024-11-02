use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use backhand::{FilesystemReader, FilesystemWriter};
use backhand_cli::after_help;
use clap::Parser;
use tracing::error;
use tracing_subscriber::EnvFilter;

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
          styles = clap_cargo::style::CLAP_STYLING,
)]
struct Args {
    /// Squashfs input image
    input_image: PathBuf,

    /// Path of file to read, to write into squashfs
    file: PathBuf,

    /// Path of file replaced in image
    #[clap(name = "FILE_PATH_IN_IMAGE")]
    file_path: PathBuf,

    /// Squashfs output image
    output_image: PathBuf,

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
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("replace=info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let args = Args::parse();

    // read of squashfs
    let Ok(file) = File::open(&args.input_image) else {
        error!("unable to open {}", args.input_image.display());
        return ExitCode::FAILURE;
    };
    let file = BufReader::new(file);
    let filesystem = FilesystemReader::from_reader(file).unwrap();
    let mut filesystem = FilesystemWriter::from_fs_reader(&filesystem).unwrap();

    // Modify file
    let Ok(new_file) = File::open(&args.file) else {
        error!("unable to open {}", args.file.display());
        return ExitCode::FAILURE;
    };
    if let Err(e) = filesystem.replace_file(args.file_path, new_file) {
        error!("{e}");
        return ExitCode::FAILURE;
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
    filesystem.write(output).unwrap();
    println!("replaced file and wrote to {}", args.output_image.display());

    ExitCode::SUCCESS
}
