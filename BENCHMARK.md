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
Architecture:             x86_64
  CPU op-mode(s):         32-bit, 64-bit
  Address sizes:          39 bits physical, 48 bits virtual
  Byte Order:             Little Endian
CPU(s):                   8
  On-line CPU(s) list:    0-7
Vendor ID:                GenuineIntel
  Model name:             Intel(R) Core(TM) i7-7700K CPU @ 4.20GHz
    CPU family:           6
    Model:                158
    Thread(s) per core:   2
    Core(s) per socket:   4
    Socket(s):            1
    Stepping:             9
    CPU(s) scaling MHz:   76%
    CPU max MHz:          4500.0000
    CPU min MHz:          800.0000
    BogoMIPS:             8403.00
    Flags:                fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush
                          dts acpi mmx fxsr sse sse2 ss ht tm pbe syscall nx pdpe1gb rdtscp lm constant_ts
                          c art arch_perfmon pebs bts rep_good nopl xtopology nonstop_tsc cpuid aperfmperf
                           pni pclmulqdq dtes64 monitor ds_cpl vmx est tm2 ssse3 sdbg fma cx16 xtpr pdcm p
                          cid sse4_1 sse4_2 x2apic movbe popcnt tsc_deadline_timer aes xsave avx f16c rdra
                          nd lahf_lm abm 3dnowprefetch cpuid_fault pti ssbd ibrs ibpb stibp tpr_shadow fle
                          xpriority ept vpid ept_ad fsgsbase tsc_adjust bmi1 avx2 smep bmi2 erms invpcid m
                          px rdseed adx smap clflushopt intel_pt xsaveopt xsavec xgetbv1 xsaves dtherm ida
                           arat pln pts hwp hwp_notify hwp_act_window hwp_epp vnmi md_clear flush_l1d arch
                          _capabilities
Virtualization features:
  Virtualization:         VT-x
Caches (sum of all):
  L1d:                    128 KiB (4 instances)
  L1i:                    128 KiB (4 instances)
  L2:                     1 MiB (4 instances)
  L3:                     8 MiB (1 instance)
NUMA:
  NUMA node(s):           1
  NUMA node0 CPU(s):      0-7
```

</details>

```
$ ./bench.bash
```

## Wall time: `backhand/unsquashfs` vs `squashfs-tools/unsquashfs-4.6.1`
### `openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.18.0` | 50.0 ± 2.7 | 46.4 | 56.3 | 1.02 ± 0.09 |
| `backhand-dist` | 49.0 ± 3.4 | 42.4 | 56.6 | 1.00 |
| `backhand-dist-musl` | 77.7 ± 4.7 | 66.7 | 87.9 | 1.59 ± 0.15 |
| `squashfs-tools` | 108.8 ± 13.0 | 84.7 | 142.8 | 2.22 ± 0.31 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.18.0` | 51.0 ± 2.9 | 45.1 | 57.3 | 1.02 ± 0.09 |
| `backhand-dist` | 50.2 ± 3.1 | 45.9 | 60.7 | 1.00 |
| `backhand-dist-musl` | 77.0 ± 4.4 | 69.6 | 85.8 | 1.54 ± 0.13 |
| `squashfs-tools` | 112.2 ± 14.2 | 81.0 | 137.9 | 2.24 ± 0.31 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.18.0` | 252.7 ± 15.0 | 217.5 | 271.6 | 1.32 ± 0.10 |
| `backhand-dist` | 253.3 ± 15.0 | 219.3 | 271.4 | 1.32 ± 0.10 |
| `backhand-dist-musl` | 423.6 ± 25.7 | 370.5 | 456.1 | 2.21 ± 0.17 |
| `squashfs-tools` | 191.6 ± 9.5 | 166.1 | 214.2 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.18.0` | 225.4 ± 7.0 | 214.0 | 242.2 | 1.00 ± 0.04 |
| `backhand-dist` | 224.3 ± 6.3 | 214.5 | 246.7 | 1.00 |
| `backhand-dist-musl` | 360.1 ± 12.9 | 343.9 | 395.1 | 1.61 ± 0.07 |
| `squashfs-tools` | 285.1 ± 10.3 | 259.5 | 305.5 | 1.27 ± 0.06 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.18.0` | 104.9 ± 4.6 | 96.4 | 120.9 | 1.00 |
| `backhand-dist` | 104.9 ± 4.0 | 98.1 | 119.2 | 1.00 ± 0.06 |
| `backhand-dist-musl` | 172.2 ± 7.8 | 152.3 | 191.2 | 1.64 ± 0.10 |
| `squashfs-tools` | 188.6 ± 14.1 | 159.1 | 217.2 | 1.80 ± 0.16 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.18.0` | 689.8 ± 7.9 | 675.7 | 705.9 | 3.41 ± 0.15 |
| `backhand-dist` | 438.0 ± 4.2 | 430.1 | 448.3 | 2.16 ± 0.10 |
| `backhand-dist-musl` | 519.5 ± 3.2 | 511.8 | 526.1 | 2.57 ± 0.11 |
| `squashfs-tools` | 202.4 ± 8.9 | 187.0 | 223.5 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.18.0` | 1.395 ± 0.011 | 1.372 | 1.422 | 1.20 ± 0.01 |
| `backhand-dist` | 1.372 ± 0.008 | 1.358 | 1.387 | 1.18 ± 0.01 |
| `backhand-dist-musl` | 1.160 ± 0.005 | 1.151 | 1.176 | 1.00 |
| `squashfs-tools` | 1.827 ± 0.046 | 1.745 | 1.990 | 1.57 ± 0.04 |
### `airootfs.sfs`
| Command | Mean [µs] | Min [µs] | Max [µs] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.18.0` | 813.8 ± 62.2 | 720.6 | 948.4 | 1.27 ± 0.13 |
| `backhand-dist` | 793.0 ± 92.2 | 708.4 | 1340.4 | 1.23 ± 0.17 |
| `backhand-dist-musl` | 1110.4 ± 88.0 | 893.1 | 1381.3 | 1.73 ± 0.18 |
| `squashfs-tools` | 643.0 ± 43.3 | 571.8 | 798.9 | 1.00 |

## Heap Usage: `backhand/unsquashfs` vs `squashfs-tools/unsquashfs-4.6.1`
```
$ cargo +stable build -p backhand-cli --bins --locked --profile=dist
```

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 34.4MB |
| `heaptrack unsquashfs -quiet -no-progress -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 76.8MB |

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 52.3MB |
| `heaptrack unsquashfs -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 103.4MB |
