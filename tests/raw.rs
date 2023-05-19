mod common;

use backhand::compression::Compressor;
use backhand::{
    kind, CompressionExtra, ExtraXz, FilesystemCompressor, FilesystemWriter, NodeHeader,
    DEFAULT_BLOCK_SIZE,
};
use common::test_unsquashfs;
use test_assets::TestAssetDef;

#[test]
#[cfg(feature = "xz")]
fn test_raw_00() {
    use backhand::kind::Kind;
    use backhand::SuperBlock_V4_0;

    let asset_defs = [TestAssetDef {
        filename: "control.squashfs".to_string(),
        hash: "e3d8f94f8402412ecf742d44680f1dd5d8fd28cc3d1a502e5fcfcc9e2f5f949a".to_string(),
        url: "wcampbell.dev/squashfs/testing/test_raw_00/control.squashfs".to_string(),
    }];
    const TEST_PATH: &str = "test-assets/test_raw_00";
    let new_path = format!("{TEST_PATH}/bytes.squashfs");
    test_assets::download_test_files(&asset_defs, TEST_PATH, true).unwrap();

    let header = NodeHeader {
        permissions: 0o755,
        uid: 0,
        gid: 0,
        mtime: 0,
    };

    let o_header = NodeHeader {
        permissions: 0o766,
        ..header
    };

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
    fs.push_file(
        std::io::Cursor::new(vec![0x00, 0x01]),
        "usr/bin/heyo",
        header,
    )
    .unwrap();
    fs.push_dir_all("this/is/a", o_header).unwrap();
    fs.push_file(
        std::io::Cursor::new(vec![0x0f; 0xff]),
        "this/is/a/file",
        header,
    )
    .unwrap();

    // create the modified squashfs
    let mut output = std::io::BufWriter::new(std::fs::File::create(&new_path).unwrap());
    let (superblock, bytes_written) = fs.write(&mut output).unwrap();

    // 8KiB
    assert_eq!(bytes_written, 0x2000);

    // compare
    let control_new_path = format!("{TEST_PATH}/control.squashfs");
    test_unsquashfs(&new_path, &control_new_path, None);
}
