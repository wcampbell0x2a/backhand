use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use squashfs_deku::Squashfs;

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
    ExtractAll {
        #[arg(short, long, default_value = "output")]
        output: PathBuf,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    match args.cmd {
        Command::ExtractFiles { filenames, output } => extract(&args.input, filenames, &output),
        Command::ExtractAll { output } => extract_all(&args.input, &output),
    }
}

fn extract(input: &Path, filenames: Vec<String>, output: &Path) {
    let file = File::open(input).unwrap();

    let squashfs = Squashfs::from_reader(file).unwrap();
    tracing::info!("SuperBlock: {:#02x?}", squashfs.superblock);
    tracing::debug!("Inodes: {:#02x?}", squashfs.inodes);
    tracing::debug!("Root inode: {:#02x?}", squashfs.root_inode);
    tracing::debug!("Fragments {:#02x?}", squashfs.fragments);

    for filename in &filenames {
        fs::create_dir_all(output).unwrap();
        let (filepath, bytes) = squashfs.extract_file(&squashfs, filename).unwrap();
        let path = format!("{}/{filename}", output.to_str().unwrap());
        std::fs::write(&path, bytes).unwrap();
        println!("[-] squashfs filepath: {}", filepath.display());
        println!("[-] success, wrote to {path}");
    }
}

fn extract_all(input: &Path, output: &Path) {
    let file = File::open(input).unwrap();

    let squashfs = Squashfs::from_reader(file).unwrap();
    tracing::info!("SuperBlock: {:#02x?}", squashfs.superblock);
    tracing::debug!("Inodes: {:#02x?}", squashfs.inodes);
    tracing::debug!("Root inode: {:#02x?}", squashfs.root_inode);
    tracing::debug!("Fragments {:#02x?}", squashfs.fragments);

    fs::create_dir_all(output).unwrap();
    let filepath_bytes = squashfs.extract_all_files(&squashfs).unwrap();
    for (filepath, bytes) in filepath_bytes {
        let filepath = Path::new(output).join(filepath);
        std::fs::write(&filepath, bytes).unwrap();
        println!("[-] success, wrote to {}", filepath.display());
    }
}
