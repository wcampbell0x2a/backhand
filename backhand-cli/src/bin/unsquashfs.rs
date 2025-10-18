use std::collections::HashSet;
use std::fs::{self, File, Permissions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::lchown;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

use backhand::kind::Kind;
use backhand::traits::filesystem::{BackhandInnerNode, BackhandNode, BackhandNodeHeader};
#[cfg(feature = "v3")]
use backhand::V3;
use backhand::{
    create_squashfs_from_kind, BufReadSeek, FilesystemReaderTrait, SquashfsVersion,
    DEFAULT_BLOCK_SIZE, V4,
};
use backhand_cli::after_help;
use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use console::Term;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use nix::fcntl::AT_FDCWD;
use nix::libc::geteuid;
use nix::sys::stat::{dev_t, mknod, mode_t, umask, utimensat, utimes, Mode, SFlag, UtimensatFlags};
use nix::sys::time::{TimeSpec, TimeVal};
use nix::unistd::mkfifo;
use rayon::prelude::*;
use std::time::{Duration, Instant};

// -musl malloc is slow, use jemalloc
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

const AVAILABLE_KINDS: &[&str] = &[
    "le_v4_0",
    "be_v4_0",
    #[cfg(feature = "v3")]
    "le_v3_0",
    #[cfg(feature = "v3")]
    "be_v3_0",
    #[cfg(feature = "v3_lzma")]
    "le_v3_0_lzma",
    #[cfg(feature = "v3_lzma")]
    "be_v3_0_lzma",
    #[cfg(feature = "v3_lzma")]
    "netgear_be_v3_0_lzma_standard",
    #[cfg(feature = "v3_lzma")]
    "netgear_be_v3_0_lzma",
    "avm_be_v4_0",
];

pub fn required_root(a: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(a);

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
          styles = clap_cargo::style::CLAP_STYLING,
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

    /// Kind(type of image) to parse. If not specified, will auto-detect by trying all kinds.
    #[arg(short, long, value_parser = PossibleValuesParser::new(AVAILABLE_KINDS))]
    kind: Option<String>,

    /// Emit shell completion scripts
    #[arg(long)]
    completions: Option<Shell>,

    /// Silence all progress bar and RUST_LOG output
    #[arg(long)]
    quiet: bool,
}

fn try_all_kinds(file: &mut BufReader<File>, offset: u64) -> Result<(Kind, &'static str), String> {
    let mut errors = Vec::new();

    for &kind_name in AVAILABLE_KINDS {
        if let Ok(kind) = Kind::from_target(kind_name) {
            file.seek(SeekFrom::Start(0)).map_err(|e| format!("Failed to seek: {}", e))?;

            let success = {
                let result = create_squashfs_from_kind(&mut *file, offset, kind.clone());
                match result {
                    Ok(_) => true,
                    Err(e) => {
                        errors.push(format!("{}: {}", kind_name, e));
                        false
                    }
                }
            };

            if success {
                file.seek(SeekFrom::Start(0)).map_err(|e| format!("Failed to seek back: {}", e))?;
                return Ok((kind, kind_name));
            }
        }
    }

    Err(format!("Could not detect kind. Tried all kinds, errors:\n{}", errors.join("\n")))
}

fn handle_with_kind<'a>(
    args: &mut Args,
    file: &'a mut BufReader<File>,
    kind_str: &str,
    pb: &ProgressBar,
    blue_bold: &console::Style,
    red_bold: &console::Style,
) -> Result<(Box<dyn FilesystemReaderTrait + 'a>, Kind), ExitCode> {
    let kind = Kind::from_target(kind_str).unwrap();

    if args.auto_offset {
        if !args.quiet {
            pb.enable_steady_tick(Duration::from_millis(120));
            let line = format!("{:>14}", blue_bold.apply_to("Searching for magic"));
            pb.set_message(line);
        }
        if let Some(found_offset) = find_offset(file, &kind) {
            if !args.quiet {
                let line =
                    format!("{:>14} 0x{:08x}", blue_bold.apply_to("Found magic"), found_offset,);
                pb.finish_with_message(line);
            }
            args.offset = found_offset;
        } else {
            if !args.quiet {
                let line = format!("{:>14}", red_bold.apply_to("Magic not found"),);
                pb.finish_with_message(line);
            }
            return Err(ExitCode::FAILURE);
        }
    }

    match create_squashfs_from_kind(file, args.offset, kind.clone()) {
        Ok(filesystem) => Ok((filesystem, kind)),
        Err(e) => {
            let line = format!("{:>14}", red_bold.apply_to(format!("Could not read image: {e}")));
            pb.finish_with_message(line);
            eprintln!("Debug error: {e:?}");
            Err(ExitCode::FAILURE)
        }
    }
}

