use std::fs::{self, File, Permissions};
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};

use backhand::filesystem::{
    InnerNode, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir, SquashfsFile,
    SquashfsSymlink,
};
use backhand::Squashfs;
use clap::Parser;

/// tool to uncompress, extract and list squashfs filesystems
#[derive(Parser, Debug)]
#[command(author, version)]
struct Args {
    /// Squashfs file
    filesystem: PathBuf,

    /// Skip BYTES at the start of FILESYSTEM
    #[arg(short, long, default_value_t = 0, name = "BYTES")]
    offset: u64,

    /// List filesystem, do not write to DEST
    #[arg(short, long)]
    list: bool,

    /// Extract to [PATHNAME]
    #[arg(short, long, default_value = "squashfs-root", name = "PATHNAME")]
    dest: PathBuf,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    extract_all(&args);
}

fn extract_all(args: &Args) {
    let file = File::open(&args.filesystem).unwrap();
    let squashfs = Squashfs::from_reader_with_offset(file, args.offset).unwrap();
    let _ = fs::create_dir_all(&args.dest);
    let filesystem = squashfs.into_filesystem().unwrap();

    for node in filesystem.nodes {
        let path = node.path;
        if !args.list {
            match node.inner {
                InnerNode::File(SquashfsFile { bytes, .. }) => {
                    let path: PathBuf = path.iter().skip(1).collect();
                    tracing::debug!("file {}", path.display());
                    let filepath = Path::new(&args.dest).join(path);
                    let _ = std::fs::create_dir_all(filepath.parent().unwrap());
                    match std::fs::write(&filepath, bytes) {
                        Ok(_) => println!("[-] success, wrote {}", filepath.display()),
                        Err(e) => {
                            println!("[!] failed write: {} : {e}", filepath.display())
                        },
                    }
                },
                InnerNode::Symlink(SquashfsSymlink { link, .. }) => {
                    let path: PathBuf = path.iter().skip(1).collect();
                    tracing::debug!("symlink {} {}", path.display(), link);
                    let filepath = Path::new(&args.dest).join(path);
                    let _ = std::fs::create_dir_all(filepath.parent().unwrap());
                    if std::os::unix::fs::symlink(&link, &filepath).is_ok() {
                        println!("[-] success, wrote {}->{link}", filepath.display());
                    } else {
                        println!("[!] failed write: {}->{link}", filepath.display());
                    }
                },
                InnerNode::Dir(SquashfsDir { header, .. }) => {
                    let path: PathBuf = path.iter().skip(1).collect();
                    let path = Path::new(&args.dest).join(&path);
                    tracing::debug!("path {}", path.display());
                    let _ = std::fs::create_dir_all(&path);
                    let perms = Permissions::from_mode(u32::from(header.permissions));
                    fs::set_permissions(&path, perms).unwrap();
                    println!("[-] success, wrote {}", &path.display());
                },
                InnerNode::CharacterDevice(SquashfsCharacterDevice {
                    header: _,
                    device_number: _,
                }) => {
                    println!("[-] character device not supported");
                },
                InnerNode::BlockDevice(SquashfsBlockDevice {
                    header: _,
                    device_number: _,
                }) => {
                    println!("[-] block device not supported");
                },
            }
        } else {
            println!("{}", path.display());
        }
    }
}
