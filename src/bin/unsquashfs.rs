#[path = "../../common/common.rs"]
mod common;
use std::collections::HashSet;
use std::fs::{self, File, Permissions};
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::iter::Iterator;
use std::os::unix::prelude::{OsStrExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

use backhand::compressor::CompressionOptions;
use backhand::kind::Kind;
use backhand::{
    BufReadSeek, FilesystemReader, InnerNode, MultiFilesystemReader, MultiSquashfs,
    MultiSuperBlock, Node, NodeHeader, Squashfs, SquashfsBlockDevice, SquashfsCharacterDevice,
    SquashfsDir, SquashfsFileReader, SquashfsSymlink,
};
use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use common::after_help;
use console::Term;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use libc::lchown;
use nix::libc::geteuid;
use nix::sys::stat::{dev_t, mknod, mode_t, umask, utimensat, utimes, Mode, SFlag, UtimensatFlags};
use nix::sys::time::{TimeSpec, TimeVal};
use rayon::prelude::*;
use std::time::{Duration, Instant};

// -musl malloc is slow, use jemalloc
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

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

pub fn extracted(pb: &ProgressBar, s: &str) {
    let blue_bold: console::Style = console::Style::new().blue().bold();
    let line = format!("{:>16} {}", blue_bold.apply_to("Extracted"), s,);
    pb.println(line);
}

pub fn created(pb: &ProgressBar, s: &str) {
    let blue_bold: console::Style = console::Style::new().blue().bold();
    let line = format!("{:>16} {}", blue_bold.apply_to("Created"), s,);
    pb.println(line);
}

pub fn exists(pb: &ProgressBar, s: &str) {
    let red_bold: console::Style = console::Style::new().red().bold();
    let line = format!("{:>16} {}", red_bold.apply_to("Exists"), s,);
    pb.println(line);
}

pub fn failed(pb: &ProgressBar, s: &str) {
    let red_bold: console::Style = console::Style::new().red().bold();
    let line = format!("{:>16} {}", red_bold.apply_to("Failed"), s,);
    pb.println(line);
}

/// tool to uncompress, extract and list squashfs filesystems
#[derive(Parser)]
#[command(author,
          version,
          name = "unsquashfs-backhand",
          after_help = after_help(true),
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

    /// List filesystem, do not write to DEST (ignores --quiet)
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

    /// Display filesystem superblock information (ignores --quiet)
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
              "be_v3_0",
          ]
    ))]
    kind: String,

    /// Emit shell completion scripts
    #[arg(long)]
    completions: Option<Shell>,

    /// Silence all progress bar and RUST_LOG output
    #[arg(long)]
    quiet: bool,
}

