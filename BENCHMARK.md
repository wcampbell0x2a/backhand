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
| `backhand-dist-v0.14.2-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 177.3 ± 10.0 | 165.3 | 219.7 | 1.06 ± 0.08 |
| `backhand-dist-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 195.8 ± 10.6 | 182.9 | 236.5 | 1.17 ± 0.08 |
| `backhand-dist-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 175.2 ± 16.5 | 163.7 | 264.5 | 1.05 ± 0.11 |
| `squashfs-tools-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 166.8 ± 7.5 | 153.7 | 188.3 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2--openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 177.4 ± 5.4 | 170.1 | 190.5 | 1.12 ± 0.07 |
| `backhand-dist-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 193.9 ± 6.9 | 187.7 | 232.1 | 1.22 ± 0.08 |
| `backhand-dist-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 170.1 ± 5.4 | 163.9 | 189.2 | 1.07 ± 0.06 |
| `squashfs-tools-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 159.0 ± 8.1 | 149.5 | 179.2 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-870D97.squashfs` | 841.3 ± 20.1 | 793.6 | 868.3 | 1.26 ± 0.03 |
| `backhand-dist-870D97.squashfs` | 963.6 ± 24.0 | 921.0 | 1019.6 | 1.44 ± 0.04 |
| `backhand-dist-870D97.squashfs` | 833.1 ± 24.7 | 791.7 | 883.8 | 1.25 ± 0.04 |
| `squashfs-tools-870D97.squashfs` | 667.1 ± 4.8 | 660.4 | 675.2 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-img-1571203182_vol-ubi_rootfs.ubifs` | 905.3 ± 15.7 | 891.8 | 995.9 | 1.01 ± 0.03 |
| `backhand-dist-img-1571203182_vol-ubi_rootfs.ubifs` | 1014.2 ± 12.0 | 995.1 | 1048.1 | 1.13 ± 0.04 |
| `backhand-dist-img-1571203182_vol-ubi_rootfs.ubifs` | 895.2 ± 26.2 | 870.3 | 1006.7 | 1.00 |
| `squashfs-tools-img-1571203182_vol-ubi_rootfs.ubifs` | 929.4 ± 12.3 | 910.3 | 965.6 | 1.04 ± 0.03 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-2611E3.squashfs` | 422.5 ± 15.1 | 398.4 | 466.1 | 1.05 ± 0.06 |
| `backhand-dist-2611E3.squashfs` | 461.3 ± 26.6 | 429.5 | 508.8 | 1.15 ± 0.09 |
| `backhand-dist-2611E3.squashfs` | 402.0 ± 19.7 | 376.2 | 447.8 | 1.00 |
| `squashfs-tools-2611E3.squashfs` | 447.2 ± 12.2 | 416.8 | 469.5 | 1.11 ± 0.06 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-Plexamp-4.6.1.AppImage` | 726.9 ± 10.0 | 705.8 | 750.8 | 1.41 ± 0.02 |
| `backhand-dist-Plexamp-4.6.1.AppImage` | 1345.6 ± 12.3 | 1318.9 | 1372.9 | 2.60 ± 0.03 |
| `backhand-dist-Plexamp-4.6.1.AppImage` | 761.9 ± 9.6 | 735.7 | 780.4 | 1.47 ± 0.02 |
| `squashfs-tools-Plexamp-4.6.1.AppImage` | 516.7 ± 4.3 | 509.6 | 527.9 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.14.2-crates-io.squashfs` | 67.632 ± 0.266 | 66.927 | 68.240 | 27.68 ± 0.35 |
| `backhand-dist-crates-io.squashfs` | 2.444 ± 0.030 | 2.398 | 2.540 | 1.00 |
| `backhand-dist-crates-io.squashfs` | 2.519 ± 0.013 | 2.488 | 2.555 | 1.03 ± 0.01 |
| `squashfs-tools-crates-io.squashfs` | 2.519 ± 0.060 | 2.393 | 2.639 | 1.03 ± 0.03 |

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
