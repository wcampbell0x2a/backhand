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
| `backhand-dist-v0.15.0` | 50.2 ± 3.2 | 45.1 | 59.0 | 1.01 ± 0.10 |
| `backhand-dist` | 49.6 ± 3.5 | 43.1 | 58.2 | 1.00 |
| `backhand-dist-musl` | 78.2 ± 4.7 | 66.8 | 86.1 | 1.58 ± 0.15 |
| `squashfs-tools` | 113.1 ± 13.1 | 86.3 | 151.0 | 2.28 ± 0.31 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.15.0` | 50.8 ± 2.5 | 46.6 | 56.5 | 1.00 |
| `backhand-dist` | 51.3 ± 2.8 | 44.0 | 59.2 | 1.01 ± 0.08 |
| `backhand-dist-musl` | 77.2 ± 3.9 | 69.9 | 88.5 | 1.52 ± 0.11 |
| `squashfs-tools` | 114.0 ± 15.1 | 88.7 | 167.6 | 2.24 ± 0.32 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.15.0` | 257.2 ± 13.0 | 224.5 | 274.6 | 1.34 ± 0.10 |
| `backhand-dist` | 255.4 ± 14.2 | 218.3 | 278.2 | 1.33 ± 0.10 |
| `backhand-dist-musl` | 426.4 ± 28.8 | 364.6 | 463.7 | 2.23 ± 0.19 |
| `squashfs-tools` | 191.6 ± 10.3 | 173.6 | 214.1 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.15.0` | 227.0 ± 6.7 | 216.0 | 241.1 | 1.00 ± 0.04 |
| `backhand-dist` | 226.8 ± 6.3 | 217.3 | 244.2 | 1.00 |
| `backhand-dist-musl` | 361.8 ± 11.0 | 345.4 | 391.7 | 1.60 ± 0.07 |
| `squashfs-tools` | 288.2 ± 11.8 | 264.4 | 316.6 | 1.27 ± 0.06 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.15.0` | 105.6 ± 4.2 | 97.7 | 115.6 | 1.00 |
| `backhand-dist` | 105.9 ± 5.5 | 98.4 | 128.3 | 1.00 ± 0.07 |
| `backhand-dist-musl` | 172.2 ± 6.7 | 157.5 | 184.7 | 1.63 ± 0.09 |
| `squashfs-tools` | 189.7 ± 17.3 | 157.8 | 237.4 | 1.80 ± 0.18 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.15.0` | 503.4 ± 5.8 | 493.9 | 517.8 | 2.49 ± 0.08 |
| `backhand-dist` | 505.3 ± 7.4 | 490.1 | 519.4 | 2.50 ± 0.09 |
| `backhand-dist-musl` | 848.2 ± 8.2 | 833.2 | 872.2 | 4.19 ± 0.14 |
| `squashfs-tools` | 202.3 ± 6.5 | 188.9 | 214.9 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.15.0` | 1.435 ± 0.008 | 1.417 | 1.452 | 1.18 ± 0.01 |
| `backhand-dist` | 1.434 ± 0.009 | 1.417 | 1.455 | 1.18 ± 0.01 |
| `backhand-dist-musl` | 1.219 ± 0.006 | 1.210 | 1.245 | 1.00 |
| `squashfs-tools` | 1.838 ± 0.054 | 1.767 | 2.047 | 1.51 ± 0.05 |

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
