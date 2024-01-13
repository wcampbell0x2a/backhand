#!/bin/bash

BACKHAND="./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand"
UNSQUASHFS="/usr/bin/unsquashfs"

bench () {
    file $1
    hyperfine --runs 50 --warmup 10 --export-markdown bench-results/$3.md -i \
        "$BACKHAND --quiet -f -d $(mktemp -d /tmp/BHXXX) -o $(rz-ax $2) $1" \
        "$UNSQUASHFS -quiet -no-progress -d $(mktemp -d /tmp/BHXXX)      -f -o $(rz-ax $2) -ignore-errors $1"
}

cross +stable build -p backhand-cli --bins --locked --target x86_64-unknown-linux-musl --profile=dist
mkdir -p bench-results

# xz
bench "backhand-test/test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin" 0x225fd0 openwrt1
# xz
bench "backhand-test/test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img" 0x2c0080 openwrt2
# xz
bench "backhand-test/test-assets/test_re815_xev160/870D97.squashfs" 0x0 re815
# xz
bench "backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs" 0x0 ax18000
# xz
#bench "test-assets/test_archlinux_iso_rootfs/airootfs.sfs" 0x0
# xz
bench "backhand-test/test-assets/test_er605_v2_2/2611E3.squashfs" 0x0 er605
# gzip
bench "backhand-test/test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage" 0x2dfe8 plexamp

rm -rf /tmp/BH*
