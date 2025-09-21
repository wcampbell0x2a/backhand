mod common;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};

use backhand::compression::{CompressionAction, Compressor, DefaultCompressor};
use backhand::kind::{self, Kind};
use backhand::traits::SimpleCompression;
use backhand::{BackhandError, FilesystemCompressor, FilesystemReader, FilesystemWriter};
use test_assets_ureq::TestAssetDef;
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
    pad: Option<u32>,
) {
    common::download_backoff(&assets_defs, test_path);
    let og_path = format!("{test_path}/{filepath}");
    let new_path = format!("{test_path}/bytes.squashfs");
    {
        let file = BufReader::new(File::open(og_path).unwrap());
        info!("calling from_reader");
        let og_filesystem =
            FilesystemReader::from_reader_with_offset_and_kind(file, offset, Kind::from_kind(kind))
                .unwrap();
        let mut new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();
        if let Some(pad) = pad {
            new_filesystem.set_kib_padding(pad);
        }

        // Test Debug is impl'ed properly on FilesystemWriter
        let _ = format!("{new_filesystem:#02x?}");

        // convert to bytes
        info!("calling to_bytes");
        let mut output = BufWriter::new(File::create(&new_path).unwrap());
        new_filesystem.write_with_offset(&mut output, offset).unwrap();
    }

    {
        // assert that our library can at least read the output
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

    const FILE_NAME: &str = "squashfs_v4.unblob.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "9c7c523c5d1d1cafc0b679af9092ce0289d9656f6a24bc3bd0009f95b69397c0".to_string(),
        url: "https://wcampbell.dev/squashfs/testing/test_custom/squashfs_v4.unblob.bin"
            .to_string(),
    }];
    const TEST_PATH: &str = "test-assets/non_standard_be_v4_0";
    full_test(
        &asset_defs,
        FILE_NAME,
        TEST_PATH,
        0,
        &Kind::from_const(kind::BE_V4_0).unwrap(),
        None,
    );

    // test custom kind "builder-lite"
    let _kind = Kind::new(&DefaultCompressor)
        .with_magic(kind::Magic::Big)
        .with_all_endian(kind::Endian::Big);
}

#[test]
#[cfg(feature = "gzip")]
fn test_non_standard_be_v4_1() {
    const FILE_NAME: &str = "squashfs_v4.nopad.unblob.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "a29ddc15f5a6abcabf28b7161837eb56b34111e48420e7392e648f2fdfe956ed".to_string(),
        url: "https://wcampbell.dev/squashfs/testing/test_custom/squashfs_v4.nopad.unblob.bin"
            .to_string(),
    }];
    const TEST_PATH: &str = "test-assets/non_standard_be_v4_1";
    full_test(
        &asset_defs,
        FILE_NAME,
        TEST_PATH,
        0,
        &Kind::from_const(kind::BE_V4_0).unwrap(),
        None,
    );
}

#[test]
#[cfg(feature = "gzip")]
fn test_custom_compressor() {
    use backhand::SuperBlock;

    const FILE_NAME: &str = "squashfs_v4.nopad.unblob.bin";
    let asset_defs = [TestAssetDef {
        filename: FILE_NAME.to_string(),
        hash: "a29ddc15f5a6abcabf28b7161837eb56b34111e48420e7392e648f2fdfe956ed".to_string(),
        url: "https://wcampbell.dev/squashfs/testing/test_custom/squashfs_v4.nopad.unblob.bin"
            .to_string(),
    }];

    #[derive(Copy, Clone)]
    pub struct CustomCompressor;

    // TODO: I'm confused about needing both CompressionAction and SimpleCompression...
    // Special decompress that only has support for the Rust version of gzip: lideflator for
    // decompression
    impl CompressionAction for CustomCompressor {
        type Compressor = Compressor;
        type FilesystemCompressor = FilesystemCompressor;
        type SuperBlock = SuperBlock;
        type Error = BackhandError;

        fn decompress(
            &self,
            bytes: &[u8],
            out: &mut Vec<u8>,
            compressor: Self::Compressor,
        ) -> Result<(), Self::Error> {
            if let Compressor::Gzip = compressor {
                out.resize(out.capacity(), 0);
                let mut decompressor = libdeflater::Decompressor::new();
                let amt = decompressor.zlib_decompress(bytes, out).unwrap();
                out.truncate(amt);
            } else {
                unimplemented!();
            }

            Ok(())
        }

        // Just pass to default compressor
        fn compress(
            &self,
            bytes: &[u8],
            fc: Self::FilesystemCompressor,
            block_size: u32,
        ) -> Result<Vec<u8>, Self::Error> {
            CompressionAction::compress(&DefaultCompressor, bytes, fc, block_size)
                .map_err(|e| e.into())
        }

        fn compression_options(
            &self,
            _superblock: &mut Self::SuperBlock,
            _kind: &Kind,
            _fs_compressor: Self::FilesystemCompressor,
        ) -> Result<Vec<u8>, Self::Error> {
            CompressionAction::compression_options(
                &DefaultCompressor,
                _superblock,
                _kind,
                _fs_compressor,
            )
            .map_err(|e| e.into())
        }
    }

    impl SimpleCompression for CustomCompressor {
        fn decompress(
            &self,
            bytes: &[u8],
            out: &mut Vec<u8>,
            compressor: backhand::traits::Compressor,
        ) -> Result<(), backhand::traits::BackhandError> {
            let v4_compressor = match compressor {
                backhand::traits::Compressor::None => Compressor::Gzip, // Fallback to gzip
                backhand::traits::Compressor::Gzip => Compressor::Gzip,
                _ => unimplemented!(),
            };
            CompressionAction::decompress(self, bytes, out, v4_compressor)
        }

        fn compress(
            &self,
            bytes: &[u8],
            _compressor: backhand::traits::Compressor,
            block_size: u32,
        ) -> Result<Vec<u8>, backhand::traits::BackhandError> {
            let fc = FilesystemCompressor::new(Compressor::Gzip, None).unwrap();
            CompressionAction::compress(self, bytes, fc, block_size)
        }

        fn compression_options(
            &self,
            _compressor: backhand::traits::Compressor,
            _kind: &Kind,
        ) -> Result<Vec<u8>, backhand::traits::BackhandError> {
            Ok(vec![])
        }
    }

    let kind = Kind::new_with_const(&CustomCompressor, kind::BE_V4_0);

    const TEST_PATH: &str = "test-assets/custom_compressor";
    full_test(&asset_defs, FILE_NAME, TEST_PATH, 0, &kind, Some(0));
}
