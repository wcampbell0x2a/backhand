mod common;

use backhand::compression::Compressor;
use backhand::internal::Id;
use backhand::{kind, FilesystemWriter, NodeHeader, SquashfsDir};
use common::test_unsquashfs;
use test_assets::TestAssetDef;

#[test]
#[cfg(feature = "xz")]
fn test_raw_00() {
    use backhand::{CompressionExtra, ExtraXz, FilesystemCompressor};

    let asset_defs = [TestAssetDef {
        filename: "control.squashfs".to_string(),
        hash: "a2970a4e82014740b2333f4555eecf321898633ccadb443affec966f47f3acb3".to_string(),
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

    let mut fs: FilesystemWriter<'_, std::fs::File> = FilesystemWriter::new(
        kind::LE_V4_0,
        0x0004_0000,
        0x634f_5237,
        Id::root(),
        SquashfsDir { header },
        compressor,
    );

    fs.push_dir("usr", o_header);
    fs.push_dir("usr/bin", o_header);
    fs.push_file(
        std::io::Cursor::new(vec![0x00, 0x01]),
        "usr/bin/heyo",
        header,
    );
    fs.push_file(
        std::io::Cursor::new(vec![0x0f; 0xff]),
        "this/is/a/file",
        header,
    );

    // create the modified squashfs
    let mut output = std::fs::File::create(&new_path).unwrap();
    fs.write(&mut output).unwrap();

    // compare
    let control_new_path = format!("{TEST_PATH}/control.squashfs");
    test_unsquashfs(&new_path, &control_new_path, None);
}
