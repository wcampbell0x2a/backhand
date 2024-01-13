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

## Wall time: `backhand/unsquashfs-v0.14.0` vs `squashfs-tools/unsquashfs-4.6.1`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHnU0 -o 0 backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 1.052 ± 0.033 | 1.025 | 1.194 | 1.13 ± 0.04 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BH6PE      -f -o 0 -ignore-errors backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 0.934 ± 0.014 | 0.912 | 0.985 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHXbp -o 0 backhand-test/test-assets/test_er605_v2_2/2611E3.squashfs` | 486.1 ± 19.9 | 462.0 | 542.5 | 1.07 ± 0.06 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHCdV      -f -o 0 -ignore-errors backhand-test/test-assets/test_er605_v2_2/2611E3.squashfs` | 454.6 ± 14.9 | 420.1 | 502.1 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BH3Uj -o 2252752 backhand-test/test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 198.2 ± 4.6 | 192.7 | 217.3 | 1.18 ± 0.07 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHGE9      -f -o 2252752 -ignore-errors backhand-test/test-assets/test_openwrt_tplink_archera7v5/openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 167.7 ± 8.7 | 155.4 | 196.3 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHu2n -o 2883712 backhand-test/test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 200.7 ± 4.6 | 194.5 | 222.6 | 1.24 ± 0.07 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHidr      -f -o 2883712 -ignore-errors backhand-test/test-assets/test_openwrt_netgear_ex6100v2/openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 162.3 ± 8.2 | 147.9 | 180.3 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHqtd -o 188392 backhand-test/test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage` | 1.314 ± 0.011 | 1.294 | 1.349 | 2.55 ± 0.03 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BHXUc      -f -o 188392 -ignore-errors backhand-test/test-assets/test_appimage_plexamp/Plexamp-4.6.1.AppImage` | 0.516 ± 0.003 | 0.510 | 0.524 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `./target/x86_64-unknown-linux-musl/dist/unsquashfs-backhand --quiet -f -d /tmp/BHjpe -o 0 backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 983.0 ± 25.0 | 930.6 | 1026.3 | 1.46 ± 0.04 |
| `/usr/bin/unsquashfs -quiet -no-progress -d /tmp/BH6IH      -f -o 0 -ignore-errors backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 674.4 ± 5.4 | 667.8 | 697.8 | 1.00 |

## Heap Usage: `backhand/unsquashfs-v0.14.0` vs `squashfs-tools/unsquashfs-4.6.1`
| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/release/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 19.4MB |
| `heaptrack unsquashfs -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 75.7MB |
