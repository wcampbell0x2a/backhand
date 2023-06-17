mod common;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};

use backhand::compression::{CompressionAction, Compressor, DefaultCompressor};
use backhand::kind::{self, Kind};
use backhand::{BackhandError, FilesystemCompressor, FilesystemReader, FilesystemWriter};
use test_assets::TestAssetDef;
use test_log::test;
use tracing::info;

/// - Download file
/// - Read into Squashfs
/// - Into Filesystem
/// - Into Bytes
/// - - Into Squashfs
/// - - Into Filesystem
/// - Can't test with unsquashfs, as it doesn't support these custom filesystems
fn full_test(
    assets_defs: &[TestAssetDef],
    filepath: &str,
    test_path: &str,
    offset: u64,
    kind: &Kind,
) {
    test_assets::download_test_files(assets_defs, test_path, true).unwrap();
    let og_path = format!("{test_path}/{filepath}");
    let new_path = format!("{test_path}/bytes.squashfs");
    {
        let file = BufReader::new(File::open(og_path).unwrap());
        info!("calling from_reader");
        let og_filesystem =
            FilesystemReader::from_reader_with_offset_and_kind(file, offset, Kind::from_kind(kind))
                .unwrap();
        let mut new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();

        // Test Debug is impl'ed properly on FilesystemWriter
        let _ = format!("{new_filesystem:#02x?}");

        // convert to bytes
        info!("calling to_bytes");
        let mut output = BufWriter::new(File::create(&new_path).unwrap());
        new_filesystem
            .write_with_offset(&mut output, offset)
            .unwrap();
    }

    {
        // assert that our library can atleast read the output
        info!("calling from_reader");
        let created_file = BufReader::new(File::open(&new_path).unwrap());
        let _new_filesystem = FilesystemReader::from_reader_with_offset_and_kind(
            created_file,
            offset,
            Kind::from_kind(kind),
        )
        .unwrap();
    }
}

#[test]
#[cfg(feature = "gzip")]
fn test_non_standard_be_v4_0() {
    use backhand::compression::DefaultCompressor;

    const FILE_NAME: &str = "squashfs_v4.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "9c7c523c5d1d1cafc0b679af9092ce0289d9656f6a24bc3bd0009f95b69397c0".to_string(),
        url: "https://github.com/onekey-sec/unblob/raw/3c7e886e2616413a4d6109ba3d197f91c9596881/tests/integration/filesystem/squashfs/squashfs_v4_be/__input__/squashfs_v4.bin".to_string(),
    }];
    const TEST_PATH: &str = "test-assets/non_standard_be_v4_0";
    full_test(
        &asset_defs,
        FILE_NAME,
        TEST_PATH,
        0,
        &Kind::from_const(kind::BE_V4_0).unwrap(),
    );

    // test custom kind "builder-lite"
    let _kind = Kind::new(&DefaultCompressor)
        .with_magic(kind::Magic::Big)
        .with_all_endian(kind::Endian::Big);
}

#[test]
#[cfg(feature = "gzip")]
fn test_non_standard_be_v4_1() {
    const FILE_NAME: &str = "squashfs_v4.nopad.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "a29ddc15f5a6abcabf28b7161837eb56b34111e48420e7392e648f2fdfe956ed".to_string(),
        url: "https://github.com/onekey-sec/unblob/raw/3c7e886e2616413a4d6109ba3d197f91c9596881/tests/integration/filesystem/squashfs/squashfs_v4_be/__input__/squashfs_v4.nopad.bin".to_string(),
    }];
    const TEST_PATH: &str = "test-assets/non_standard_be_v4_1";
    full_test(
        &asset_defs,
        FILE_NAME,
        TEST_PATH,
        0,
        &Kind::from_const(kind::BE_V4_0).unwrap(),
    );
}

#[test]
#[cfg(feature = "gzip")]
fn test_custom_compressor() {
    const FILE_NAME: &str = "squashfs_v4.nopad.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "a29ddc15f5a6abcabf28b7161837eb56b34111e48420e7392e648f2fdfe956ed".to_string(),
        url: "https://github.com/onekey-sec/unblob/raw/3c7e886e2616413a4d6109ba3d197f91c9596881/tests/integration/filesystem/squashfs/squashfs_v4_be/__input__/squashfs_v4.nopad.bin".to_string(),
    }];

    #[derive(Copy, Clone)]
    pub struct CustomCompressor;

    // Special decompress that only has support for the Rust version of gzip: zune-inflate for
    // decompression
    impl CompressionAction for CustomCompressor {
        fn decompress(
            &self,
            bytes: &[u8],
            out: &mut Vec<u8>,
            compressor: Compressor,
        ) -> Result<(), BackhandError> {
            if compressor == Compressor::Gzip {
                let mut decoder = zune_inflate::DeflateDecoder::new(bytes);
                let decompressed_data = decoder.decode_zlib().unwrap();
                out.write_all(&decompressed_data)?;
            } else {
                unimplemented!();
            }

            Ok(())
        }

        // Just pass to default compressor
        fn compress(
            &self,
            bytes: &[u8],
            fc: FilesystemCompressor,
            block_size: u32,
        ) -> Result<Vec<u8>, BackhandError> {
            DefaultCompressor.compress(bytes, fc, block_size)
        }
    }

    let kind = Kind::new_with_const(&CustomCompressor, kind::BE_V4_0);

    const TEST_PATH: &str = "test-assets/non_standard_be_v4_1";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, &kind);
}
