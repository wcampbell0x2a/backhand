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
| `backhand-dist-musl-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 189.9 ± 3.9 | 183.0 | 199.7 | 1.15 ± 0.06 |
| `backhand-dist-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 167.4 ± 3.8 | 162.0 | 177.6 | 1.01 ± 0.05 |
| `squashfs-tools-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 165.0 ± 7.5 | 151.8 | 185.5 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 191.8 ± 3.7 | 185.8 | 203.9 | 1.21 ± 0.05 |
| `backhand-dist-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 170.5 ± 5.2 | 163.4 | 185.4 | 1.07 ± 0.05 |
| `squashfs-tools-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 158.8 ± 6.2 | 147.3 | 178.0 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-870D97.squashfs` | 969.4 ± 28.3 | 919.4 | 1019.1 | 1.46 ± 0.04 |
| `backhand-dist-870D97.squashfs` | 834.9 ± 25.9 | 790.0 | 877.4 | 1.25 ± 0.04 |
| `squashfs-tools-870D97.squashfs` | 666.3 ± 3.5 | 660.3 | 672.7 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-img-1571203182_vol-ubi_rootfs.ubifs` | 1.013 ± 0.014 | 0.994 | 1.054 | 1.14 ± 0.02 |
| `backhand-dist-img-1571203182_vol-ubi_rootfs.ubifs` | 0.890 ± 0.011 | 0.869 | 0.927 | 1.00 |
| `squashfs-tools-img-1571203182_vol-ubi_rootfs.ubifs` | 0.928 ± 0.015 | 0.896 | 0.955 | 1.04 ± 0.02 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-2611E3.squashfs` | 466.0 ± 26.3 | 428.9 | 505.5 | 1.15 ± 0.09 |
| `backhand-dist-2611E3.squashfs` | 404.1 ± 22.1 | 382.8 | 446.9 | 1.00 |
| `squashfs-tools-2611E3.squashfs` | 447.8 ± 15.7 | 410.5 | 479.9 | 1.11 ± 0.07 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-Plexamp-4.6.1.AppImage` | 1.351 ± 0.011 | 1.317 | 1.368 | 2.65 ± 0.04 |
| `backhand-dist-Plexamp-4.6.1.AppImage` | 0.744 ± 0.011 | 0.721 | 0.768 | 1.46 ± 0.03 |
| `squashfs-tools-Plexamp-4.6.1.AppImage` | 0.509 ± 0.006 | 0.502 | 0.536 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-crates-io.squashfs` | 2.520 ± 0.015 | 2.499 | 2.567 | 1.00 |
| `backhand-dist-crates-io.squashfs` | 2.523 ± 0.038 | 2.495 | 2.778 | 1.00 ± 0.02 |
| `squashfs-tools-crates-io.squashfs` | 2.526 ± 0.064 | 2.396 | 2.664 | 1.00 ± 0.03 |

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