fn main() -> ExitCode {
    let mut args = Args::parse();
    if !args.quiet {
        tracing_subscriber::fmt::init();
    }

    if let Some(completions) = args.completions {
        let mut cmd = Args::command();
        let name = cmd.get_name().to_string();
        generate(completions, &mut cmd, name, &mut io::stdout());
        return ExitCode::SUCCESS;
    }

    let kind = Kind::from_target(&args.kind).unwrap();

    let mut file = BufReader::new(File::open(args.filesystem.as_ref().unwrap()).unwrap());

    let blue_bold: console::Style = console::Style::new().blue().bold();
    let red_bold: console::Style = console::Style::new().blue().bold();
    let pb = ProgressBar::new_spinner();

    if args.auto_offset {
        if !args.quiet {
            pb.enable_steady_tick(Duration::from_millis(120));
            let line = format!("{:>14}", blue_bold.apply_to("Searching for magic"));
            pb.set_message(line);
        }
        if let Some(found_offset) = find_offset(&mut file, &kind) {
            if !args.quiet {
                let line = format!(
                    "{:>14} 0x{:08x}",
                    blue_bold.apply_to("Found magic"),
                    found_offset,
                );
                pb.finish_with_message(line);
            }
            args.offset = found_offset;
        } else {
            if !args.quiet {
                let line = format!("{:>14}", red_bold.apply_to("Magic not found"),);
                pb.finish_with_message(line);
            }
            return ExitCode::FAILURE;
        }
    }

    if args.stat {
        stat(args, file, kind);
        return ExitCode::SUCCESS;
    }

    let multi_squashfs =
        MultiSquashfs::from_reader_with_offset_and_kind(file, args.offset, kind).unwrap();
    // let MultiSquashfs::V4(multi_squashfs) = multi_squashfs else {
    //     panic!("ah!");
    // };
    let root_process = unsafe { geteuid() == 0 };
    if root_process {
        umask(Mode::from_bits(0).unwrap());
    }

    // Start new spinner as we extract all the inode and other information from the image
    // This can be very time consuming
    let start = Instant::now();
    let pb = ProgressBar::new_spinner();
    if !args.quiet {
        pb.enable_steady_tick(Duration::from_millis(120));
        let line = format!("{:>14}", blue_bold.apply_to("Reading image"));
        pb.set_message(line);
    }
    let filesystem = multi_squashfs.into_filesystem_reader().unwrap();
    if !args.quiet {
        let line = format!("{:>14}", blue_bold.apply_to("Read image"));
        pb.finish_with_message(line);
    }

    match filesystem {
        MultiFilesystemReader::V3(v3) => fs_v3(&pb, &args, v3, root_process, start),
        MultiFilesystemReader::V4(v4) => fs_v4(&pb, &args, v4, root_process, start),
    }
}

fn fs_v3(
    pb: &ProgressBar,
    args: &Args,
    filesystem: backhand::v3::FilesystemReader,
    root_process: bool,
    start: Instant,
) -> ExitCode {
    let red_bold: console::Style = console::Style::new().blue().bold();

    // if we can find a parent, then a filter must be applied and the exact parent dirs must be
    // found above it
    let mut files: Vec<&backhand::v3::Node<backhand::v3::SquashfsFileReader>> = vec![];
    if args.path_filter.parent().is_some() {
        let mut current = PathBuf::new();
        current.push("/");
        for part in args.path_filter.iter() {
            current.push(part);
            if let Some(exact) = filesystem.files().find(|&a| a.fullpath == current) {
                files.push(exact);
            } else {
                if !args.quiet {
                    let line = format!(
                        "{:>14}",
                        red_bold.apply_to("Invalid --path-filter, path doesn't exist")
                    );
                    pb.finish_with_message(line);
                }
                return ExitCode::FAILURE;
            }
        }
        // remove the final node, this is a file and will be caught in the following statement
        files.pop();
    }

    // gather all files and dirs
    let files_len = files.len();
    let nodes = files.into_iter().chain(
        filesystem
            .files()
            .filter(|a| a.fullpath.starts_with(&args.path_filter)),
    );

    // extract or list
    if args.list {
        //list(nodes);
    } else {
        // This could be expensive, only pass this in when not quiet
        let n_nodes = if !args.quiet {
            Some(
                files_len
                    + filesystem
                        .files()
                        .filter(|a| a.fullpath.starts_with(&args.path_filter))
                        .count(),
            )
        } else {
            None
        };

        extract_all_v3(
            &args,
            &filesystem,
            root_process,
            nodes
                .collect::<Vec<&backhand::v3::Node<backhand::v3::SquashfsFileReader>>>()
                .into_par_iter(),
            n_nodes,
            start,
        );
    }

    ExitCode::SUCCESS
}

