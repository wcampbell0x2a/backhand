# libary benchmarks
```
$ cargo bench
```

# compare benchmarks

These benchmarks are created from `bench.bash`, on the following CPU running arch linux:

</details>

<details><summary>lscpu</summary>

```
$ lscpu
Architecture:            x86_64
  CPU op-mode(s):        32-bit, 64-bit
  Address sizes:         39 bits physical, 48 bits virtual
  Byte Order:            Little Endian
CPU(s):                  4
  On-line CPU(s) list:   0-3
Vendor ID:               GenuineIntel
  Model name:            Intel(R) Core(TM) i5-6300U CPU @ 2.40GHz
    CPU family:          6
    Model:               78
    Thread(s) per core:  2
    Core(s) per socket:  2
    Socket(s):           1
    Stepping:            3
    CPU(s) scaling MHz:  80%
    CPU max MHz:         3000.0000
    CPU min MHz:         400.0000
    BogoMIPS:            5001.23
```

</details>

## Wall time: `backhand/unsquashfs-v0.14.2` vs `squashfs-tools/unsquashfs-4.6.1`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHgGC -o 2252752 backhand-test/test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 199.8 ± 5.4 | 192.9 | 219.4 | 1.18 ± 0.07 |
| `./target/dist/unsquashfs-backhand --quiet -f -d /tmp/BHGD0 -o 2252752 backhand-test/test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 180.0 ± 4.9 | 170.1 | 190.6 | 1.06 ± 0.06 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BH7Ww      -f -o 2252752 -ignore-errors backhand-test/test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 169.0 ± 8.5 | 159.1 | 194.9 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHi7s -o 2883712 backhand-test/test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 202.6 ± 4.1 | 196.1 | 212.0 | 1.25 ± 0.06 |
| `./target/dist/unsquashfs-backhand --quiet -f -d /tmp/BHxh9 -o 2883712 backhand-test/test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 183.4 ± 8.5 | 169.2 | 219.8 | 1.13 ± 0.07 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHpvG      -f -o 2883712 -ignore-errors backhand-test/test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 162.1 ± 6.8 | 152.1 | 185.0 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BH04Q -o 0 backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 1.005 ± 0.042 | 0.949 | 1.221 | 1.44 ± 0.08 |
| `./target/dist/unsquashfs-backhand --quiet -f -d /tmp/BHNDa -o 0 backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 0.870 ± 0.023 | 0.820 | 0.911 | 1.25 ± 0.05 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHlK0      -f -o 0 -ignore-errors backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 0.698 ± 0.024 | 0.684 | 0.815 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHkLA -o 0 backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 1.070 ± 0.041 | 1.041 | 1.243 | 1.12 ± 0.05 |
| `./target/dist/unsquashfs-backhand --quiet -f -d /tmp/BHxKt -o 0 backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 0.964 ± 0.048 | 0.923 | 1.087 | 1.01 ± 0.05 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHmjP      -f -o 0 -ignore-errors backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 0.953 ± 0.019 | 0.925 | 1.037 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHyeJ -o 0 backhand-test/test-assets/test_er605_v2_2/2611E3.squashfs` | 489.4 ± 20.7 | 465.6 | 542.5 | 1.10 ± 0.06 |
| `./target/dist/unsquashfs-backhand --quiet -f -d /tmp/BHKaH -o 0 backhand-test/test-assets/test_er605_v2_2/2611E3.squashfs` | 444.6 ± 17.0 | 418.4 | 485.0 | 1.00 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHxoF      -f -o 0 -ignore-errors backhand-test/test-assets/test_er605_v2_2/2611E3.squashfs` | 456.1 ± 12.9 | 429.4 | 482.8 | 1.03 ± 0.05 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHQl3 -o 188392 backhand-test/test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage` | 646.1 ± 6.4 | 634.5 | 661.8 | 1.21 ± 0.02 |
| `./target/dist/unsquashfs-backhand --quiet -f -d /tmp/BHm5w -o 188392 backhand-test/test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage` | 635.6 ± 8.2 | 618.4 | 663.0 | 1.19 ± 0.02 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHUlC      -f -o 188392 -ignore-errors backhand-test/test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage` | 533.3 ± 6.6 | 527.6 | 571.7 | 1.00 |

## Heap Usage: `backhand/unsquashfs-v0.14.0` vs `squashfs-tools/unsquashfs-4.6.1`
| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/release/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 19.4MB |
| `heaptrack unsquashfs -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 75.7MB |
