use std::fs::{self, File, Permissions};
use std::io;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};

use backhand::{
    FilesystemReader, InnerNode, ReadSeek, Squashfs, SquashfsBlockDevice, SquashfsCharacterDevice,
    SquashfsDir, SquashfsSymlink,
};
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

    /// Display filesystem superblock information
    #[arg(short, long)]
    stat: bool,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let file = File::open(&args.filesystem).unwrap();
    let squashfs = Squashfs::from_reader_with_offset(file, args.offset).unwrap();

    if args.list {
        let filesystem = squashfs.into_filesystem_reader().unwrap();
        list(filesystem);
    } else if args.stat {
        stat(squashfs);
    } else {
        let filesystem = squashfs.into_filesystem_reader().unwrap();
        extract_all(&args, filesystem);
    }
}

fn list<R: std::io::Read + std::io::Seek>(filesystem: FilesystemReader<R>) {
    for node in &filesystem.nodes {
        let path = &node.path;
        println!("{}", path.display());
    }
}

fn stat<R: ReadSeek>(squashfs: Squashfs<R>) {
    let superblock = squashfs.superblock;
    // show info about flags
    println!("{superblock:#08x?}");

    // show info about flags
    if superblock.inodes_uncompressed() {
        println!("flag: inodes uncompressed");
    }

    if superblock.data_block_stored_uncompressed() {
        println!("flag: data blocks stored uncompressed");
    }

    if superblock.fragments_stored_uncompressed() {
        println!("flag: fragments stored uncompressed");
    }

    if superblock.fragments_are_not_used() {
        println!("flag: fragments are not used");
    }

    if superblock.fragments_are_always_generated() {
        println!("flag: fragments are always generated");
    }

    if superblock.data_has_been_duplicated() {
        println!("flag: data has been duplicated");
    }

    if superblock.nfs_export_table_exists() {
        println!("flag: nfs export table exists");
    }

    if superblock.xattrs_are_stored_uncompressed() {
        println!("flag: xattrs are stored uncompressed");
    }

    if superblock.compressor_options_are_present() {
        println!("flag: compressor options are present");
    }
}

fn extract_all<R: std::io::Read + std::io::Seek>(args: &Args, filesystem: FilesystemReader<R>) {
    let _ = fs::create_dir_all(&args.dest);

    for node in &filesystem.nodes {
        let path = &node.path;
        let path: PathBuf = path.iter().skip(1).collect();
        match &node.inner {
            InnerNode::File(file) => {
                // read file
                let filepath = Path::new(&args.dest).join(path);

                // check if file exists
                if !args.force && filepath.exists() {
                    println!("[-] failed, file already exists {}", filepath.display());
                    continue;
                }

                // write to file
                let mut fd = File::create(&filepath).unwrap();
                let mut reader = filesystem.file(&file.basic).reader();
                match io::copy(&mut reader, &mut fd) {
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
                if !args.force && filepath.exists() {
                    println!("[-] failed, file already exists {}", filepath.display());
                    continue;
                }

                // remove symlink so this doesn't fail
                let _ = fs::remove_file(&filepath);

                match std::os::unix::fs::symlink(link, &filepath) {
                    Ok(_) => {
                        if args.info {
                            println!("[-] success, wrote {}->{link_display}", filepath.display());
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
    }
}
