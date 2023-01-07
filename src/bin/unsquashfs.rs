use std::fs::{self, File, Permissions};
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};

use backhand::filesystem::{
    Node, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsFile, SquashfsPath, SquashfsSymlink,
};
use backhand::Squashfs;
use clap::{Parser, Subcommand};

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
    ///// Extract single file from image
    //ExtractFiles {
    //    /// Filepath to extract
    //    #[arg(short, long)]
    //    filepath: Vec<String>,

    //    #[arg(short, long, default_value = "output")]
    //    output: PathBuf,
    //},
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
        //Command::ExtractFiles { filepath, output } => {
        //    extract(&args.input, args.offset, filepath, &output)
        //},
        Command::ExtractAll { output } => extract_all(&args.input, args.offset, &output),
    }
}

fn extract_all(input: &Path, offset: u64, output: &Path) {
    let file = File::open(input).unwrap();
    let squashfs = Squashfs::from_reader_with_offset(file, offset).unwrap();
    tracing::info!("SuperBlock: {:#02x?}", squashfs.superblock);
    tracing::info!("Inodes: {:#02x?}", squashfs.inodes);
    tracing::info!("Dirs: {:#02x?}", squashfs.all_dirs());
    tracing::info!("Id: {:#02x?}", squashfs.id);
    tracing::info!("Root inode: {:#02x?}", squashfs.root_inode);
    tracing::info!("Fragments {:#02x?}", squashfs.fragments);

    fs::create_dir_all(output).unwrap();
    let filesystem = squashfs.into_filesystem().unwrap();

    for node in filesystem.nodes {
        match node {
            Node::File(SquashfsFile { path, bytes, .. }) => {
                let path: PathBuf = path.iter().skip(1).collect();
                tracing::debug!("file {}", path.display());
                let filepath = Path::new(output).join(path);
                let _ = std::fs::create_dir_all(filepath.parent().unwrap());
                match std::fs::write(&filepath, bytes) {
                    Ok(_) => println!("[-] success, wrote {}", filepath.display()),
                    Err(e) => {
                        println!("[!] failed write: {} : {e}", filepath.display())
                    },
                }
            },
            Node::Symlink(SquashfsSymlink { path, link, .. }) => {
                let path: PathBuf = path.iter().skip(1).collect();
                tracing::debug!("symlink {} {}", path.display(), link);
                let filepath = Path::new(output).join(path);
                let _ = std::fs::create_dir_all(filepath.parent().unwrap());
                if std::os::unix::fs::symlink(&link, &filepath).is_ok() {
                    println!("[-] success, wrote {}->{link}", filepath.display());
                } else {
                    println!("[!] failed write: {}->{link}", filepath.display());
                }
            },
            Node::Path(SquashfsPath { header, path, .. }) => {
                let path: PathBuf = path.iter().skip(1).collect();
                let path = Path::new(output).join(&path);
                tracing::debug!("path {}", path.display());
                let _ = std::fs::create_dir_all(&path);
                let perms = Permissions::from_mode(u32::from(header.permissions));
                fs::set_permissions(&path, perms).unwrap();
                println!("[-] success, wrote {}", &path.display());
            },
            Node::CharacterDevice(SquashfsCharacterDevice {
                header: _,
                device_number: _,
                path: _,
            }) => {
                println!("[-] character device not supported");
            },
            Node::BlockDevice(SquashfsBlockDevice {
                header: _,
                device_number: _,
                path: _,
            }) => {
                println!("[-] block device not supported");
            },
        }
    }
}
