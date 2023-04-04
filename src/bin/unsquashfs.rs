use std::fs::{self, File, Permissions};
use std::io;
use std::os::unix::prelude::{OsStrExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use backhand::kind::Kind;
use backhand::{
    FilesystemReader, InnerNode, NodeHeader, ReadSeek, Squashfs, SquashfsBlockDevice,
    SquashfsCharacterDevice, SquashfsDir, SquashfsSymlink,
};
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::Parser;
use libc::lchown;
use nix::libc::geteuid;
use nix::sys::stat::{mknod, umask, utimensat, utimes, Mode, SFlag, UtimensatFlags};
use nix::sys::time::{TimeSpec, TimeVal};

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

    /// Kind(type of image) to parse
    #[arg(short, long,
          default_value = "le_v4_0",
          value_parser = PossibleValuesParser::new(
              [
                "be_v4_0",
                "le_v4_0",
                "amv_be_v4_0",
              ]
          ).map(|s| Kind::from_str(&s).unwrap())
    )]
    kind: Kind,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let file = File::open(&args.filesystem).unwrap();
    let squashfs =
        Squashfs::from_reader_with_offset_and_kind(file, args.offset, args.kind).unwrap();
    let root_process = unsafe { geteuid() == 0 };
    if root_process {
        umask(Mode::from_bits(0).unwrap());
    }

    if args.list {
        let filesystem = squashfs.into_filesystem_reader().unwrap();
        list(filesystem);
    } else if args.stat {
        stat(squashfs);
    } else {
        let filesystem = squashfs.into_filesystem_reader().unwrap();
        extract_all(&args, filesystem, root_process);
    }
}

fn list(filesystem: FilesystemReader) {
    for node in filesystem.files() {
        let path = &node.fullpath;
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

fn set_attributes(path: &Path, header: &NodeHeader, root_process: bool, is_file: bool) {
    // TODO Use (file_set_times) when not nightly: https://github.com/rust-lang/rust/issues/98245
    let timeval = TimeVal::new(i64::from(header.mtime), 0);
    utimes(path, &timeval, &timeval).unwrap();

    let mut mode = u32::from(header.permissions);

    // Only chown when root
    if root_process {
        // TODO: Use (unix_chown) when not nightly: https://github.com/rust-lang/rust/issues/88989
        let path_bytes = PathBuf::from(path)
            .as_os_str()
            .as_bytes()
            .as_ptr()
            .cast::<i8>();
        unsafe {
            lchown(path_bytes, u32::from(header.uid), u32::from(header.gid));
        }
    } else if is_file {
        // bitwise-not if not rooted (disable write permissions for user/group). Following
        // squashfs-tools/unsquashfs behaviour
        mode &= !0o022;
    }

    // set permissions
    //
    // NOTE: In squashfs-tools/unsquashfs they remove the write bits for user and group?
    // I don't know if there is a reason for that but I keep the permissions the same if possible
    match fs::set_permissions(path, Permissions::from_mode(mode)) {
        Ok(_) => (),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                // try without sticky bit
                if fs::set_permissions(path, Permissions::from_mode(mode & !1000)).is_err() {
                    println!("[!] could not set permissions");
                }
            }
        },
    }
}

