use std::{
    fs::{read_link, File, Metadata, OpenOptions},
    io::{self, BufReader},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use backhand::{
    compression::{CompressionOptions, Compressor, Gzip, Lz4, Lzo, Xz, XzFilter, Zstd},
    BackhandError, FilesystemCompressor, FilesystemReader, FilesystemWriter, NodeHeader,
};
use backhand_cli::{
    after_help_common,
    parse::{parse_window_size, parse_xz_dict_size, parse_xz_filter},
};
use clap::{error::ErrorKind, CommandFactory, Parser};
use clap_complete::{generate, Shell};
use console::Term;
use dateparser::DateTimeUtc;
use indicatif::{ProgressBar, ProgressStyle};

use backhand_cli::parse::{parse_block_size, parse_compressor, parse_gid, parse_octal, parse_uid};
use pathdiff::diff_paths;

/// Command-line tool to create squashfs filesystems from a list of input files.
#[derive(Parser)]
#[command(author,
          version,
          name = "mksquashfs-backhand",
          after_help = after_help_common(true),
          max_term_width = 98,
          styles = clap_cargo::style::CLAP_STYLING,
)]
struct Args {
    #[arg(required_unless_present = "completions")]
    filesystem: Option<PathBuf>,
    sources: Vec<PathBuf>,
    /// Emit shell completion scripts
    #[arg(long)]
    completions: Option<Shell>,

    /// Silence all progress bar and RUST_LOG output
    #[arg(long)]
    quiet: bool,

    /// Size of compressed data blocks. Supports an optional K or M suffix
    #[arg(short, default_value = "128K", value_parser = parse_block_size)]
    block_size: u32,

    /// Compression type used to build squashfs. Compressors available are gzip, lzo, lz4, xz and zstd
    #[arg(long = "comp", default_value = "xz", value_parser = parse_compressor)]
    comp: Compressor,

    /// Filesystem creation timestamp.
    /// Can be a Unix timestamp or any time format supported by the dateparser crate
    #[arg(long = "mkfs-time")]
    mkfs_time: Option<DateTimeUtc>,

    /// Do not check for duplicate files
    #[arg(long = "no-duplicates")]
    no_duplicates: bool,

    /// Pad the filesystem to a multiple of [PADDING] KiB
    #[arg(long = "padding", default_value_t = 4)]
    padding: u32,

    /// Make all files in the squashfs owned by the root user
    #[arg(long = "all-root")]
    all_root: bool,

    /// Set the root directory permissions to the given octal mode
    #[arg(long = "root-dir-mode", value_parser = parse_octal)]
    root_dir_mode: Option<u16>,

    /// Set the root directory uid. Can be an integer uid, or a username when run on a Unix host
    #[arg(long = "root-dir-uid", value_parser = parse_uid)]
    root_dir_uid: Option<u32>,

    /// Set the root directory gid. Can be an integer gid, or a group name when run on a Unix host
    #[arg(long = "root-dir-gid", value_parser = parse_gid)]
    root_dir_gid: Option<u32>,

    /// Maximum depth to which source paths will be searched for files. By default, search depth will not be limited
    #[arg(long = "max-depth")]
    max_depth: Option<usize>,

    /// Set the uids of all entries to this value. Can be an integer uid, or a username when run on a Unix host
    #[arg(long = "force-uid", value_parser = parse_uid)]
    force_uid: Option<u32>,

    /// Set the gids of all entries to this value. Can be an integer gid, or a group name when run on a Unix host
    #[arg(long = "force-gid", value_parser = parse_gid)]
    force_gid: Option<u32>,

    /// Append to the existing squashfs at [FILESYSTEM] instead of creating a new one
    #[arg(long = "append")]
    append: bool,

    /// Compression level to use when building the squashfs. Range varies by compressor:
    ///
    /// gzip: [1, 9]
    ///
    /// lzo: [1, 9]
    ///
    /// zstd: [1, 22]
    #[arg(long = "Xcompression-level")]
    compression_level: Option<u32>,

    /// Window size to use for gzip compression. Must be in the range [1, 15]
    #[arg(long = "Xgzip-window-size", default_value_t = 15, requires_if("gzip", "comp"), value_parser = parse_window_size)]
    gzip_window_size: u16,

    /// Branch/call/jump filters to try for xz compression before taking the best one
    #[arg(long = "Xxz-bcj", value_parser = parse_xz_filter, requires_if("xz", "comp"))]
    xz_bcj: Option<XzFilter>,

    /// Dictionary size to use for xz compression, which defaults to the same as the block size.
    ///
    /// Dictionary size can be a raw value or have an optional K/M suffix.
    /// It must be in the range [8K, block size] and be either a power of 2 or a power of 2 multiplied by 3
    #[arg(long = "Xxz-dict-size", value_parser = parse_xz_dict_size, requires_if("xz", "comp"))]
    xz_dict_size: Option<u32>,