fn fs_v4(
    pb: &ProgressBar,
    args: &Args,
    filesystem: backhand::v4::FilesystemReader,
    root_process: bool,
    start: Instant,
) -> ExitCode {
    let red_bold: console::Style = console::Style::new().blue().bold();

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
                if !args.quiet {
                    let line = format!(
                        "{:>14}",
                        red_bold.apply_to("Invalid --path-filter, path doesn't exist")
                    );
                    pb.finish_with_message(line);
                }
                return ExitCode::FAILURE;
            }
        }
        // remove the final node, this is a file and will be caught in the following statement
        files.pop();
    }

    // gather all files and dirs
    let files_len = files.len();
    let nodes = files.into_iter().chain(
        filesystem
            .files()
            .filter(|a| a.fullpath.starts_with(&args.path_filter)),
    );

    // extract or list
    if args.list {
        list(nodes);
    } else {
        // This could be expensive, only pass this in when not quiet
        let n_nodes = if !args.quiet {
            Some(
                files_len
                    + filesystem
                        .files()
                        .filter(|a| a.fullpath.starts_with(&args.path_filter))
                        .count(),
            )
        } else {
            None
        };

        extract_all(
            &args,
            &filesystem,
            root_process,
            nodes
                .collect::<Vec<&Node<SquashfsFileReader>>>()
                .into_par_iter(),
            n_nodes,
            start,
        );
    }

    ExitCode::SUCCESS
}

fn list<'a>(nodes: impl Iterator<Item = &'a Node<SquashfsFileReader>>) {
    for node in nodes {
        let path = &node.fullpath;
        println!("{}", path.display());
    }
}

fn stat(args: Args, mut file: BufReader<File>, kind: Kind) {
    file.seek(SeekFrom::Start(args.offset)).unwrap();
    let mut reader: Box<dyn BufReadSeek> = Box::new(file);
    let (superblock, compression_options) =
        MultiSuperBlock::superblock_and_compression_options(&mut reader, &kind).unwrap();

    match superblock {
        MultiSuperBlock::V3(v3) => stat_v3(v3),
        MultiSuperBlock::V4(v4) => stat_v4(v4, compression_options),
    }
}

fn stat_v4(superblock: backhand::v4::SuperBlock, co: Option<CompressionOptions>) {
    println!("{superblock:#08x?}");

    // show info about compression options
    println!("Compression Options: {co:#x?}");

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

    if superblock.duplicate_data_removed() {
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

fn stat_v3(superblock: backhand::v3::SuperBlock) {
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

    if superblock.duplicate_data_removed() {
        println!("flag: data duplicated was removed");
    }

    if superblock.nfs_export_table_exists() {
        println!("flag: nfs export table exists");
    }
}

fn set_attributes(
    pb: &ProgressBar,
    args: &Args,
    path: &Path,
    header: &NodeHeader,
    root_process: bool,
    is_file: bool,
) {
    // TODO Use (file_set_times) when not nightly: https://github.com/rust-lang/rust/issues/98245
    let timeval = TimeVal::new(header.mtime as _, 0);
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
            lchown(path_bytes as *const _, header.uid, header.gid);
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
    if let Err(e) = fs::set_permissions(path, Permissions::from_mode(mode)) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            // try without sticky bit
            if fs::set_permissions(path, Permissions::from_mode(mode & !1000)).is_err()
                && !args.quiet
            {
                let line = format!("{} : could not set permissions", path.to_str().unwrap());
                failed(pb, &line);
            }
        }
    }
}

fn v3_set_attributes(
    pb: &ProgressBar,
    args: &Args,
    path: &Path,
    header: &backhand::v3::NodeHeader,
    root_process: bool,
    is_file: bool,
) {
    // TODO Use (file_set_times) when not nightly: https://github.com/rust-lang/rust/issues/98245
    let timeval = TimeVal::new(header.mtime as _, 0);
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
            lchown(path_bytes as *const _, header.uid, header.gid);
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
    if let Err(e) = fs::set_permissions(path, Permissions::from_mode(mode)) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            // try without sticky bit
            if fs::set_permissions(path, Permissions::from_mode(mode & !1000)).is_err()
                && !args.quiet
            {
                let line = format!("{} : could not set permissions", path.to_str().unwrap());
                failed(pb, &line);
            }
        }
    }
}