fn handle_auto_detect_kind<'a>(
    args: &Args,
    file: &'a mut BufReader<File>,
    pb: &ProgressBar,
    blue_bold: &console::Style,
    red_bold: &console::Style,
) -> Result<(Box<dyn FilesystemReaderTrait + 'a>, Kind), ExitCode> {
    if !args.quiet {
        pb.enable_steady_tick(Duration::from_millis(120));
        let line = format!("{:>14}", blue_bold.apply_to("Auto-detecting kind"));
        pb.set_message(line);
    }

    if args.auto_offset {
        if !args.quiet {
            let line =
                format!("{:>14}", red_bold.apply_to("Cannot use --auto-offset without --kind"),);
            pb.finish_with_message(line);
        }
        eprintln!("Error: --auto-offset requires --kind to be specified");
        return Err(ExitCode::FAILURE);
    }

    match try_all_kinds(file, args.offset) {
        Ok((detected_kind, kind_name)) => {
            if !args.quiet {
                let line = format!("{:>14} {}", blue_bold.apply_to("Detected kind"), kind_name);
                pb.finish_and_clear();
                eprintln!("{}", line);
            }

            match create_squashfs_from_kind(file, args.offset, detected_kind.clone()) {
                Ok(filesystem) => Ok((filesystem, detected_kind)),
                Err(e) => {
                    let line =
                        format!("{:>14}", red_bold.apply_to(format!("Could not read image: {e}")));
                    pb.finish_with_message(line);
                    eprintln!("Debug error: {e:?}");
                    Err(ExitCode::FAILURE)
                }
            }
        }
        Err(e) => {
            let line = format!("{:>14}", red_bold.apply_to("Auto-detection failed"));
            pb.finish_with_message(line);
            eprintln!("{}", e);
            Err(ExitCode::FAILURE)
        }
    }
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

    let mut file = BufReader::with_capacity(
        DEFAULT_BLOCK_SIZE as usize,
        File::open(args.filesystem.as_ref().unwrap()).unwrap(),
    );

    let blue_bold: console::Style = console::Style::new().blue().bold();
    let red_bold: console::Style = console::Style::new().red().bold();
    let pb = ProgressBar::new_spinner();

    if args.stat {
        if let Some(kind_str) = &args.kind {
            let kind = Kind::from_target(kind_str).unwrap();
            stat(args, file, kind);
            return ExitCode::SUCCESS;
        } else {
            if !args.quiet {
                let line =
                    format!("{:>14}", red_bold.apply_to("Cannot use --stat without --kind"),);
                pb.finish_with_message(line);
            }
            eprintln!("Error: --stat requires --kind to be specified");
            return ExitCode::FAILURE;
        }
    }

    let (filesystem, _) = if let Some(kind_str) = args.kind.clone() {
        match handle_with_kind(&mut args, &mut file, &kind_str, &pb, &blue_bold, &red_bold) {
            Ok(result) => result,
            Err(exit_code) => return exit_code,
        }
    } else {
        match handle_auto_detect_kind(&args, &mut file, &pb, &blue_bold, &red_bold) {
            Ok(result) => result,
            Err(exit_code) => return exit_code,
        }
    };

    process_filesystem(filesystem.as_ref(), args, pb, red_bold, blue_bold)
}

