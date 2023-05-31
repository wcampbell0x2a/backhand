#!/bin/bash

BACKHAND="./target/release/unsquashfs"
BACKHAND_MSRV="./target-msrv/release/unsquashfs"
BACKHAND_MUSL="./target/x86_64-unknown-linux-musl/release/unsquashfs"
UNSQUASHFS="/usr/bin/unsquashfs"

bench () {
    file $1
    hyperfine --runs 20 --warmup 5 -i "$BACKHAND -f -d $(mktemp -d /tmp/BHXXX) -o $(rz-ax $2) $1" \
        "$BACKHAND_MUSL -f -d $(mktemp -d /tmp/BHXXX) -o $(rz-ax $2) $1" \
        "$BACKHAND_MSRV -f -d $(mktemp -d /tmp/BHXXX) -o $(rz-ax $2) $1" \
        "$UNSQUASHFS -d $(mktemp -d /tmp/BHXXX) -p 1 -f -o $(rz-ax $2) -ignore-errors $1" \
        "$UNSQUASHFS -d $(mktemp -d /tmp/BHXXX)      -f -o $(rz-ax $2) -ignore-errors $1"
}

# install msrv (make sure no perf regressions)
rustup toolchain install 1.64.0
cargo +1.64.0 build --release --target-dir target-msrv
cargo build --release
cargo build --release --target x86_64-unknown-linux-musl

# xz
bench "test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin" 0x225fd0
# xz
bench "test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img" 0x2c0080
# xz
bench "test-assets/test_re815_xev160/870D97.squashfs" 0x0
# xz
bench "test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs" 0x0
# xz
#bench "test-assets/test_archlinux_iso_rootfs/airootfs.sfs" 0x0
# xz
bench "test-assets/test_er605_v2_2/2611E3.squashfs" 0x0
# gzip
bench "test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage" 0x2dfe8