fn extract_all(args: &Args, filesystem: FilesystemReader, root_process: bool) {
    // TODO: fixup perms for this?
    let _ = fs::create_dir_all(&args.dest);

    // alloc required space for file data readers
    let (mut buf_read, mut buf_decompress) = filesystem.alloc_read_buffers();

    for node in filesystem.files() {
        let path = &node.fullpath;
        let path = path.strip_prefix(Component::RootDir).unwrap_or(path);
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
                let mut reader = filesystem
                    .file(&file.basic)
                    .reader(&mut buf_read, &mut buf_decompress);

                match io::copy(&mut reader, &mut fd) {
                    Ok(_) => {
                        if args.info {
                            println!("[-] success, wrote {}", filepath.display());
                        }

                        set_attributes(&filepath, &node.header, root_process, true);
                    },
                    Err(e) => {
                        println!("[!] failed write: {} : {e}", filepath.display());
                        continue;
                    },
                }
            },
            InnerNode::Symlink(SquashfsSymlink { link }) => {
                // create symlink
                let link_display = link.display();
                let filepath = Path::new(&args.dest).join(path);

                // check if file exists
                if !args.force && filepath.exists() {
                    println!("[-] failed, file already exists {}", filepath.display());
                    continue;
                }

                match std::os::unix::fs::symlink(link, &filepath) {
                    Ok(_) => {
                        if args.info {
                            println!("[-] success, wrote {}->{link_display}", filepath.display());
                        }
                    },
                    Err(e) => {
                        println!(
                            "[!] failed write: {}->{link_display} : {e}",
                            filepath.display()
                        );
                        continue;
                    },
                }

                // set attributes, but special to not follow the symlink

                if root_process {
                    // TODO: Use (unix_chown) when not nightly: https://github.com/rust-lang/rust/issues/88989
                    let path_bytes = PathBuf::from(&filepath)
                        .as_os_str()
                        .as_bytes()
                        .as_ptr()
                        .cast::<i8>();
                    unsafe {
                        lchown(
                            path_bytes,
                            u32::from(node.header.uid),
                            u32::from(node.header.gid),
                        );
                    }
                }

                // TODO Use (file_set_times) when not nightly: https://github.com/rust-lang/rust/issues/98245
                // Make sure this doesn't follow symlinks when changed to std library!
                let timespec = TimeSpec::new(i64::from(node.header.mtime), 0);
                utimensat(
                    None,
                    &filepath,
                    &timespec,
                    &timespec,
                    UtimensatFlags::NoFollowSymlink,
                )
                .unwrap();
            },
            InnerNode::Dir(SquashfsDir { .. }) => {
                // create dir
                let path = Path::new(&args.dest).join(path);
                let _ = std::fs::create_dir(&path);

                // These permissionsn are corrected later (user default permissions for now)

                if args.info {
                    println!("[-] success, wrote {}", &path.display());
                }
            },
            InnerNode::CharacterDevice(SquashfsCharacterDevice { device_number }) => {
                let path = Path::new(&args.dest).join(path);
                if root_process {
                    match mknod(
                        &path,
                        SFlag::S_IFCHR,
                        Mode::from_bits(u32::from(node.header.permissions)).unwrap(),
                        u64::from(*device_number),
                    ) {
                        Ok(_) => {
                            if args.info {
                                println!("[-] char device created: {}", path.display());
                            }

                            set_attributes(&path, &node.header, root_process, true);
                        },
                        Err(_) => {
                            println!(
                                "[!] could not create char device {}, are you superuser?",
                                path.display()
                            );
                            continue;
                        },
                    }
                } else {
                    println!(
                        "[!] could not create char device {}, you are not superuser!",
                        path.display()
                    );
                    continue;
                }
            },
            InnerNode::BlockDevice(SquashfsBlockDevice { device_number }) => {
                let path = Path::new(&args.dest).join(path);
                match mknod(
                    &path,
                    SFlag::S_IFBLK,
                    Mode::from_bits(u32::from(node.header.permissions)).unwrap(),
                    u64::from(*device_number),
                ) {
                    Ok(_) => {
                        if args.info {
                            println!("[-] block device created: {}", path.display());
                        }

                        set_attributes(&path, &node.header, root_process, true);
                    },
                    Err(_) => {
                        println!(
                            "[!] could not create block device {}, are you superuser?",
                            path.display()
                        );
                        continue;
                    },
                }
            },
            InnerNode::FilePhase2(_, _) => unreachable!(),
        }
    }

    // fixup dir permissions
    for node in filesystem.files() {
        if let InnerNode::Dir(SquashfsDir { .. }) = &node.inner {
            let path = &node.fullpath;
            let path = path.strip_prefix(Component::RootDir).unwrap_or(path);
            let path = Path::new(&args.dest).join(path);
            set_attributes(&path, &node.header, root_process, false);
        }
    }
}