fn extract_all<'a, S>(
    args: &Args,
    filesystem: &'a FilesystemReader,
    root_process: bool,
    nodes: S,
    n_nodes: Option<usize>,
    start: Instant,
) where
    S: ParallelIterator<Item = &'a Node<SquashfsFileReader>>,
{
    let pb = ProgressBar::new(n_nodes.unwrap_or(0) as u64);
    if !args.quiet {
        pb.set_style(ProgressStyle::default_spinner());
        pb.set_style(
            ProgressStyle::with_template(
                // note that bar size is fixed unlike cargo which is dynamic
                // and also the truncation in cargo uses trailers (`...`)
                if Term::stdout().size().1 > 80 {
                    "{prefix:>16.cyan.bold} [{bar:57}] {pos}/{len} {wide_msg}"
                } else {
                    "{prefix:>16.cyan.bold} [{bar:57}] {pos}/{len}"
                },
            )
            .unwrap()
            .progress_chars("=> "),
        );
        pb.set_prefix("Extracting");
        pb.inc(1);
    }

    let processing = Mutex::new(HashSet::new());

    nodes.for_each(|node| {
        let path = &node.fullpath;
        let fullpath = path.strip_prefix(Component::RootDir).unwrap_or(path);
        let mut p = processing.lock().unwrap();
        p.insert(fullpath);
        if !args.quiet {
            pb.set_message(
                p.iter()
                    .map(|a| a.to_path_buf().into_os_string().into_string().unwrap())
                    .collect::<Vec<String>>()
                    .join(", "),
            );
            pb.inc(1);
        }
        drop(p);

        let filepath = Path::new(&args.dest).join(fullpath);
        // create required dirs, we will fix permissions later
        let _ = fs::create_dir_all(filepath.parent().unwrap());

        match &node.inner {
            InnerNode::File(file) => {
                // alloc required space for file data readers
                let (mut buf_read, mut buf_decompress) = filesystem.alloc_read_buffers();

                // check if file exists
                if !args.force && filepath.exists() {
                    if !args.quiet {
                        exists(&pb, filepath.to_str().unwrap());
                    }
                    let mut p = processing.lock().unwrap();
                    p.remove(fullpath);
                    drop(p);
                    return;
                }

                // write to file
                let mut fd = File::create(&filepath).unwrap();
                let file = filesystem.file(&file.basic);
                let mut reader = file.reader(&mut buf_read, &mut buf_decompress);

                match io::copy(&mut reader, &mut fd) {
                    Ok(_) => {
                        if args.info && !args.quiet {
                            extracted(&pb, filepath.to_str().unwrap());
                        }
                        set_attributes(&pb, args, &filepath, &node.header, root_process, true);
                    }
                    Err(e) => {
                        if !args.quiet {
                            let line = format!("{} : {e}", filepath.to_str().unwrap());
                            failed(&pb, &line);
                        }
                        let mut p = processing.lock().unwrap();
                        p.remove(fullpath);
                        drop(p);
                        return;
                    }
                }
            }
            InnerNode::Symlink(SquashfsSymlink { link }) => {
                // create symlink
                let link_display = link.display();
                // check if file exists
                if !args.force && filepath.exists() {
                    exists(&pb, filepath.to_str().unwrap());
                    let mut p = processing.lock().unwrap();
                    p.remove(fullpath);
                    drop(p);
                    return;
                }

                match std::os::unix::fs::symlink(link, &filepath) {
                    Ok(_) => {
                        if args.info && !args.quiet {
                            let line = format!("{}->{link_display}", filepath.to_str().unwrap());
                            created(&pb, &line);
                        }
                    }
                    Err(e) => {
                        if !args.quiet {
                            let line =
                                format!("{}->{link_display} : {e}", filepath.to_str().unwrap());
                            failed(&pb, &line);
                        }
                        let mut p = processing.lock().unwrap();
                        p.remove(fullpath);
                        drop(p);
                        return;
                    }
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
                        lchown(path_bytes as *const _, node.header.uid, node.header.gid);
                    }
                }

                // TODO Use (file_set_times) when not nightly: https://github.com/rust-lang/rust/issues/98245
                // Make sure this doesn't follow symlinks when changed to std library!
                let timespec = TimeSpec::new(node.header.mtime as _, 0);
                utimensat(
                    None,
                    &filepath,
                    &timespec,
                    &timespec,
                    UtimensatFlags::NoFollowSymlink,
                )
                .unwrap();
            }
            InnerNode::Dir(SquashfsDir { .. }) => {
                // These permissions are corrected later (user default permissions for now)
                //
                // don't display error if this was already created, we might have already
                // created it in another thread to put down a file
                if std::fs::create_dir(&filepath).is_ok() && args.info && !args.quiet {
                    created(&pb, filepath.to_str().unwrap())
                }
            }
            InnerNode::CharacterDevice(SquashfsCharacterDevice { device_number }) => {
                if root_process {
                    match mknod(
                        &filepath,
                        SFlag::S_IFCHR,
                        Mode::from_bits(mode_t::from(node.header.permissions)).unwrap(),
                        dev_t::try_from(*device_number).unwrap(),
                    ) {
                        Ok(_) => {
                            if args.info && !args.quiet {
                                created(&pb, filepath.to_str().unwrap());
                            }

                            set_attributes(&pb, args, &filepath, &node.header, root_process, true);
                        }
                        Err(_) => {
                            if !args.quiet {
                                let line = format!(
                                    "char device {}, are you superuser?",
                                    filepath.to_str().unwrap()
                                );
                                failed(&pb, &line);
                            }
                            let mut p = processing.lock().unwrap();
                            p.remove(fullpath);
                            drop(p);
                            return;
                        }
                    }
                } else {
                    if !args.quiet {
                        let line = format!(
                            "char device {}, are you superuser?",
                            filepath.to_str().unwrap()
                        );
                        failed(&pb, &line);
                    }
                    let mut p = processing.lock().unwrap();
                    p.remove(fullpath);
                    drop(p);
                    return;
                }
            }
            InnerNode::BlockDevice(SquashfsBlockDevice { device_number }) => {
                match mknod(
                    &filepath,
                    SFlag::S_IFBLK,
                    Mode::from_bits(mode_t::from(node.header.permissions)).unwrap(),
                    dev_t::try_from(*device_number).unwrap(),
                ) {
                    Ok(_) => {
                        if args.info && !args.quiet {
                            created(&pb, filepath.to_str().unwrap());
                        }

                        set_attributes(&pb, args, &filepath, &node.header, root_process, true);
                    }
                    Err(_) => {
                        if args.info && !args.quiet {
                            created(&pb, filepath.to_str().unwrap());
                        }
                        let mut p = processing.lock().unwrap();
                        p.remove(fullpath);
                        drop(p);
                        return;
                    }
                }
            }
        }
        let mut p = processing.lock().unwrap();
        p.remove(fullpath);
        drop(p);
    });

    // fixup dir permissions
    for node in filesystem
        .files()
        .filter(|a| a.fullpath.starts_with(&args.path_filter))
    {
        if let InnerNode::Dir(SquashfsDir { .. }) = &node.inner {
            let path = &node.fullpath;
            let path = path.strip_prefix(Component::RootDir).unwrap_or(path);
            let path = Path::new(&args.dest).join(path);
            set_attributes(&pb, args, &path, &node.header, root_process, false);
        }
    }

    pb.finish_and_clear();

    // extraction is finished
    let green_bold: console::Style = console::Style::new().green().bold();
    if !args.quiet {
        println!(
            "{:>16} extraction of {} nodes in {}",
            green_bold.apply_to("Finished"),
            n_nodes.unwrap(),
            HumanDuration(start.elapsed())
        );
    }
}

