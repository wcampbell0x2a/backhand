use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use squashfs_deku::Squashfs;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// squashfs file
    input: PathBuf,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    ExtractFiles {
        /// Name of file to extract
        #[arg(short, long)]
        filenames: Vec<String>,

        #[arg(short, long, default_value = "output")]
        output: PathBuf,
    },
}

fn main() {
    //simple_logger::SimpleLogger::new().init().unwrap();
    //
    let args = Args::parse();

    match args.cmd {
        Command::ExtractFiles { filenames, output } => extract(&args.input, filenames, &output),
    }
}

fn extract(input: &Path, filenames: Vec<String>, output: &Path) {
    let file = File::open(input).unwrap();

    let mut squashfs = Squashfs::from_reader(file);
    println!("SuperBlock: {:#02x?}", squashfs.superblock);

    let dirs = squashfs.dirs();
    let inodes = squashfs.inodes();
    let fragments = squashfs.fragments();

    for filename in &filenames {
        fs::create_dir_all(output).unwrap();
        let bytes = squashfs.extract_file(filename, &dirs, &inodes, &fragments);
        let path = format!("{}/{filename}", output.to_str().unwrap());
        std::fs::write(&path, bytes).unwrap();
        println!("Success, wrote {path}");
    }
}
