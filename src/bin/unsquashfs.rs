use std::fs::{self, File, Permissions};
use std::io::Read;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};

use backhand::filesystem::{
    InnerNode, SquashfsBlockDevice, SquashfsCharacterDevice, SquashfsDir, SquashfsSymlink,
};
use backhand::Squashfs;
use clap::Parser;
use nix::sys::stat::{mknod, Mode, SFlag};

/// tool to uncompress, extract and list squashfs filesystems
#[derive(Parser, Debug)]
#[command(author, version, name = "unsquashfs-backhand")]
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

    /// Print files as they are extracted
    #[arg(short, long)]
    info: bool,

    /// If file already exists then overwrite
    #[arg(short, long)]
    force: bool,
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
    let filesystem = squashfs.into_filesystem_reader().unwrap();

    for node in &filesystem.nodes {
        let path = &node.path;
        if !args.list {
            let path: PathBuf = path.iter().skip(1).collect();
            match &node.inner {
                InnerNode::File(file) => {
                    // read file
                    let filepath = Path::new(&args.dest).join(path);

                    // check if file exists
                    if !args.force {
                        if filepath.exists() {
                            println!("[-] failed, file already exists {}", filepath.display());
                            continue;
                        }
                    }
                    let mut bytes = Vec::with_capacity(file.basic.file_size as usize);
                    let mut reader = filesystem.file(&file.basic);
                    reader.read_to_end(&mut bytes).unwrap();
                    // write file
                    match std::fs::write(&filepath, bytes) {
                        Ok(_) => {
                            if args.info {
                                println!("[-] success, wrote {}", filepath.display());
                            }
                            // write permissions
                            let perms = Permissions::from_mode(u32::from(file.header.permissions));
                            fs::set_permissions(&filepath, perms).unwrap();
                        },
                        Err(e) => {
                            println!("[!] failed write: {} : {e}", filepath.display())
                        },
                    }
                },
                InnerNode::Symlink(SquashfsSymlink { link, .. }) => {
                    // create symlink
                    let link_display = link.display();
                    let filepath = Path::new(&args.dest).join(path);

                    // check if file exists
                    if !args.force {
                        if filepath.exists() {
                            println!("[-] failed, file already exists {}", filepath.display());
                            continue;
                        }
                    }

                    // remove symlink so this doesn't fail
                    let _ = fs::remove_file(&filepath);

                    match std::os::unix::fs::symlink(link, &filepath) {
                        Ok(_) => {
                            if args.info {
                                println!(
                                    "[-] success, wrote {}->{link_display}",
                                    filepath.display()
                                );
                            }
                        },
                        Err(e) => println!(
                            "[!] failed write: {}->{link_display} : {e}",
                            filepath.display()
                        ),
                    }
                },
                InnerNode::Dir(SquashfsDir { header }) => {
                    // create dir
                    let path = Path::new(&args.dest).join(path);
                    let _ = std::fs::create_dir(&path);

                    // set permissions
                    let perms = Permissions::from_mode(u32::from(header.permissions));
                    fs::set_permissions(&path, perms).unwrap();
                    if args.info {
                        println!("[-] success, wrote {}", &path.display());
                    }
                },
                InnerNode::CharacterDevice(SquashfsCharacterDevice {
                    header,
                    device_number,
                }) => {
                    let path = Path::new(&args.dest).join(path);
                    match mknod(
                        &path,
                        SFlag::S_IFCHR,
                        Mode::from_bits(u32::from(header.permissions)).unwrap(),
                        u64::from(*device_number),
                    ) {
                        Ok(_) => {
                            if args.info {
                                println!("[-] char device created: {}", path.display());
                            }
                        },
                        Err(_) => {
                            println!(
                                "[!] could not create char device {}, are you superuser?",
                                path.display()
                            );
                        },
                    }
                },
                InnerNode::BlockDevice(SquashfsBlockDevice {
                    header,
                    device_number,
                }) => {
                    let path = Path::new(&args.dest).join(path);
                    match mknod(
                        &path,
                        SFlag::S_IFBLK,
                        Mode::from_bits(u32::from(header.permissions)).unwrap(),
                        u64::from(*device_number),
                    ) {
                        Ok(_) => {
                            if args.info {
                                println!("[-] block device created: {}", path.display());
                            }
                        },
                        Err(_) => {
                            println!(
                                "[!] could not create block device {}, are you superuser?",
                                path.display()
                            );
                        },
                    }
                },
            }
        } else {
            // --list
            println!("{}", path.display());
        }
    }
}
