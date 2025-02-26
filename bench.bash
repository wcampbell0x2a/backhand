#!/bin/bash
set -ex

LAST_RELEASE="v0.20.0"

BACKHAND_LAST_RELEASE="./last-release/unsquashfs-backhand"
BACKHAND_NATIVE_GNU="./native-gnu/dist/unsquashfs-backhand"
BACKHAND_NATIVE_MUSL="./native-musl/x86_64-unknown-linux-musl/dist/unsquashfs-backhand"
BACKHAND="./target/dist/unsquashfs-backhand"
BACKHAND_MUSL="./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand"
UNSQUASHFS="/usr/bin/unsquashfs"

# Using dynamic linked xz for perf reasons and matching unsquashfs in this testing
FLAGS="--bins --locked --profile=dist --no-default-features --features xz --features zstd --features gzip"

bench () {
    echo ""
    file $1
    hyperfine --sort command --runs 50 --warmup 10 \
        --command-name backhand-dist-${LAST_RELEASE}-musl "$BACKHAND_LAST_RELEASE --quiet -f -d $(mktemp -d /tmp/BHXXX) -o $(($2)) $1" \
        --command-name backhand-dist-musl "$BACKHAND_MUSL --quiet -f -d $(mktemp -d /tmp/BHXXX) -o $(($2)) $1" \
        --command-name backhand-dist-musl-native "$BACKHAND_NATIVE_MUSL --quiet -f -d $(mktemp -d /tmp/BHXXX) -o $(($2)) $1" \
        --command-name backhand-dist-gnu "$BACKHAND --quiet -f -d $(mktemp -d /tmp/BHXXX) -o $(($2)) $1" \
        --command-name backhand-dist-gnu-native "$BACKHAND_NATIVE_GNU  --quiet -f -d $(mktemp -d /tmp/BHXXX) -o $(($2)) $1" \
        --command-name squashfs-tools "$UNSQUASHFS -quiet -no-progress -d $(mktemp -d /tmp/BHXXX)      -f -o $(($2)) -ignore-errors $1" \
        --export-markdown bench-results/$3.md -i
    (echo "### \`$(basename $1)\`"; cat bench-results/$3.md) > bench-results/$3_final.md
}

rm -rf bench-results
rm -rf last-release 
mkdir -p  last-release
curl -sL https://github.com/wcampbell0x2a/backhand/releases/download/$LAST_RELEASE/backhand-$LAST_RELEASE-x86_64-unknown-linux-musl.tar.gz | tar xz -C last-release
cargo +stable build -p backhand-cli $FLAGS --target x86_64-unknown-linux-musl
cargo +stable build -p backhand-cli $FLAGS
RUSTFLAGS='-C target-cpu=native' cargo +stable build -p backhand-cli $FLAGS --target-dir native-gnu
RUSTFLAGS='-C target-cpu=native' cargo +stable build -p backhand-cli --target x86_64-unknown-linux-musl $FLAGS --target-dir native-musl

mkdir -p bench-results
# xz
bench "backhand-test/test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin" 0x225fd0 0_openwrt1
# xz
bench "backhand-test/test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img" 0x2c0080 1_openwrt2
# xz
bench "backhand-test/test-assets/test_re815_xev160/870D97.squashfs" 0x0 2_re815
# xz
bench "backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs" 0x0 3_ax18000
# xz
bench "test-assets/test_archlinux_iso_rootfs/airootfs.sfs" 0x0
# xz
bench "backhand-test/test-assets/test_er605_v2_2/2611E3.squashfs" 0x0 4_er605
# gzip
bench "backhand-test/test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage" 0x2dfe8 5_plexamp
# zstd
bench "backhand-test/test-assets/crates_io_zstd/crates-io.squashfs" 0x0 6_crates_zstd

rm -rf /tmp/BH*
cat bench-results/*_final.md > results.md
echo "Cool, now add results.md to BENCHMARK.md"