    /// When set, source paths will be stored in their entirety, rather than only keeping the relative portion
    #[arg(long = "no-strip")]
    no_strip: bool,
}

/// Print the given message in red, bold text and exit the program
macro_rules! err_fatal {
    ($input:expr, $pb:expr) => {{
        let line = format!("{:>14}", ::backhand_cli::RED_BOLD.apply_to($input));
        $pb.finish_with_message(line);
        $pb.tick();
        return ::std::process::ExitCode::FAILURE;
    }};
}

fn main() -> ExitCode {
    let args = Args::parse();
    // Create progress bar and set a style similar to that of squashfs-tools' mksquashfs
    let pb = ProgressBar::new(0);
    if !args.quiet {
        tracing_subscriber::fmt::init();
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
    }

    // Generate shell completions
    if let Some(completions) = args.completions {
        let mut cmd = Args::command();
        let name = cmd.get_name().to_string();
        generate(completions, &mut cmd, name, &mut io::stdout());
        return ExitCode::SUCCESS;
    }

    // If appending to an existing filesystem, open a reader
    let reader = if args.append {
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .open(args.filesystem.as_ref().unwrap())
            .map_err(BackhandError::StdIo);
        let filesystem_reader =
            file.and_then(|file| FilesystemReader::from_reader(BufReader::new(file)));
        match filesystem_reader {
            Ok(reader) => Some(reader),
            Err(e) => err_fatal!(format!("Could not read source filesystem: {e:?}"), pb),
        }
    } else {
        None
    };

    // Either open the reader to which we are appending or create a new writer
    let mut writer = match reader.as_ref().map(FilesystemWriter::from_fs_reader) {
        Some(fs_writer) => match fs_writer {
            Ok(writer) => writer,
            Err(e) => {
                err_fatal!(format!("Could not create writer from existing filesystem: {e:?}"), pb)
            }
        },
        None => Default::default(),
    };

    // Validate compression level now, since the logic is more complex than what clap derives can handle
    match (&args.compression_level, &args.comp) {
        (None, _) => {}
        (Some(compression_level), Compressor::Gzip) => {
            if *compression_level < 1 || *compression_level > 9 {
                let mut cmd = Args::command();
                cmd.error(
                    ErrorKind::InvalidValue,
                    "Compression level for gzip must be in the range [1, 9].",
                )
                .exit();
            }
        }
        (Some(compression_level), Compressor::Lzo) => {
            if *compression_level < 1 || *compression_level > 9 {
                let mut cmd = Args::command();
                cmd.error(
                    ErrorKind::InvalidValue,
                    "Compression level for lzo must be in the range [1, 9].",
                )
                .exit();
            }
        }
        (Some(compression_level), Compressor::Zstd) => {
            if *compression_level < 1 || *compression_level > 22 {
                let mut cmd = Args::command();
                cmd.error(
                    ErrorKind::InvalidValue,
                    "Compression level for zstd must be in the range [1, 22].",
                )
                .exit();
            }
        }
        (Some(_), comp) => {
            let mut cmd = Args::command();
            cmd.error(
                ErrorKind::InvalidValue,
                format!("A compression level cannot be specified for compressor {comp:?}"),
            )
            .exit();
        }
    }

    // Everything here set to a zero is unimplemented in the backhand library
    let compression_options = match args.comp {
        Compressor::None => unreachable!(),
        Compressor::Gzip => {
            let gzip_options = Gzip {
                window_size: args.gzip_window_size,
                strategies: 0,
                compression_level: args.compression_level.unwrap_or(9),
            };
            CompressionOptions::Gzip(gzip_options)
        }
        Compressor::Lzma => unreachable!(),
        Compressor::Lzo => CompressionOptions::Lzo(Lzo { algorithm: 0, compression_level: 0 }),
        Compressor::Xz => {
            let xz_options = Xz {
                filters: args.xz_bcj.unwrap_or_default(),
                dictionary_size: args.xz_dict_size.unwrap_or(args.block_size),
                bit_opts: None,
                fb: None,
            };
            CompressionOptions::Xz(xz_options)
        }
        Compressor::Lz4 => CompressionOptions::Lz4(Lz4 { version: 0, flags: 0 }),
        Compressor::Zstd => {
            let zstd_options = Zstd { compression_level: args.compression_level.unwrap_or(15) };
            CompressionOptions::Zstd(zstd_options)
        }
    };
    // Apply command-line options to the filesystem writer
    writer.set_compressor(FilesystemCompressor::new(args.comp, Some(compression_options)).unwrap());
    writer.set_block_size(args.block_size);
    if let Some(ref mkfs_time) = args.mkfs_time {
        writer.set_time(mkfs_time.0.timestamp() as u32);
    }
    writer.set_no_duplicate_files(args.no_duplicates);
    writer.set_kib_padding(args.padding);
    if args.all_root {
        writer.set_only_root_id();
    }
    if let Some(mode) = args.root_dir_mode {
        writer.set_root_mode(mode);
    }
    if let Some(uid) = args.root_dir_uid {
        writer.set_root_uid(uid);
    } else {
        writer.set_root_mode(0o555);
    }
    if let Some(gid) = args.root_dir_gid {
        writer.set_root_gid(gid);
    }

    // Add files one at a time to the squashfs
    let file_count = match push_entries(&mut writer, &args) {
        Ok(count) => count,
        Err(e) => err_fatal!(format!("Could not add entries to squashfs: {e}"), pb),
    };
    if !args.quiet {
        pb.set_length(file_count);
        pb.enable_steady_tick(Duration::from_millis(120));
    }
    let mut output = File::create(args.filesystem.unwrap()).unwrap();
    writer.write_callback(&mut output, Some(|| pb.inc(1))).unwrap();

    pb.finish_with_message("Done!");
    ExitCode::SUCCESS
}