fn extract_all_v3<'a, S>(
    args: &Args,
    filesystem: &'a backhand::v3::FilesystemReader,
    root_process: bool,
    nodes: S,
    n_nodes: Option<usize>,
    start: Instant,
) where
    S: ParallelIterator<Item = &'a backhand::v3::Node<backhand::v3::SquashfsFileReader>>,
{
    let pb = ProgressBar::new(n_nodes.unwrap_or(0) as u64);
    if !args.quiet {
        pb.set_style(ProgressStyle::default_spinner());
        pb.set_style(
            ProgressStyle::with_template(
                // note that bar size is fixed unlike cargo which is dynamic
                // and also the truncation in cargo uses trailers (`...`)
                if Term::stdout().size().1 > 80 {
                    "{prefix:>16.cyan.bold} [{bar:57}] {pos}/{len} {wide_msg}"
                } else {
                    "{prefix:>16.cyan.bold} [{bar:57}] {pos}/{len}"
                },
            )
            .unwrap()
            .progress_chars("=> "),
        );
        pb.set_prefix("Extracting");
        pb.inc(1);
    }

    let processing = Mutex::new(HashSet::new());

    nodes.for_each(|node| {
        let path = &node.fullpath;
        let fullpath = path.strip_prefix(Component::RootDir).unwrap_or(path);
        let mut p = processing.lock().unwrap();
        p.insert(fullpath.clone());
        if !args.quiet {
            pb.set_message(
                p.iter()
                    .map(|a| a.to_path_buf().into_os_string().into_string().unwrap())
                    .collect::<Vec<String>>()
                    .join(", "),
            );
            pb.inc(1);
        }
        drop(p);

        let filepath = Path::new(&args.dest).join(fullpath);
        // create required dirs, we will fix permissions later
        let _ = fs::create_dir_all(filepath.parent().unwrap());

        match &node.inner {
            backhand::v3::InnerNode::File(file) => {
                // alloc required space for file data readers
                let (mut buf_read, mut buf_decompress) = filesystem.alloc_read_buffers();

                // check if file exists
                if !args.force && filepath.exists() {
                    if !args.quiet {
                        exists(&pb, filepath.to_str().unwrap());
                    }
                    let mut p = processing.lock().unwrap();
                    p.remove(fullpath);
                    drop(p);
                    return;
                }

                // write to file
                let mut fd = File::create(&filepath).unwrap();
                let file = filesystem.file(&file.basic);
                let mut reader = file.reader(&mut buf_read, &mut buf_decompress);

                match io::copy(&mut reader, &mut fd) {
                    Ok(_) => {
                        if args.info && !args.quiet {
                            extracted(&pb, filepath.to_str().unwrap());
                        }
                        v3_set_attributes(&pb, args, &filepath, &node.header, root_process, true);
                    }
                    Err(e) => {
                        if !args.quiet {
                            let line = format!("{} : {e}", filepath.to_str().unwrap());
                            failed(&pb, &line);
                        }
                        let mut p = processing.lock().unwrap();
                        p.remove(fullpath);
                        drop(p);
                        return;
                    }
                }
            }
            backhand::v3::InnerNode::Symlink(backhand::v3::SquashfsSymlink { link }) => {
                // create symlink
                let link_display = link.display();
                // check if file exists
                if !args.force && filepath.exists() {
                    exists(&pb, filepath.to_str().unwrap());
                    let mut p = processing.lock().unwrap();
                    p.remove(fullpath);
                    drop(p);
                    return;
                }

                match std::os::unix::fs::symlink(link, &filepath) {
                    Ok(_) => {
                        if args.info && !args.quiet {
                            let line = format!("{}->{link_display}", filepath.to_str().unwrap());
                            created(&pb, &line);
                        }
                    }
                    Err(e) => {
                        if !args.quiet {
                            let line =
                                format!("{}->{link_display} : {e}", filepath.to_str().unwrap());
                            failed(&pb, &line);
                        }
                        let mut p = processing.lock().unwrap();
                        p.remove(fullpath);
                        drop(p);
                        return;
                    }
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
                        lchown(path_bytes as *const _, node.header.uid, node.header.gid);
                    }
                }

                // TODO Use (file_set_times) when not nightly: https://github.com/rust-lang/rust/issues/98245
                // Make sure this doesn't follow symlinks when changed to std library!
                let timespec = TimeSpec::new(node.header.mtime as _, 0);
                utimensat(
                    None,
                    &filepath,
                    &timespec,
                    &timespec,
                    UtimensatFlags::NoFollowSymlink,
                )
                .unwrap();
            }
            backhand::v3::InnerNode::Dir(backhand::v3::SquashfsDir { .. }) => {
                // These permissions are corrected later (user default permissions for now)
                //
                // don't display error if this was already created, we might have already
                // created it in another thread to put down a file
                if std::fs::create_dir(&filepath).is_ok() && args.info && !args.quiet {
                    created(&pb, filepath.to_str().unwrap())
                }
            }
            backhand::v3::InnerNode::CharacterDevice(backhand::v3::SquashfsCharacterDevice {
                device_number,
            }) => {
                if root_process {
                    match mknod(
                        &filepath,
                        SFlag::S_IFCHR,
                        Mode::from_bits(mode_t::from(node.header.permissions)).unwrap(),
                        dev_t::try_from(*device_number).unwrap(),
                    ) {
                        Ok(_) => {
                            if args.info && !args.quiet {
                                created(&pb, filepath.to_str().unwrap());
                            }

                            v3_set_attributes(
                                &pb,
                                args,
                                &filepath,
                                &node.header,
                                root_process,
                                true,
                            );
                        }
                        Err(_) => {
                            if !args.quiet {
                                let line = format!(
                                    "char device {}, are you superuser?",
                                    filepath.to_str().unwrap()
                                );
                                failed(&pb, &line);
                            }
                            let mut p = processing.lock().unwrap();
                            p.remove(fullpath);
                            drop(p);
                            return;
                        }
                    }
                } else {
                    if !args.quiet {
                        let line = format!(
                            "char device {}, are you superuser?",
                            filepath.to_str().unwrap()
                        );
                        failed(&pb, &line);
                    }
                    let mut p = processing.lock().unwrap();
                    p.remove(fullpath);
                    drop(p);
                    return;
                }
            }
            backhand::v3::InnerNode::BlockDevice(backhand::v3::SquashfsBlockDevice {
                device_number,
            }) => {
                match mknod(
                    &filepath,
                    SFlag::S_IFBLK,
                    Mode::from_bits(mode_t::from(node.header.permissions)).unwrap(),
                    dev_t::try_from(*device_number).unwrap(),
                ) {
                    Ok(_) => {
                        if args.info && !args.quiet {
                            created(&pb, filepath.to_str().unwrap());
                        }

                        v3_set_attributes(&pb, args, &filepath, &node.header, root_process, true);
                    }
                    Err(_) => {
                        if args.info && !args.quiet {
                            created(&pb, filepath.to_str().unwrap());
                        }
                        let mut p = processing.lock().unwrap();
                        p.remove(fullpath);
                        drop(p);
                        return;
                    }
                }
            }
        }
        let mut p = processing.lock().unwrap();
        p.remove(fullpath);
        drop(p);
    });

    // fixup dir permissions
    for node in filesystem
        .files()
        .filter(|a| a.fullpath.starts_with(&args.path_filter))
    {
        if let backhand::v3::InnerNode::Dir(backhand::v3::SquashfsDir { .. }) = &node.inner {
            let path = &node.fullpath;
            let path = path.strip_prefix(Component::RootDir).unwrap_or(path);
            let path = Path::new(&args.dest).join(path);
            v3_set_attributes(&pb, args, &path, &node.header, root_process, false);
        }
    }

    pb.finish_and_clear();

    // extraction is finished
    let green_bold: console::Style = console::Style::new().green().bold();
    if !args.quiet {
        println!(
            "{:>16} extraction of {} nodes in {}",
            green_bold.apply_to("Finished"),
            n_nodes.unwrap(),
            HumanDuration(start.elapsed())
        );
    }
}
