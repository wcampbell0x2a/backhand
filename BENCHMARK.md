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
| `backhand-dist-musl-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 205.4 ± 6.0 | 190.5 | 220.8 | 1.17 ± 0.06 |
| `backhand-dist-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 184.8 ± 4.7 | 176.4 | 196.3 | 1.05 ± 0.06 |
| `squashfs-tools-openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin` | 176.3 ± 8.2 | 162.6 | 205.3 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 209.7 ± 11.5 | 197.5 | 255.4 | 1.25 ± 0.09 |
| `backhand-dist-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 189.5 ± 8.3 | 171.3 | 212.3 | 1.13 ± 0.07 |
| `squashfs-tools-openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img` | 167.2 ± 7.1 | 156.0 | 194.7 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-870D97.squashfs` | 1.029 ± 0.035 | 0.963 | 1.095 | 1.44 ± 0.05 |
| `backhand-dist-870D97.squashfs` | 0.900 ± 0.023 | 0.838 | 0.946 | 1.26 ± 0.03 |
| `squashfs-tools-870D97.squashfs` | 0.716 ± 0.002 | 0.712 | 0.721 | 1.00 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-img-1571203182_vol-ubi_rootfs.ubifs` | 1.121 ± 0.047 | 1.073 | 1.291 | 1.18 ± 0.05 |
| `backhand-dist-img-1571203182_vol-ubi_rootfs.ubifs` | 1.003 ± 0.078 | 0.920 | 1.293 | 1.06 ± 0.08 |
| `squashfs-tools-img-1571203182_vol-ubi_rootfs.ubifs` | 0.949 ± 0.016 | 0.923 | 0.997 | 1.00 |

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-2611E3.squashfs` | 477.5 ± 21.9 | 446.3 | 528.8 | 1.12 ± 0.07 |
| `backhand-dist-2611E3.squashfs` | 425.8 ± 15.7 | 396.4 | 472.3 | 1.00 |
| `squashfs-tools-2611E3.squashfs` | 453.5 ± 16.1 | 425.1 | 496.6 | 1.07 ± 0.05 |

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl-Plexamp-4.6.1.AppImage` | 1.264 ± 0.014 | 1.236 | 1.297 | 2.41 ± 0.07 |
| `backhand-dist-Plexamp-4.6.1.AppImage` | 0.718 ± 0.010 | 0.698 | 0.745 | 1.37 ± 0.04 |
| `squashfs-tools-Plexamp-4.6.1.AppImage` | 0.524 ± 0.015 | 0.514 | 0.603 | 1.00 |

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
