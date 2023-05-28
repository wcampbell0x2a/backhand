#[path = "../../common/common.rs"]
mod common;
use std::fs::{self, File, Permissions};
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::os::unix::prelude::{OsStrExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use backhand::kind::Kind;
use backhand::{
    BufReadSeek, FilesystemReader, InnerNode, Node, NodeHeader, Squashfs, SquashfsBlockDevice,
    SquashfsCharacterDevice, SquashfsDir, SquashfsFileReader, SquashfsSymlink, SuperBlock,
    SuperBlock_V4_0,
};
use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use common::after_help;
use libc::lchown;
use nix::libc::geteuid;
use nix::sys::stat::{mknod, umask, utimensat, utimes, Mode, SFlag, UtimensatFlags};
use nix::sys::time::{TimeSpec, TimeVal};

pub fn required_root(a: &str) -> Result<PathBuf, String> {
    let p = PathBuf::try_from(a).or(Err("could not".to_string()))?;

    if p.has_root() {
        Ok(p)
    } else {
        Err("argument requires root \"/\"".to_string())
    }
}

fn find_offset(file: &mut BufReader<File>, kind: &Kind) -> Option<u64> {
    let mut magic = [0_u8; 4];
    while file.read_exact(&mut magic).is_ok() {
        if magic == kind.magic() {
            let found = file.stream_position().unwrap() - magic.len() as u64;
            file.rewind().unwrap();
            return Some(found);
        }
    }
    file.rewind().unwrap();
    None
}

/// tool to uncompress, extract and list squashfs filesystems
#[derive(Parser)]
#[command(author,
          version,
          name = "unsquashfs-backhand",
          after_help = after_help(),
          max_term_width = 98,
)]
struct Args {
    /// Squashfs file
    ///
    /// Required for all usage, except --completions
    #[arg(required_unless_present = "completions")]
    filesystem: Option<PathBuf>,

    /// Skip BYTES at the start of FILESYSTEM
    #[arg(short, long, default_value_t = 0, name = "BYTES")]
    offset: u64,

    /// Find first instance of squashfs --kind magic
    ///
    /// Will overwrite given --offset
    #[arg(short, long)]
    auto_offset: bool,

    /// List filesystem, do not write to DEST
    #[arg(short, long)]
    list: bool,

    /// Extract to [PATHNAME]
    #[arg(short, long, default_value = "squashfs-root", name = "PATHNAME")]
    dest: PathBuf,

    /// Print files as they are extracted
    #[arg(short, long)]
    info: bool,

    /// Limit filesystem extraction
    ///
    /// For example, "/www/webpages/data" will return all files under that dir, such as
    /// "/www/webpages/data/region.json" and "/www/webpages/data/timezone.json". When given an
    /// exact file, only that file will be extracted.
    ///
    /// Like normal operation, these will be extracted as {arg.dest}{arg.path_filter}{files} with
    /// correct file permissions.
    #[arg(long, default_value = "/", value_parser = required_root)]
    path_filter: PathBuf,

    /// If file already exists then overwrite
    #[arg(short, long)]
    force: bool,

    /// Display filesystem superblock information
    #[arg(short, long)]
    stat: bool,

    /// Kind(type of image) to parse
    #[arg(short,
          long,
          default_value = "le_v4_0",
          value_parser = PossibleValuesParser::new(
          [
              "be_v4_0",
              "le_v4_0",
              "avm_be_v4_0",
              "le_v3_0",
          ]
    ))]
    kind: String,

    /// Emit shell completion scripts
    #[arg(long)]
    completions: Option<Shell>,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let mut args = Args::parse();

    if let Some(completions) = args.completions {
        let mut cmd = Args::command();
        let name = cmd.get_name().to_string();
        generate(completions, &mut cmd, name, &mut io::stdout());
        return ExitCode::SUCCESS;
    }

    let kind = Kind::from_target(&args.kind).unwrap();

    let mut file = BufReader::new(File::open(args.filesystem.as_ref().unwrap()).unwrap());

    if args.auto_offset {
        if let Some(found_offset) = find_offset(&mut file, &kind) {
            println!("found: {found_offset:02x?}");
            args.offset = found_offset;
        } else {
            println!("[!] magic not found");
            return ExitCode::FAILURE;
        }
    }

    if args.stat {
        stat(args, file, kind);
        return ExitCode::SUCCESS;
    }

    let squashfs = Squashfs::from_reader_with_offset_and_kind(file, args.offset, kind).unwrap();
    let root_process = unsafe { geteuid() == 0 };
    if root_process {
        umask(Mode::from_bits(0).unwrap());
    }

    let filesystem = squashfs.into_filesystem_reader().unwrap();

    // if we can find a parent, then a filter must be applied and the exact parent dirs must be
    // found above it
    let mut files: Vec<&Node<SquashfsFileReader>> = vec![];
    if args.path_filter.parent().is_some() {
        let mut current = PathBuf::new();
        current.push("/");
        for part in args.path_filter.iter() {
            current.push(part);
            if let Some(exact) = filesystem.files().find(|&a| a.fullpath == current) {
                files.push(exact);
            } else {
                panic!("Invalid --path-filter, path doesn't exist");
            }
        }
        // remove the final node, this is a file and will be caught in the following statement
        files.pop();
    }

    // gather all files and dirs
    let nodes = files.into_iter().chain(
        filesystem
            .files()
            .filter(|a| a.fullpath.starts_with(&args.path_filter)),
    );

    // extract or list
    if args.list {
        list(nodes);
    } else {
        extract_all(&args, &filesystem, root_process, nodes);
    }

    ExitCode::SUCCESS
}

fn list<'a>(nodes: impl std::iter::Iterator<Item = &'a Node<SquashfsFileReader>>) {
    for node in nodes {
        let path = &node.fullpath;
        println!("{}", path.display());
    }
}

fn stat(args: Args, mut file: BufReader<File>, kind: Kind) {
    file.seek(SeekFrom::Start(args.offset)).unwrap();
    let mut reader: Box<dyn BufReadSeek> = Box::new(file);
    let (superblock, compression_options) = SuperBlock::from_reader(&mut reader, &kind).unwrap();

    // show info about flags
    println!("{superblock:#08x?}");

    // show info about compression options
    println!("Compression Options: {compression_options:#x?}");

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
        // squashfs-tools/unsquashfs behavior
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

fn extract_all<'a>(
    args: &Args,
    filesystem: &'a FilesystemReader,
    root_process: bool,
    nodes: impl std::iter::Iterator<Item = &'a Node<SquashfsFileReader>>,
) {
    // TODO: fixup perms for this?
    let _ = fs::create_dir_all(&args.dest);

    // alloc required space for file data readers
    let (mut buf_read, mut buf_decompress) = filesystem.alloc_read_buffers();

    for node in nodes {
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
                let file = filesystem.file(&file.basic);
                let mut reader = file.reader(&mut buf_read, &mut buf_decompress);

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

                // These permissions are corrected later (user default permissions for now)

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
        }
    }

    // fixup dir permissions
    for node in filesystem
        .files()
        .filter(|a| a.fullpath.starts_with(&args.path_filter))
    {
        if let InnerNode::Dir(SquashfsDir { .. }) = &node.inner {
            let path = &node.fullpath;
            let path = path.strip_prefix(Component::RootDir).unwrap_or(path);
            let path = Path::new(&args.dest).join(path);
            set_attributes(&path, &node.header, root_process, false);
        }
    }
}
