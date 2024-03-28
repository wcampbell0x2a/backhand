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
### `openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl` | 195.1 ± 10.3 | 185.0 | 233.9 | 1.16 ± 0.09 |
| `backhand-dist-v0.15.0` | 170.9 ± 7.3 | 163.0 | 200.1 | 1.01 ± 0.07 |
| `backhand-dist` | 170.6 ± 7.0 | 159.8 | 202.7 | 1.01 ± 0.07 |
| `squashfs-tools` | 168.6 ± 9.8 | 154.7 | 195.8 | 1.00 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl` | 196.1 ± 5.6 | 188.9 | 213.4 | 1.22 ± 0.07 |
| `backhand-dist-v0.15.0` | 173.7 ± 7.3 | 164.9 | 197.6 | 1.08 ± 0.07 |
| `backhand-dist` | 175.7 ± 10.3 | 160.7 | 215.8 | 1.09 ± 0.08 |
| `squashfs-tools` | 160.8 ± 7.7 | 147.4 | 188.5 | 1.00 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl` | 990.9 ± 34.6 | 929.5 | 1072.6 | 1.45 ± 0.06 |
| `backhand-dist-v0.15.0` | 846.9 ± 27.6 | 793.2 | 892.7 | 1.24 ± 0.05 |
| `backhand-dist` | 848.7 ± 28.8 | 801.0 | 892.8 | 1.24 ± 0.05 |
| `squashfs-tools` | 682.1 ± 13.5 | 668.5 | 731.3 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl` | 1038.8 ± 28.0 | 1005.7 | 1184.9 | 1.14 ± 0.05 |
| `backhand-dist-v0.15.0` | 915.0 ± 28.6 | 883.8 | 1020.2 | 1.00 |
| `backhand-dist` | 920.3 ± 36.7 | 884.9 | 1037.0 | 1.01 ± 0.05 |
| `squashfs-tools` | 945.2 ± 18.4 | 905.3 | 1004.2 | 1.03 ± 0.04 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl` | 471.4 ± 25.2 | 429.4 | 515.0 | 1.15 ± 0.08 |
| `backhand-dist-v0.15.0` | 409.3 ± 19.3 | 378.8 | 457.4 | 1.00 |
| `backhand-dist` | 411.8 ± 25.3 | 385.1 | 521.9 | 1.01 ± 0.08 |
| `squashfs-tools` | 449.8 ± 15.5 | 422.0 | 499.1 | 1.10 ± 0.06 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl` | 1364.9 ± 19.4 | 1329.6 | 1407.3 | 2.59 ± 0.13 |
| `backhand-dist-v0.15.0` | 773.3 ± 13.5 | 746.4 | 805.9 | 1.47 ± 0.08 |
| `backhand-dist` | 755.9 ± 13.8 | 733.3 | 796.9 | 1.43 ± 0.08 |
| `squashfs-tools` | 527.0 ± 26.3 | 506.9 | 646.9 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-musl` | 2.941 ± 0.089 | 2.833 | 3.233 | 1.15 ± 0.04 |
| `backhand-dist-v0.15.0` | 2.988 ± 0.186 | 2.886 | 3.867 | 1.16 ± 0.08 |
| `backhand-dist` | 2.875 ± 0.134 | 2.731 | 3.412 | 1.12 ± 0.06 |
| `squashfs-tools` | 2.568 ± 0.051 | 2.477 | 2.733 | 1.00 |

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