/// Add entries to a squashfs writer based on a command-line configuration.
#[tracing::instrument(skip(args))]
fn push_entries(writer: &mut FilesystemWriter, args: &Args) -> Result<u64, BackhandError> {
    let mut file_count = 0;
    for source in &args.sources {
        // Recursively walk through the source path and add all children
        let entries = ignore::WalkBuilder::new(source)
            .standard_filters(false)
            .max_depth(args.max_depth)
            .build();

        for entry in entries {
            let dir_entry = entry.map_err(|_| BackhandError::FileNotFound)?;
            let meta = dir_entry.metadata().map_err(|_| BackhandError::FileNotFound)?;
            // Convert paths to relative paths unless no_strip is specified
            let pushed_path = if args.no_strip {
                dir_entry.path().to_path_buf()
            } else if source != dir_entry.path() {
                Path::new("/").join(diff_paths(dir_entry.path(), source).unwrap())
            } else if dir_entry.metadata().as_ref().is_ok_and(Metadata::is_file) {
                Path::new("/").join(dir_entry.file_name())
            } else {
                continue;
            };

            #[cfg(target_family = "unix")]
            let node = {
                use std::os::unix::fs::MetadataExt;
                let mode = (meta.mode() & 0xfff) as u16;
                let uid = args.force_uid.unwrap_or(meta.uid());
                let gid = args.force_gid.unwrap_or(meta.gid());
                let mtime = meta.mtime() as u32;
                NodeHeader::new(mode, uid, gid, mtime)
            };
            #[cfg(not(target_family = "unix"))]
            let node = NodeHeader::default();

            // Add a new inode to the squashfs based on the file type of this entry
            let ftype = meta.file_type();
            if ftype.is_file() {
                let file = File::open(dir_entry.path()).map_err(BackhandError::StdIo)?;
                writer.push_file(file, &pushed_path, node)?;
                file_count += 1;
            } else if ftype.is_dir() {
                writer.push_dir_all(&pushed_path, node)?;
            } else if ftype.is_symlink() {
                writer.push_symlink(
                    read_link(dir_entry.path()).map_err(BackhandError::StdIo)?,
                    &pushed_path,
                    node,
                )?;
            }
            #[cfg(target_family = "unix")]
            {
                use libc::stat;
                use std::os::unix::fs::FileTypeExt;
                use std::{
                    alloc::{alloc, Layout},
                    ffi::CString,
                };

                if ftype.is_block_device() {
                    // Retrieve device number
                    let statbuf = unsafe { alloc(Layout::new::<stat>()) } as *mut stat;
                    unsafe {
                        stat(
                            CString::new(dir_entry.path().as_os_str().as_encoded_bytes())
                                .unwrap()
                                .as_ptr(),
                            statbuf,
                        )
                    };
                    let device_id = unsafe { (*statbuf).st_rdev } as u32;
                    writer.push_block_device(device_id, &pushed_path, node)?;
                } else if ftype.is_char_device() {
                    // Retrieve device number
                    let statbuf = unsafe { alloc(Layout::new::<stat>()) } as *mut stat;
                    unsafe {
                        stat(
                            CString::new(dir_entry.path().as_os_str().as_encoded_bytes())
                                .unwrap()
                                .as_ptr(),
                            statbuf,
                        )
                    };
                    let device_id = unsafe { (*statbuf).st_rdev } as u32;
                    writer.push_char_device(device_id, &pushed_path, node)?;
                } else if ftype.is_fifo() {
                    writer.push_fifo(&pushed_path, node)?;
                } else if ftype.is_socket() {
                    writer.push_socket(&pushed_path, node)?;
                }
            }
        }
    }
    Ok(file_count)
}
