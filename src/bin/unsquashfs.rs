use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use squashfs_deku::squashfs::Unsquashfs;
use squashfs_deku::Squashfs;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// squashfs file
    input: PathBuf,

    // Offset in file for Squashfs
    #[arg(short, long, default_value_t = 0)]
    offset: u64,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Extract single file from image
    ExtractFiles {
        /// Filepath to extract
        #[arg(short, long)]
        filepath: Vec<String>,

        #[arg(short, long, default_value = "output")]
        output: PathBuf,
    },
    /// Extract all files(Symlink/Files/Dirs) from image
    ExtractAll {
        #[arg(short, long, default_value = "output")]
        output: PathBuf,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    match args.cmd {
        Command::ExtractFiles { filepath, output } => {
            extract(&args.input, args.offset, filepath, &output)
        },
        Command::ExtractAll { output } => extract_all(&args.input, args.offset, &output),
    }
}

fn extract(input: &Path, offset: u64, filepath: Vec<String>, output: &Path) {
    let file = File::open(input).unwrap();
    let squashfs = Squashfs::from_reader_with_offset(file, offset).unwrap();
    tracing::info!("SuperBlock: {:#02x?}", squashfs.superblock);
    tracing::debug!("Inodes: {:#02x?}", squashfs.inodes);
    tracing::debug!("Dir Blocks: {:#02x?}", squashfs.dir_blocks);
    tracing::debug!("Root inode: {:#02x?}", squashfs.root_inode);
    tracing::debug!("Fragments {:#02x?}", squashfs.fragments);

    for filepath in &filepath {
        let (filepath, bytes) = squashfs.extract_file(filepath).unwrap();
        let filepath = Path::new(output).join(filepath);
        //println!("[-] {}", filepath.parent().unwrap().display());
        let _ = std::fs::create_dir_all(filepath.parent().unwrap());
        match std::fs::write(&filepath, bytes) {
            Ok(_) => println!("[-] success, wrote to {}", filepath.display()),
            Err(e) => {
                println!("[!] failed to write: {} : {e}", filepath.display())
            },
        }
    }
}

fn extract_all(input: &Path, offset: u64, output: &Path) {
    let file = File::open(input).unwrap();
    let squashfs = Squashfs::from_reader_with_offset(file, offset).unwrap();
    tracing::info!("SuperBlock: {:#02x?}", squashfs.superblock);
    tracing::debug!("Inodes: {:#02x?}", squashfs.inodes);
    tracing::debug!("Dir Blocks: {:#02x?}", squashfs.dir_blocks);
    tracing::debug!("Root inode: {:#02x?}", squashfs.root_inode);
    tracing::debug!("Fragments {:#02x?}", squashfs.fragments);

    fs::create_dir_all(output).unwrap();
    for unsquashfs_file in squashfs.extract_all_files().unwrap() {
        match unsquashfs_file {
            Unsquashfs::File((filepath, bytes)) => {
                let filepath = Path::new(output).join(filepath);
                let _ = std::fs::create_dir_all(filepath.parent().unwrap());
                match std::fs::write(&filepath, bytes) {
                    Ok(_) => println!("[-] success, wrote {}", filepath.display()),
                    Err(e) => {
                        println!("[!] failed write: {} : {e}", filepath.display())
                    },
                }
            },
            Unsquashfs::Symlink((filepath, _, link)) => {
                let filepath = Path::new(output).join(filepath);
                let _ = std::fs::create_dir_all(filepath.parent().unwrap());
                if std::os::unix::fs::symlink(&link, &filepath).is_ok() {
                    println!("[-] success, wrote {}->{link}", filepath.display());
                } else {
                    println!("[!] failed write: {}->{link}", filepath.display());
                }
            },
            Unsquashfs::Path(path) => {
                let path = Path::new(output).join(path);
                let _ = std::fs::create_dir_all(&path);
                println!("[-] success, wrote {}", &path.display());
            },
        }
    }
}
