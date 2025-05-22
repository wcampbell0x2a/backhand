# library benchmarks
```
$ cargo bench
```

# compare benchmarks

These benchmarks are created from `bench.bash`, on the following CPU running arch linux:

> [!WARNING]  
> This is not meant to be a perfect benchmark against squashfs-tools. Certain features such
> as LTO are used for backhand and it's compression libraries, and are not enabled when using
> squashfs-tools from a package manager.

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
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 84.7 ± 5.2 | 76.7 | 94.5 | 1.55 ± 0.12 |
| `backhand-dist-musl` | 61.7 ± 2.4 | 57.1 | 66.5 | 1.13 ± 0.07 |
| `backhand-dist-musl-native` | 62.3 ± 3.8 | 58.6 | 75.9 | 1.14 ± 0.09 |
| `backhand-dist-gnu` | 56.1 ± 2.7 | 50.8 | 62.1 | 1.03 ± 0.07 |
| `backhand-dist-gnu-native` | 54.6 ± 2.4 | 51.1 | 61.1 | 1.00 |
| `squashfs-tools` | 67.9 ± 9.2 | 54.0 | 92.3 | 1.24 ± 0.18 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 87.0 ± 5.4 | 78.0 | 99.5 | 1.56 ± 0.12 |
| `backhand-dist-musl` | 62.3 ± 2.2 | 57.9 | 66.6 | 1.12 ± 0.07 |
| `backhand-dist-musl-native` | 63.1 ± 2.0 | 58.6 | 65.9 | 1.13 ± 0.07 |
| `backhand-dist-gnu` | 55.7 ± 2.7 | 51.2 | 60.8 | 1.00 |
| `backhand-dist-gnu-native` | 56.1 ± 2.5 | 52.2 | 62.9 | 1.01 ± 0.07 |
| `squashfs-tools` | 68.1 ± 7.3 | 55.9 | 81.5 | 1.22 ± 0.14 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 446.2 ± 23.7 | 408.1 | 471.3 | 2.64 ± 0.15 |
| `backhand-dist-musl` | 219.6 ± 3.0 | 216.1 | 226.8 | 1.30 ± 0.03 |
| `backhand-dist-musl-native` | 218.7 ± 1.6 | 216.9 | 221.2 | 1.29 ± 0.03 |
| `backhand-dist-gnu` | 185.0 ± 2.7 | 181.3 | 189.6 | 1.09 ± 0.03 |
| `backhand-dist-gnu-native` | 185.2 ± 3.2 | 181.7 | 191.3 | 1.09 ± 0.03 |
| `squashfs-tools` | 169.3 ± 3.1 | 165.7 | 175.3 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 375.1 ± 10.6 | 360.2 | 389.1 | 1.57 ± 0.06 |
| `backhand-dist-musl` | 281.9 ± 7.4 | 270.0 | 291.9 | 1.18 ± 0.04 |
| `backhand-dist-musl-native` | 277.9 ± 6.1 | 268.3 | 289.3 | 1.17 ± 0.04 |
| `backhand-dist-gnu` | 238.3 ± 6.1 | 229.5 | 249.9 | 1.00 |
| `backhand-dist-gnu-native` | 238.5 ± 3.5 | 233.5 | 244.3 | 1.00 ± 0.03 |
| `squashfs-tools` | 261.8 ± 10.6 | 248.6 | 277.1 | 1.10 ± 0.05 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 183.7 ± 6.1 | 175.6 | 195.4 | 1.62 ± 0.08 |
| `backhand-dist-musl` | 132.2 ± 4.0 | 126.1 | 138.9 | 1.17 ± 0.06 |
| `backhand-dist-musl-native` | 132.8 ± 4.6 | 125.9 | 141.6 | 1.17 ± 0.06 |
| `backhand-dist-gnu` | 113.0 ± 4.5 | 108.8 | 122.7 | 1.00 |
| `backhand-dist-gnu-native` | 113.5 ± 4.4 | 106.8 | 123.9 | 1.00 ± 0.06 |
| `squashfs-tools` | 150.9 ± 11.2 | 131.7 | 161.5 | 1.34 ± 0.11 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 559.5 ± 7.4 | 551.4 | 574.2 | 2.72 ± 0.05 |
| `backhand-dist-musl` | 313.2 ± 2.0 | 311.1 | 317.4 | 1.52 ± 0.02 |
| `backhand-dist-musl-native` | 300.1 ± 2.6 | 295.9 | 304.0 | 1.46 ± 0.02 |
| `backhand-dist-gnu` | 296.5 ± 1.9 | 293.6 | 299.2 | 1.44 ± 0.02 |
| `backhand-dist-gnu-native` | 285.8 ± 2.5 | 280.9 | 288.4 | 1.39 ± 0.02 |
| `squashfs-tools` | 205.4 ± 2.5 | 201.0 | 208.2 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 1.225 ± 0.009 | 1.207 | 1.238 | 1.00 |
| `backhand-dist-musl` | 1.243 ± 0.008 | 1.232 | 1.256 | 1.01 ± 0.01 |
| `backhand-dist-musl-native` | 1.245 ± 0.008 | 1.234 | 1.259 | 1.02 ± 0.01 |
| `backhand-dist-gnu` | 1.454 ± 0.013 | 1.422 | 1.465 | 1.19 ± 0.01 |
| `backhand-dist-gnu-native` | 1.463 ± 0.009 | 1.448 | 1.480 | 1.19 ± 0.01 |
| `squashfs-tools` | 1.816 ± 0.025 | 1.790 | 1.860 | 1.48 ± 0.02 |
### `airootfs.sfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 5.7 ± 0.3 | 4.8 | 6.4 | 1.00 |
| `backhand-dist-musl` | 6.4 ± 0.3 | 5.1 | 6.9 | 1.13 ± 0.07 |
| `backhand-dist-musl-native` | 6.1 ± 0.3 | 4.9 | 6.5 | 1.07 ± 0.07 |
| `backhand-dist-gnu` | 6.0 ± 0.4 | 4.7 | 6.5 | 1.05 ± 0.08 |
| `backhand-dist-gnu-native` | 5.9 ± 0.2 | 5.6 | 6.3 | 1.04 ± 0.06 |
| `squashfs-tools` | 6.7 ± 0.3 | 5.9 | 7.2 | 1.18 ± 0.07 |

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
