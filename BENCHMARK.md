# library benchmarks
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

```
$ ./bench.bash
```

## Wall time: `backhand/unsquashfs-master` vs `squashfs-tools/unsquashfs-4.6.1`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 173.9 ± 4.4 | 165.5 | 192.6 | 1.05 ± 0.04 |
| `backhand-dist-musl-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 190.6 ± 5.0 | 180.8 | 203.1 | 1.15 ± 0.05 |
| `backhand-dist-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 168.0 ± 4.5 | 162.3 | 185.3 | 1.01 ± 0.04 |
| `squashfs-tools-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 165.9 ± 5.7 | 157.2 | 179.9 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 175.3 ± 5.5 | 166.7 | 196.8 | 1.09 ± 0.06 |
| `backhand-dist-musl-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 192.2 ± 3.9 | 187.6 | 205.5 | 1.19 ± 0.06 |
| `backhand-dist-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 170.3 ± 5.0 | 154.7 | 182.1 | 1.06 ± 0.06 |
| `squashfs-tools-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 161.0 ± 7.0 | 148.8 | 179.1 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-870D97.squashfs` | 837.4 ± 22.2 | 795.5 | 868.1 | 1.25 ± 0.03 |
| `backhand-dist-musl-870D97.squashfs` | 970.6 ± 26.0 | 927.2 | 1022.3 | 1.45 ± 0.04 |
| `backhand-dist-870D97.squashfs` | 835.1 ± 24.8 | 790.3 | 883.6 | 1.25 ± 0.04 |
| `squashfs-tools-870D97.squashfs` | 668.6 ± 4.1 | 663.1 | 675.4 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-img-1571203182_vol-ubi_rootfs.ubifs` | 911.9 ± 30.4 | 887.1 | 1053.4 | 1.02 ± 0.04 |
| `backhand-dist-musl-img-1571203182_vol-ubi_rootfs.ubifs` | 1016.6 ± 16.2 | 994.2 | 1066.0 | 1.14 ± 0.03 |
| `backhand-dist-img-1571203182_vol-ubi_rootfs.ubifs` | 891.5 ± 20.1 | 868.4 | 1013.3 | 1.00 |
| `squashfs-tools-img-1571203182_vol-ubi_rootfs.ubifs` | 928.6 ± 13.0 | 900.9 | 958.0 | 1.04 ± 0.03 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-2611E3.squashfs` | 425.6 ± 12.8 | 403.5 | 455.3 | 1.07 ± 0.06 |
| `backhand-dist-musl-2611E3.squashfs` | 462.9 ± 25.4 | 432.7 | 509.2 | 1.16 ± 0.08 |
| `backhand-dist-2611E3.squashfs` | 399.1 ± 19.1 | 378.3 | 448.5 | 1.00 |
| `squashfs-tools-2611E3.squashfs` | 447.8 ± 15.7 | 416.4 | 479.7 | 1.12 ± 0.07 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-Plexamp-4.6.1.AppImage` | 724.5 ± 11.1 | 697.6 | 753.0 | 1.40 ± 0.03 |
| `backhand-dist-musl-Plexamp-4.6.1.AppImage` | 1344.0 ± 17.4 | 1310.8 | 1375.2 | 2.60 ± 0.04 |
| `backhand-dist-Plexamp-4.6.1.AppImage` | 760.9 ± 13.0 | 733.5 | 788.9 | 1.47 ± 0.03 |
| `squashfs-tools-Plexamp-4.6.1.AppImage` | 516.6 ± 4.8 | 505.1 | 526.6 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-crates-io.squashfs` | 69.386 ± 0.968 | 67.132 | 71.444 | 28.22 ± 0.47 |
| `backhand-dist-musl-crates-io.squashfs` | 2.458 ± 0.023 | 2.432 | 2.526 | 1.00 |
| `backhand-dist-crates-io.squashfs` | 2.539 ± 0.018 | 2.507 | 2.620 | 1.03 ± 0.01 |
| `squashfs-tools-crates-io.squashfs` | 2.521 ± 0.057 | 2.386 | 2.668 | 1.03 ± 0.03 |

## Heap Usage: `backhand/unsquashfs-master` vs `squashfs-tools/unsquashfs-4.6.1`
```
$ cargo +stable build -p backhand-cli --bins --locked --profile=dist --no-default-features --features xz --features gzip-zune-inflate
```

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 19.3MB |
| `heaptrack unsquashfs -quiet -no-progress -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 75.7MB |

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 39.5MB |
| `heaptrack unsquashfs -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 114.4MB |
