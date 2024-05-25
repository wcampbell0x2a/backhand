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
| `backhand-dist-v0.17.0` | 50.2 ± 3.1 | 43.7 | 58.3 | 1.02 ± 0.09 |
| `backhand-dist` | 49.5 ± 3.2 | 44.2 | 57.8 | 1.00 |
| `backhand-dist-musl` | 78.7 ± 5.1 | 66.0 | 90.2 | 1.59 ± 0.15 |
| `squashfs-tools` | 111.8 ± 11.9 | 84.2 | 133.1 | 2.26 ± 0.28 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.17.0` | 51.0 ± 3.3 | 44.1 | 58.8 | 1.01 ± 0.08 |
| `backhand-dist` | 50.3 ± 2.6 | 45.1 | 58.9 | 1.00 |
| `backhand-dist-musl` | 78.0 ± 4.3 | 71.3 | 89.8 | 1.55 ± 0.12 |
| `squashfs-tools` | 112.9 ± 16.7 | 90.1 | 159.0 | 2.24 ± 0.35 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.17.0` | 258.9 ± 12.5 | 228.4 | 275.5 | 1.35 ± 0.11 |
| `backhand-dist` | 252.8 ± 14.9 | 220.2 | 273.7 | 1.32 ± 0.12 |
| `backhand-dist-musl` | 429.0 ± 27.9 | 367.4 | 462.5 | 2.24 ± 0.20 |
| `squashfs-tools` | 191.7 ± 12.4 | 168.9 | 215.0 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.17.0` | 226.1 ± 7.9 | 214.7 | 240.6 | 1.00 |
| `backhand-dist` | 226.4 ± 6.4 | 216.6 | 240.0 | 1.00 ± 0.04 |
| `backhand-dist-musl` | 360.7 ± 10.8 | 342.2 | 384.5 | 1.60 ± 0.07 |
| `squashfs-tools` | 288.9 ± 11.9 | 269.6 | 320.4 | 1.28 ± 0.07 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.17.0` | 106.4 ± 4.3 | 97.9 | 117.6 | 1.00 ± 0.06 |
| `backhand-dist` | 106.0 ± 4.7 | 97.5 | 119.3 | 1.00 |
| `backhand-dist-musl` | 171.5 ± 6.3 | 151.2 | 185.2 | 1.62 ± 0.09 |
| `squashfs-tools` | 187.3 ± 15.6 | 160.7 | 223.4 | 1.77 ± 0.17 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.17.0` | 490.1 ± 6.8 | 477.9 | 506.7 | 2.43 ± 0.10 |
| `backhand-dist` | 511.3 ± 6.8 | 498.6 | 527.0 | 2.54 ± 0.10 |
| `backhand-dist-musl` | 870.0 ± 8.2 | 856.6 | 890.6 | 4.32 ± 0.17 |
| `squashfs-tools` | 201.4 ± 7.7 | 188.2 | 222.9 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.17.0` | 1.423 ± 0.009 | 1.406 | 1.451 | 1.20 ± 0.01 |
| `backhand-dist` | 1.392 ± 0.009 | 1.375 | 1.411 | 1.18 ± 0.01 |
| `backhand-dist-musl` | 1.181 ± 0.004 | 1.174 | 1.190 | 1.00 |
| `squashfs-tools` | 1.819 ± 0.042 | 1.706 | 1.908 | 1.54 ± 0.04 |

## Heap Usage: `backhand/unsquashfs-master` vs `squashfs-tools/unsquashfs-4.6.1`
```
$ cargo +stable build -p backhand-cli --bins --locked --profile=dist --no-default-features --features xz --features gzip-zune-inflate
```

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 36.5MB |
| `heaptrack unsquashfs -quiet -no-progress -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 75.7MB |

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 56.6MB |
| `heaptrack unsquashfs -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 114.4MB |
