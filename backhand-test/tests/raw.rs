mod common;

use backhand::compression::Compressor;
use backhand::{
    kind, CompressionExtra, ExtraXz, FilesystemCompressor, FilesystemWriter, NodeHeader,
    SuperBlock, DEFAULT_BLOCK_SIZE,
};
use common::{test_bin_unsquashfs, test_squashfs_tools_unsquashfs};
use test_assets::TestAssetDef;

#[test]
#[cfg(feature = "xz")]
fn test_raw_00() {
    use backhand::kind::Kind;

    let asset_defs = [TestAssetDef {
        filename: "control.squashfs".to_string(),
        hash: "e3d8f94f8402412ecf742d44680f1dd5d8fd28cc3d1a502e5fcfcc9e2f5f949a".to_string(),
        url: "https://wcampbell.dev/squashfs/testing/test_raw_00/control.squashfs".to_string(),
    }];
    const TEST_PATH: &str = "test-assets/test_raw_00";
    let new_path = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let header = NodeHeader { permissions: 0o755, uid: 0, gid: 0, mtime: 0 };

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
    fs.set_only_root_id();
    fs.set_root_mode(0o777);
    fs.set_compressor(compressor);
    fs.set_kind(Kind::from_const(kind::LE_V4_0).unwrap());
    fs.set_kib_padding(8);

    //don't do anything if the directory exists
    fs.push_dir_all("usr/bin", o_header).unwrap();
    fs.push_file(std::io::Cursor::new(vec![0x00, 0x01]), "usr/bin/heyo", header).unwrap();
    fs.push_dir_all("this/is/a", o_header).unwrap();
    fs.push_file(std::io::Cursor::new(vec![0x0f; 0xff]), "this/is/a/file", header).unwrap();

    // create the modified squashfs
    let mut output = std::io::BufWriter::new(std::fs::File::create(&new_path).unwrap());
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
            flags: 0x0,
            id_count: 0x1,
            version_major: 0x4,
            version_minor: 0x0,
            root_inode: 0xe0,
            bytes_used: 0x1ec,
            id_table: 0x1e4,
            xattr_table: 0xffffffffffffffff,
            inode_table: 0xac,
            dir_table: 0x136,
            frag_table: 0x1d6,
            export_table: 0xffffffffffffffff,
        }
    );

    // compare
    #[cfg(feature = "__test_unsquashfs")]
    {
        let control_new_path = format!("{TEST_PATH}/control.squashfs");
        test_squashfs_tools_unsquashfs(&new_path, &control_new_path, None, true);
        test_bin_unsquashfs(&new_path, None, true, true);
    }
}
