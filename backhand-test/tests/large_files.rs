use std::collections::HashMap;
/// Test for large squashfs images with large files and file counts
/// This verifies that backhand supports large file sizes, large block counts,
/// and large offsets in the filesystem.
///
/// The test creates a squashfs image with multiple large random files,
/// then opens it for reading and verifies that all file hashes match.
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

use backhand::compression::Compressor;
use backhand::v4::filesystem::node::InnerNode;
use backhand::{FilesystemCompressor, FilesystemReader, FilesystemWriter, NodeHeader};
use crc32fast::Hasher;
use fastrand::Rng;
use tempfile::tempdir;
use test_log::test;
use tracing::info;

/// File size group configuration
#[derive(Clone)]
struct FileSizeGroup {
    name: &'static str,
    size: u64,
    count: usize,
}

/// Test file configuration
#[derive(Clone)]
struct TestFileSpec {
    name: String,
    size: u64,
}

/// Test scenario configuration
struct TestScenario {
    description: &'static str,
    size_groups: Vec<FileSizeGroup>,
}

/// Generate random data and compute its hash
fn generate_random_data(size: usize, seed: u64) -> (Vec<u8>, String) {
    // Use fastrand with a seeded RNG for reproducible random generation
    let mut rng = Rng::with_seed(seed);
    let mut data: Vec<u8> = vec![0_u8; size];
    rng.fill(&mut data[..]);

    // Compute CRC32 hash
    let mut hasher = Hasher::new();
    hasher.update(&data);
    let hash = format!("{:08x}", hasher.finalize());

    (data, hash)
}

/// Generate test file list from size groups
fn generate_test_files(size_groups: &[FileSizeGroup]) -> Vec<TestFileSpec> {
    let mut test_files = Vec::new();
    for group in size_groups {
        for i in 0..group.count {
            let name = format!("/{}_{}", group.name, i);
            test_files.push(TestFileSpec { name, size: group.size });
        }
    }
    test_files
}

/// Write squashfs image with test files
fn write_squashfs_image(
    image_path: &Path,
    test_file_specs: &[TestFileSpec],
) -> HashMap<String, String> {
    info!("Creating FilesystemWriter and adding {} test files", test_file_specs.len());
    let mut fs_writer = FilesystemWriter::default();
    let compressor = FilesystemCompressor::new(Compressor::Zstd, None).unwrap();
    fs_writer.set_compressor(compressor);
    let default_header = NodeHeader::default();
    let mut expected_hashes = HashMap::new();

    for (i, file) in test_file_specs.iter().enumerate() {
        let (data, hash) = generate_random_data(file.size as usize, i as u64);
        expected_hashes.insert(file.name.clone(), hash);

        let cursor = Cursor::new(data);
        fs_writer
            .push_file(cursor, &file.name, default_header)
            .expect(&format!("Failed to add file {}", file.name));
    }

    // Write to squashfs image
    info!("Writing squashfs image to {:?}", image_path);
    let mut output = File::create(image_path).expect("Failed to create output file");
    fs_writer.write(&mut output).expect("Failed to write squashfs image");

    info!(
        "Successfully wrote squashfs image ({} bytes)",
        std::fs::metadata(image_path).unwrap().len()
    );

    expected_hashes
}

/// Verify squashfs image file hashes
fn verify_squashfs_image(
    image_path: &Path,
    test_file_specs: &[TestFileSpec],
    expected_hashes: &HashMap<String, String>,
) {
    info!("Opening squashfs image for reading");
    let input = BufReader::new(File::open(image_path).expect("Failed to open squashfs image"));
    let fs_reader = FilesystemReader::from_reader(input).expect("Failed to read squashfs image");

    info!("Successfully opened squashfs image");

    // Verify each file
    for file in test_file_specs {
        // Find the file in the filesystem
        let file_node = fs_reader
            .files()
            .find(|node| node.fullpath.to_string_lossy() == file.name.as_str())
            .expect(&format!("File {} not found in image", file.name));

        // Extract file data from the node
        let computed_hash = match &file_node.inner {
            InnerNode::File(file_reader) => {
                let filesystem_file = fs_reader.file(file_reader);
                let mut reader = filesystem_file.reader();
                let mut hasher = Hasher::new();
                let mut buf = vec![0u8; 4096];
                while let Ok(bytes_read) = reader.read(&mut buf) {
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buf[..bytes_read]);
                }
                format!("{:08x}", hasher.finalize())
            }
            _ => panic!("Node {} is not a file", file.name),
        };

        let expected_hash =
            expected_hashes.get(&file.name).expect(&format!("No expected hash for {}", file.name));

        assert_eq!(
            &computed_hash, expected_hash,
            "Hash mismatch for file {}: computed {} != expected {}",
            file.name, computed_hash, expected_hash
        );
    }
}

