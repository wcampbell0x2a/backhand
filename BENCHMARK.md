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

## Wall time: `backhand/unsquashfs-3e25c7d` vs `squashfs-tools/unsquashfs-4.6.1`
### `openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist` | 49.7 ± 3.0 | 43.3 | 55.5 | 1.00 |
| `backhand-dist-musl-v0.18.0` | 78.6 ± 4.4 | 69.5 | 86.2 | 1.58 ± 0.13 |
| `backhand-dist-musl` | 78.5 ± 5.0 | 68.8 | 88.2 | 1.58 ± 0.14 |
| `squashfs-tools` | 108.2 ± 11.6 | 79.0 | 130.4 | 2.18 ± 0.27 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist` | 50.8 ± 3.1 | 44.7 | 59.0 | 1.00 |
| `backhand-dist-musl-v0.18.0` | 80.0 ± 4.6 | 72.0 | 91.1 | 1.58 ± 0.13 |
| `backhand-dist-musl` | 78.4 ± 4.3 | 71.0 | 87.6 | 1.54 ± 0.13 |
| `squashfs-tools` | 107.4 ± 11.7 | 88.9 | 135.7 | 2.12 ± 0.26 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist` | 255.5 ± 13.5 | 224.7 | 274.2 | 1.33 ± 0.11 |
| `backhand-dist-musl-v0.18.0` | 430.2 ± 26.6 | 368.4 | 469.8 | 2.23 ± 0.19 |
| `backhand-dist-musl` | 435.3 ± 23.2 | 371.2 | 462.1 | 2.26 ± 0.18 |
| `squashfs-tools` | 192.7 ± 11.8 | 167.3 | 216.3 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist` | 226.8 ± 6.4 | 215.4 | 239.8 | 1.00 |
| `backhand-dist-musl-v0.18.0` | 364.8 ± 12.4 | 343.5 | 391.6 | 1.61 ± 0.07 |
| `backhand-dist-musl` | 362.8 ± 13.1 | 341.3 | 394.7 | 1.60 ± 0.07 |
| `squashfs-tools` | 292.4 ± 14.1 | 266.1 | 345.0 | 1.29 ± 0.07 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist` | 105.6 ± 4.7 | 97.0 | 123.8 | 1.00 |
| `backhand-dist-musl-v0.18.0` | 172.0 ± 7.2 | 156.7 | 189.2 | 1.63 ± 0.10 |
| `backhand-dist-musl` | 171.6 ± 7.2 | 150.1 | 190.6 | 1.63 ± 0.10 |
| `squashfs-tools` | 195.2 ± 16.4 | 167.7 | 235.2 | 1.85 ± 0.18 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist` | 444.2 ± 4.4 | 436.6 | 453.2 | 2.20 ± 0.07 |
| `backhand-dist-musl-v0.18.0` | 885.2 ± 10.6 | 865.9 | 912.2 | 4.39 ± 0.14 |
| `backhand-dist-musl` | 491.0 ± 3.8 | 484.3 | 500.8 | 2.44 ± 0.08 |
| `squashfs-tools` | 201.5 ± 6.1 | 191.1 | 213.6 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist` | 1.411 ± 0.010 | 1.387 | 1.429 | 1.22 ± 0.04 |
| `backhand-dist-musl-v0.18.0` | 1.199 ± 0.005 | 1.189 | 1.209 | 1.03 ± 0.03 |
| `backhand-dist-musl` | 1.194 ± 0.005 | 1.184 | 1.206 | 1.03 ± 0.03 |
| `squashfs-tools` | 1.161 ± 0.034 | 1.089 | 1.249 | 1.00 |

## Heap Usage: `backhand/unsquashfs-master` vs `squashfs-tools/unsquashfs-4.6.1`
```
$ cargo +stable build -p backhand-cli --bins --locked --profile=dist --no-default-features --features xz --features gzip
```

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 41.7MB |
| `heaptrack unsquashfs -quiet -no-progress -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 75.7MB |

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 56.6MB |
| `heaptrack unsquashfs -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 114.4MB |
