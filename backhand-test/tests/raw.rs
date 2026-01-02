mod common;

use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::process::Command;

use backhand::compression::Compressor;
use backhand::{
    CompressionExtra, DEFAULT_BLOCK_SIZE, ExtraXz, FilesystemCompressor, FilesystemWriter,
    NodeHeader, SuperBlock, kind,
};
#[allow(unused_imports)]
use common::{test_bin_unsquashfs, test_squashfs_tools_unsquashfs};

#[test]
#[cfg(all(feature = "xz", feature = "gzip"))]
fn test_raw_00() {
    use std::io::BufReader;

    use backhand::{FilesystemReader, kind::Kind};

    common::download_asset("raw_00_control");
    let control_path = "test-assets/test_raw_00/control.squashfs";
    let dir = std::path::Path::new(&control_path).parent().unwrap();
    let new_path = dir.join("bytes.squashfs");
    let new_path = new_path.to_str().unwrap();

    let header = NodeHeader { permissions: 0o755, uid: 1000, gid: 1000, mtime: 0 };

    let o_header = NodeHeader { permissions: 0o766, ..header };

    // test out max xz level
    let mut xz_extra = ExtraXz::default();
    xz_extra.level(9).unwrap();
    let extra = CompressionExtra::Xz(xz_extra);

    let mut compressor = FilesystemCompressor::new(Compressor::Xz, None).unwrap();
    compressor.extra(extra).unwrap();

    let time = 0x634f_5237;

    // (some of these are already set with default(), but just testing...)
    let mut fs: FilesystemWriter = FilesystemWriter::default();
    fs.set_time(time);
    fs.set_block_size(DEFAULT_BLOCK_SIZE);
    fs.set_root_mode(0o777);
    fs.set_root_uid(1000);
    fs.set_root_gid(1000);
    fs.set_compressor(compressor);
    fs.set_kind(Kind::from_const(kind::LE_V4_0).unwrap());
    fs.set_kib_padding(8);

    //don't do anything if the directory exists
    fs.push_dir_all("usr/bin", o_header).unwrap();
    fs.push_file(Cursor::new(vec![0x00, 0x01]), "usr/bin/heyo", header).unwrap();
    fs.push_dir_all("this/is/a", o_header).unwrap();
    fs.push_file(Cursor::new(vec![0x0f; 0xff]), "this/is/a/file", header).unwrap();

    // create the modified squashfs
    let mut output = BufWriter::new(File::create(&new_path).unwrap());
    let (superblock, bytes_written) = fs.write(&mut output).unwrap();

    // 8KiB
    assert_eq!(bytes_written, 0x2000);
    assert_eq!(
        superblock,
        SuperBlock {
            magic: [0x68, 0x73, 0x71, 0x73],
            inode_count: 0x8,
            mod_time: time,
            block_size: 0x20000,
            frag_count: 0x1,
            compressor: Compressor::Xz,
            block_log: 0x11,
            flags: backhand::Flags::DataHasBeenDeduplicated as u16,
            id_count: 0x2,
            version_major: 0x4,
            version_minor: 0x0,
            root_inode: 0xe0,
            bytes_used: 0x1f4,
            id_table: 0x1ec,
            xattr_table: 0xffffffffffffffff,
            inode_table: 0xac,
            dir_table: 0x13a,
            frag_table: 0x1da,
            export_table: 0xffffffffffffffff,
        }
    );

    // compare
    #[cfg(feature = "__test_unsquashfs")]
    {
        let output = Command::new("unsquashfs").args(["-lln", "-UTC", &new_path]).output().unwrap();
        let expected = r#"drwxrwxrwx 1000/1000                38 1970-01-01 00:00 squashfs-root
drwxrw-rw- 1000/1000                25 1970-01-01 00:00 squashfs-root/this
drwxrw-rw- 1000/1000                24 1970-01-01 00:00 squashfs-root/this/is
drwxrw-rw- 1000/1000                27 1970-01-01 00:00 squashfs-root/this/is/a
-rwxr-xr-x 1000/1000               255 1970-01-01 00:00 squashfs-root/this/is/a/file
drwxrw-rw- 1000/1000                26 1970-01-01 00:00 squashfs-root/usr
drwxrw-rw- 1000/1000                27 1970-01-01 00:00 squashfs-root/usr/bin
-rwxr-xr-x 1000/1000                 2 1970-01-01 00:00 squashfs-root/usr/bin/heyo
"#;

        // using contains here, the output of squashfs varies between versions
        assert_eq!(std::str::from_utf8(&output.stdout).unwrap(), expected);

        test_squashfs_tools_unsquashfs(&new_path, &control_path, None, true);
        test_bin_unsquashfs(&new_path, None, true, true);
    }

    // Test downing the compression level
    let file = BufReader::new(File::open(&new_path).unwrap());
    let fs = FilesystemReader::from_reader(file).unwrap();
    let mut fs = FilesystemWriter::from_fs_reader(&fs).unwrap();
    let mut xz_extra = ExtraXz::default();
    xz_extra.level(1).unwrap();
    let extra = CompressionExtra::Xz(xz_extra);
    let mut compressor = FilesystemCompressor::new(Compressor::Xz, None).unwrap();
    compressor.extra(extra).unwrap();
    fs.set_compressor(compressor);

    // create the modified squashfs
    let new_path2 = dir.join("bytes_less_xz.squashfs");
    let new_path2 = new_path2.to_str().unwrap();
    let mut output = BufWriter::new(File::create(&new_path2).unwrap());
    let (_superblock, _bytes_written) = fs.write(&mut output).unwrap();

    // compare
    #[cfg(feature = "__test_unsquashfs")]
    {
        test_squashfs_tools_unsquashfs(&new_path2, &control_path, None, true);
        test_bin_unsquashfs(&new_path2, None, true, true);
    }

    // Test picking a different compression
    let file = BufReader::new(File::open(&new_path2).unwrap());
    let fs = FilesystemReader::from_reader(file).unwrap();
    let mut fs = FilesystemWriter::from_fs_reader(&fs).unwrap();
    let compressor = FilesystemCompressor::new(Compressor::Gzip, None).unwrap();
    fs.set_compressor(compressor);

    // create the modified squashfs
    let new_path3 = dir.join("bytes_gzip.squashfs");
    let new_path3 = new_path3.to_str().unwrap();
    let mut output = BufWriter::new(File::create(&new_path3).unwrap());
    let (_superblock, _bytes_written) = fs.write(&mut output).unwrap();

    // compare
    #[cfg(feature = "__test_unsquashfs")]
    {
        test_squashfs_tools_unsquashfs(&new_path3, &control_path, None, true);
        test_bin_unsquashfs(&new_path3, None, true, true);
    }

    // Test changing block size
    let file = BufReader::new(File::open(&new_path3).unwrap());
    let fs = FilesystemReader::from_reader(file).unwrap();
    let mut fs = FilesystemWriter::from_fs_reader(&fs).unwrap();
    fs.set_block_size(DEFAULT_BLOCK_SIZE * 2);

    // create the modified squashfs
    let new_path4 = dir.join("bytes_bigger_blocks.squashfs");
    let new_path4 = new_path4.to_str().unwrap();
    let mut output = BufWriter::new(File::create(&new_path4).unwrap());
    let (_superblock, _bytes_written) = fs.write(&mut output).unwrap();

    // compare
    #[cfg(feature = "__test_unsquashfs")]
    {
        test_squashfs_tools_unsquashfs(&new_path4, &control_path, None, true);
        test_bin_unsquashfs(&new_path4, None, true, true);
    }
}
