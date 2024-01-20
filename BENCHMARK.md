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

```
$ ./bench.bash
```

## Wall time: `backhand/unsquashfs-master` vs `squashfs-tools/unsquashfs-4.6.1`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 198.0 ± 4.3 | 191.9 | 217.8 | 1.17 ± 0.06 |
| `backhand-dist-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 180.3 ± 3.5 | 172.2 | 188.4 | 1.07 ± 0.06 |
| `squashfs-tools-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 169.0 ± 8.2 | 155.6 | 192.9 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 202.4 ± 9.4 | 195.1 | 257.2 | 1.24 ± 0.09 |
| `backhand-dist-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 184.2 ± 12.6 | 166.5 | 233.7 | 1.13 ± 0.10 |
| `squashfs-tools-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 162.8 ± 8.7 | 149.9 | 188.0 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-870D97.squashfs` | 982.8 ± 42.2 | 925.1 | 1200.4 | 1.43 ± 0.07 |
| `backhand-dist-870D97.squashfs` | 867.4 ± 27.8 | 808.8 | 930.1 | 1.26 ± 0.05 |
| `squashfs-tools-870D97.squashfs` | 686.4 ± 14.1 | 676.7 | 741.1 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-img-1571203182_vol-ubi_rootfs.ubifs` | 1.068 ± 0.044 | 1.037 | 1.200 | 1.12 ± 0.06 |
| `backhand-dist-img-1571203182_vol-ubi_rootfs.ubifs` | 0.976 ± 0.054 | 0.922 | 1.096 | 1.02 ± 0.06 |
| `squashfs-tools-img-1571203182_vol-ubi_rootfs.ubifs` | 0.955 ± 0.028 | 0.924 | 1.093 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-2611E3.squashfs` | 490.4 ± 23.1 | 463.3 | 577.6 | 1.12 ± 0.06 |
| `backhand-dist-2611E3.squashfs` | 439.2 ± 12.4 | 414.2 | 469.1 | 1.00 |
| `squashfs-tools-2611E3.squashfs` | 454.3 ± 18.7 | 421.1 | 524.4 | 1.03 ± 0.05 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-Plexamp-4.6.1.AppImage` | 1.266 ± 0.017 | 1.239 | 1.321 | 2.37 ± 0.16 |
| `backhand-dist-Plexamp-4.6.1.AppImage` | 0.716 ± 0.017 | 0.695 | 0.783 | 1.34 ± 0.09 |
| `squashfs-tools-Plexamp-4.6.1.AppImage` | 0.533 ± 0.034 | 0.518 | 0.698 | 1.00 |

## Heap Usage: `backhand/unsquashfs-master` vs `squashfs-tools/unsquashfs-4.6.1`
```
$ cargo +stable build -p backhand-cli --bins --locked --profile=dist --no-default-features --features xz --features gzip-zune-inflate
```

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 19.4MB |
| `heaptrack unsquashfs -quiet -no-progress -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 75.7MB |

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 39.6MB |
| `heaptrack unsquashfs -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 114.4MB |