/// Run a test scenario
fn run_test_scenario(scenario: &TestScenario) {
    info!("Starting test scenario: {}", scenario.description);

    let temp_dir = tempdir().unwrap();
    let image_path = temp_dir.path().join("test.squashfs");

    let test_files = generate_test_files(&scenario.size_groups);
    let expected_hashes = write_squashfs_image(&image_path, &test_files);
    verify_squashfs_image(&image_path, &test_files, &expected_hashes);

    info!("Test scenario completed successfully");
}

#[test]
fn test_small_and_large_mixed() {
    let scenario = TestScenario {
        description: "Many small files with multiple gigabyte-scale files",
        size_groups: vec![
            FileSizeGroup { name: "small_first_8kib", size: 8 * 1024, count: 2000 },
            FileSizeGroup { name: "xxlarge_4gb", size: 4u64 * 1024 * 1024 * 1024, count: 2 },
            FileSizeGroup { name: "large_256mb", size: 256 * 1024 * 1024, count: 4 },
            FileSizeGroup { name: "large_512mb", size: 512 * 1024 * 1024, count: 2 },
            FileSizeGroup { name: "small_last_8kib", size: 8 * 1024, count: 2000 },
        ],
    };
    run_test_scenario(&scenario);
}

#[test]
fn test_full_spectrum() {
    let scenario = TestScenario {
        description: "Full spectrum from empty to 8GB files",
        size_groups: vec![
            FileSizeGroup { name: "small_first_8kib", size: 8 * 1024, count: 2000 },
            FileSizeGroup { name: "xxlarge_4gb", size: 4u64 * 1024 * 1024 * 1024, count: 1 },
            FileSizeGroup { name: "xxxlarge_8gb", size: 8u64 * 1024 * 1024 * 1024, count: 1 },
            FileSizeGroup { name: "empty", size: 0, count: 2 },
            FileSizeGroup { name: "small_1kb", size: 1024, count: 4 },
            FileSizeGroup { name: "small_10mb", size: 10 * 1024 * 1024, count: 2 },
            FileSizeGroup { name: "medium_50mb", size: 50 * 1024 * 1024, count: 2 },
            FileSizeGroup { name: "medium_100mb", size: 100 * 1024 * 1024, count: 1 },
            FileSizeGroup { name: "large_256mb", size: 256 * 1024 * 1024, count: 1 },
            FileSizeGroup { name: "large_512mb", size: 512 * 1024 * 1024, count: 1 },
            FileSizeGroup { name: "xlarge_1gb", size: 1024 * 1024 * 1024, count: 1 },
            FileSizeGroup { name: "small_last_8kib", size: 8 * 1024, count: 2000 },
        ],
    };
    run_test_scenario(&scenario);
}

#[test]
fn test_unaligned_boundaries() {
    let scenario = TestScenario {
        description: "Multiple large files with non-aligned boundaries",
        size_groups: vec![
            FileSizeGroup { name: "xxlarge_4gb", size: 4u64 * 1024 * 1024 * 1024 + 1024, count: 2 },
            FileSizeGroup { name: "large_256mb", size: 256 * 1024 * 1024 + 1024, count: 4 },
            FileSizeGroup { name: "large_512mb", size: 512 * 1024 * 1024, count: 2 },
        ],
    };
    run_test_scenario(&scenario);
}
