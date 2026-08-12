use std::fs::File;
use std::hint::black_box;
use std::io::{BufReader, Cursor};

use backhand::{FilesystemReader, FilesystemWriter};
use gungraun::{
    Callgrind, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main,
};

mod gungraun_assets;
use gungraun_assets::download_asset;

const NETGEAR_OFFSET: u64 = 0x2c0080;
const TPLINK_OFFSET: u64 = 0;

fn setup_asset(asset_key: &str, offset: u64) -> (File, u64) {
    let path = download_asset(asset_key);
    (File::open(&path).unwrap(), offset)
}

fn read_write((file, offset): (File, u64)) {
    let file = BufReader::new(file);
    let og_filesystem = FilesystemReader::from_reader_with_offset(file, offset).unwrap();
    let mut new_filesystem = FilesystemWriter::from_fs_reader(&og_filesystem).unwrap();

    // convert to bytes
    let mut output = Cursor::new(vec![]);
    black_box(new_filesystem.write(&mut output).unwrap());
}

fn read((file, offset): (File, u64)) {
    let file = BufReader::new(file);
    black_box(FilesystemReader::from_reader_with_offset(file, offset).unwrap());
}

#[library_benchmark]
#[bench::netgear_ax6100v2(args = ("netgear_ex6100v2", NETGEAR_OFFSET), setup = setup_asset)]
#[bench::tplink_ax1800(args = ("tplink_ax1800", TPLINK_OFFSET), setup = setup_asset)]
fn bench_read_write(input: (File, u64)) {
    read_write(input)
}

#[library_benchmark]
#[bench::netgear_ax6100v2(args = ("netgear_ex6100v2", NETGEAR_OFFSET), setup = setup_asset)]
#[bench::tplink_ax1800(args = ("tplink_ax1800", TPLINK_OFFSET), setup = setup_asset)]
fn bench_read(input: (File, u64)) {
    read(input)
}

library_benchmark_group!(name = write_read, benchmarks = bench_read_write);

library_benchmark_group!(name = only_read, benchmarks = bench_read);

main!(
    config = LibraryBenchmarkConfig::default().tool(Callgrind::default());
    library_benchmark_groups = write_read, only_read
);