fn process_filesystem(
    filesystem: &dyn FilesystemReaderTrait,
    args: Args,
    _pb: ProgressBar,
    red_bold: console::Style,
    blue_bold: console::Style,
) -> ExitCode {
    let root_process = unsafe { geteuid() == 0 };
    if root_process {
        umask(Mode::from_bits(0).unwrap());
    }

    // Start new spinner as we extract all the inode and other information from the image
    let start = Instant::now();
    let pb = ProgressBar::new_spinner();
    if !args.quiet {
        pb.enable_steady_tick(Duration::from_millis(120));
        let line = format!("{:>14}", blue_bold.apply_to("Reading image"));
        pb.set_message(line);
    }

    if !args.quiet {
        let line = format!("{:>14}", blue_bold.apply_to("Read image"));
        pb.finish_with_message(line);
    }

    // if we can find a parent, then a filter must be applied and the exact parent dirs must be
    // found above it
    let mut files: Vec<BackhandNode> = vec![];
    if args.path_filter.parent().is_some() {
        let mut current = PathBuf::new();
        current.push("/");
        for part in args.path_filter.iter() {
            current.push(part);
            if let Some(exact) = filesystem.files().into_iter().find(|a| a.fullpath == current) {
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
    let all_files: Vec<BackhandNode> = filesystem.files();
    let filtered_files: Vec<BackhandNode> =
        all_files.iter().filter(|a| a.fullpath.starts_with(&args.path_filter)).cloned().collect();
    let nodes = files.into_iter().chain(filtered_files.iter().cloned());

    // extract or list
    if args.list {
        list(nodes);
    } else {
        // This could be expensive, only pass this in when not quiet
        let n_nodes = if !args.quiet { Some(files_len + filtered_files.len()) } else { None };

        let all_nodes: Vec<BackhandNode> = nodes.collect();
        extract_all(&args, filesystem, root_process, all_nodes, n_nodes, start);
    }

    ExitCode::SUCCESS
}

fn list(nodes: impl Iterator<Item = BackhandNode>) {
    for node in nodes {
        let path = &node.fullpath;
        println!("{}", path.display());
    }
}

fn stat_v4(args: Args, mut file: BufReader<File>, kind: Kind) {
    file.seek(SeekFrom::Start(args.offset)).unwrap();
    let mut reader: Box<dyn BufReadSeek> = Box::new(file);
    let (superblock, compression_options) =
        V4::superblock_and_compression_options(&mut reader, &kind).unwrap();

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

    if superblock.data_has_been_deduplicated() {
        println!("flag: data has been deduplicated");
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

#[cfg(feature = "v3")]
fn stat_v3(args: Args, mut file: BufReader<File>, kind: Kind) {
    file.seek(SeekFrom::Start(args.offset)).unwrap();
    let mut reader: Box<dyn BufReadSeek> = Box::new(file);
    let (superblock, _compression_options) =
        V3::superblock_and_compression_options(&mut reader, &kind).unwrap();

    // show info about flags
    println!("{superblock:#08x?}");
}

fn stat(args: Args, file: BufReader<File>, kind: Kind) {
    match (kind.version_major(), kind.version_minor()) {
        (4, 0) => stat_v4(args, file, kind),
        #[cfg(feature = "v3")]
        (3, 0) => stat_v3(args, file, kind),
        _ => {
            eprintln!(
                "Unsupported SquashFS version: {}.{}",
                kind.version_major(),
                kind.version_minor()
            );
        }
    }
}

fn set_attributes(
    pb: &ProgressBar,
    args: &Args,
    path: &Path,
    header: &BackhandNodeHeader,
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
        match lchown(path, Some(header.uid), Some(header.gid)) {
            Ok(_) => (),
            Err(e) => {
                if !args.quiet {
                    let line =
                        format!("lchown {} {} {} : {e}", path.display(), header.uid, header.gid,);
                    failed(pb, &line);
                }
                return;
            }
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

fn extract_all(
    args: &Args,
    filesystem: &dyn FilesystemReaderTrait,
    root_process: bool,
    nodes: Vec<BackhandNode>,
    n_nodes: Option<usize>,
    start: Instant,
) {
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

    tracing::trace!("{:?}", nodes);
    nodes.into_par_iter().for_each(|node| {
        let path = &node.fullpath;
        let fullpath = path.strip_prefix(Component::RootDir).unwrap_or(path);
        if !args.quiet {
            let mut p = processing.lock().unwrap();
            p.insert(fullpath.to_path_buf());
            pb.set_message(p.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(", "));
            pb.inc(1);
        }

        let filepath = Path::new(&args.dest).join(fullpath);
        // create required dirs, we will fix permissions later
        let _ = fs::create_dir_all(filepath.parent().unwrap());

        match &node.inner {
            BackhandInnerNode::File(file) => {
                // alloc required space for file data readers
                // check if file exists
                if !args.force && filepath.exists() {
                    if !args.quiet {
                        exists(&pb, filepath.to_str().unwrap());
                        let mut p = processing.lock().unwrap();
                        p.remove(&fullpath.to_path_buf());
                    }
                    return;
                }

                // write to file
                let file_data = filesystem.file_data(file);
                let fd = File::create(&filepath).unwrap();
                let mut writer = BufWriter::with_capacity(file_data.len(), &fd);

                match writer.write_all(&file_data) {
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
                            let mut p = processing.lock().unwrap();
                            p.remove(&fullpath.to_path_buf());
                        }
                        return;
                    }
                }
                writer.flush().unwrap();
            }
            BackhandInnerNode::Symlink { link } => {
                // create symlink
                let link_display = link.display();
                // check if file exists
                if !args.force && filepath.exists() {
                    exists(&pb, filepath.to_str().unwrap());
                    let mut p = processing.lock().unwrap();
                    p.remove(&fullpath.to_path_buf());
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
                            let mut p = processing.lock().unwrap();
                            p.remove(&fullpath.to_path_buf());
                        }
                        return;
                    }
                }

                // set attributes, but special to not follow the symlink

                if root_process {
                    // TODO: Use (unix_chown) when not nightly: https://github.com/rust-lang/rust/issues/88989
                    match lchown(&filepath, Some(node.header.uid), Some(node.header.gid)) {
                        Ok(_) => (),
                        Err(e) => {
                            if !args.quiet {
                                let line = format!(
                                    "lchown {} {} {} : {e}",
                                    filepath.display(),
                                    node.header.uid,
                                    node.header.gid,
                                );
                                failed(&pb, &line);
                            }
                            let mut p = processing.lock().unwrap();
                            p.remove(&fullpath.to_path_buf());
                            return;
                        }
                    }
                }

                // TODO Use (file_set_times) when not nightly: https://github.com/rust-lang/rust/issues/98245
                // Make sure this doesn't follow symlinks when changed to std library!
                let timespec = TimeSpec::new(node.header.mtime as _, 0);
                utimensat(
                    AT_FDCWD,
                    &filepath,
                    &timespec,
                    &timespec,
                    UtimensatFlags::NoFollowSymlink,
                )
                .unwrap();
            }
            BackhandInnerNode::Dir => {
                // These permissions are corrected later (user default permissions for now)
                //
                // don't display error if this was already created, we might have already
                // created it in another thread to put down a file
                if std::fs::create_dir(&filepath).is_ok() && args.info && !args.quiet {
                    created(&pb, filepath.to_str().unwrap())
                }
            }
            BackhandInnerNode::CharacterDevice { device_number } => {
                if root_process {
                    #[allow(clippy::unnecessary_fallible_conversions)]
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
                                let mut p = processing.lock().unwrap();
                                p.remove(&fullpath.to_path_buf());
                            }
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
                    p.remove(&fullpath.to_path_buf());
                    return;
                }
            }
            BackhandInnerNode::BlockDevice { device_number } => {
                #[allow(clippy::unnecessary_fallible_conversions)]
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
                            let mut p = processing.lock().unwrap();
                            p.remove(&fullpath.to_path_buf());
                        }
                        return;
                    }
                }
            }
            BackhandInnerNode::NamedPipe => {
                match mkfifo(
                    &filepath,
                    Mode::from_bits(mode_t::from(node.header.permissions)).unwrap(),
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
                        p.remove(&fullpath.to_path_buf());
                        return;
                    }
                }
            }
            BackhandInnerNode::Socket => {
                #[allow(clippy::unnecessary_fallible_conversions)]
                match mknod(
                    &filepath,
                    SFlag::S_IFSOCK,
                    Mode::from_bits(mode_t::from(node.header.permissions)).unwrap(),
                    dev_t::try_from(0_u64).unwrap(),
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
                            let mut p = processing.lock().unwrap();
                            p.remove(&fullpath.to_path_buf());
                        }
                        return;
                    }
                }
            }
        }
        let mut p = processing.lock().unwrap();
        p.remove(&fullpath.to_path_buf());
    });

    // fixup dir permissions
    let all_filesystem_files: Vec<BackhandNode> = filesystem.files();
    for node in all_filesystem_files.iter().filter(|a| a.fullpath.starts_with(&args.path_filter)) {
        if let BackhandInnerNode::Dir = &node.inner {
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
