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
Architecture:                x86_64
  CPU op-mode(s):            32-bit, 64-bit
  Address sizes:             48 bits physical, 48 bits virtual
  Byte Order:                Little Endian
CPU(s):                      16
  On-line CPU(s) list:       0-15
Vendor ID:                   AuthenticAMD
  Model name:                AMD Ryzen 7 9800X3D 8-Core Processor
    CPU family:              26
    Model:                   68
    Thread(s) per core:      2
    Core(s) per socket:      8
    Socket(s):               1
    Stepping:                0
    Frequency boost:         enabled
    CPU(s) scaling MHz:      72%
    CPU max MHz:             5271.6221
    CPU min MHz:             603.3790
    BogoMIPS:                9399.97
    Flags:                   fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ht syscall nx mmxext fxsr_opt pdpe1gb rdtscp lm constant_tsc rep_good amd_lbr_v2 nopl xtopology nonstop_tsc cpuid extd_apicid aperfmperf rapl pn
                             i pclmulqdq monitor ssse3 fma cx16 sse4_1 sse4_2 movbe popcnt aes xsave avx f16c rdrand lahf_lm cmp_legacy svm extapic cr8_legacy abm sse4a misalignsse 3dnowprefetch osvw ibs skinit wdt tce topoext perfctr_core perfctr_nb bpext perfctr_llc mw
                             aitx cpb cat_l3 cdp_l3 hw_pstate ssbd mba perfmon_v2 ibrs ibpb stibp ibrs_enhanced vmmcall fsgsbase tsc_adjust bmi1 avx2 smep bmi2 erms invpcid cqm rdt_a avx512f avx512dq rdseed adx smap avx512ifma clflushopt clwb avx512cd sha_ni avx512bw avx
                             512vl xsaveopt xsavec xgetbv1 xsaves cqm_llc cqm_occup_llc cqm_mbm_total cqm_mbm_local user_shstk avx_vnni avx512_bf16 clzero irperf xsaveerptr rdpru wbnoinvd cppc arat npt lbrv svm_lock nrip_save tsc_scale vmcb_clean flushbyasid decodeassist
                             s pausefilter pfthreshold avic v_vmsave_vmload vgif x2avic v_spec_ctrl vnmi avx512vbmi umip pku ospke avx512_vbmi2 gfni vaes vpclmulqdq avx512_vnni avx512_bitalg avx512_vpopcntdq rdpid bus_lock_detect movdiri movdir64b overflow_recov succor s
                             mca fsrm avx512_vp2intersect flush_l1d amd_lbr_pmc_freeze
Virtualization features:
  Virtualization:            AMD-V
Caches (sum of all):
  L1d:                       384 KiB (8 instances)
  L1i:                       256 KiB (8 instances)
  L2:                        8 MiB (8 instances)
  L3:                        96 MiB (1 instance)
NUMA:
  NUMA node(s):              1
  NUMA node0 CPU(s):         0-15
Vulnerabilities:
  Gather data sampling:      Not affected
  Ghostwrite:                Not affected
  Indirect target selection: Not affected
  Itlb multihit:             Not affected
  L1tf:                      Not affected
  Mds:                       Not affected
  Meltdown:                  Not affected
  Mmio stale data:           Not affected
  Reg file data sampling:    Not affected
  Retbleed:                  Not affected
  Spec rstack overflow:      Mitigation; IBPB on VMEXIT only
  Spec store bypass:         Mitigation; Speculative Store Bypass disabled via prctl
  Spectre v1:                Mitigation; usercopy/swapgs barriers and __user pointer sanitization
  Spectre v2:                Mitigation; Enhanced / Automatic IBRS; IBPB conditional; STIBP always-on; PBRSB-eIBRS Not affected; BHI Not affected
  Srbds:                     Not affected
  Tsx async abort:           Not affected
```

</details>

This uses the latest `dl` binary from https://github.com/wcampbell0x2a/test-assets-ureq.

```
$ ./bench.bash
```

## Wall time: `backhand/unsquashfs` vs `squashfs-tools/unsquashfs-4.6.1`
### `openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 34.2 ± 2.0 | 30.7 | 37.7 | 1.20 ± 0.10 |
| `backhand-dist-musl` | 34.9 ± 2.0 | 30.6 | 39.3 | 1.22 ± 0.11 |
| `backhand-dist-musl-native` | 34.1 ± 2.0 | 30.9 | 39.1 | 1.19 ± 0.10 |
| `backhand-dist-gnu` | 29.3 ± 1.9 | 25.5 | 32.7 | 1.03 ± 0.09 |
| `backhand-dist-gnu-native` | 28.6 ± 1.9 | 24.5 | 31.4 | 1.00 |
| `squashfs-tools` | 49.5 ± 5.8 | 39.3 | 63.0 | 1.73 ± 0.23 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 39.2 ± 13.5 | 32.2 | 78.7 | 1.34 ± 0.47 |
| `backhand-dist-musl` | 34.6 ± 1.7 | 31.7 | 38.2 | 1.18 ± 0.08 |
| `backhand-dist-musl-native` | 34.0 ± 2.3 | 28.8 | 38.3 | 1.16 ± 0.09 |
| `backhand-dist-gnu` | 29.3 ± 1.3 | 26.3 | 31.3 | 1.00 |
| `backhand-dist-gnu-native` | 29.5 ± 1.4 | 26.7 | 31.7 | 1.01 ± 0.07 |
| `squashfs-tools` | 51.9 ± 4.9 | 41.5 | 60.5 | 1.77 ± 0.18 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 93.5 ± 2.8 | 87.7 | 97.3 | 1.30 ± 0.10 |
| `backhand-dist-musl` | 95.6 ± 9.9 | 90.7 | 134.7 | 1.33 ± 0.17 |
| `backhand-dist-musl-native` | 95.2 ± 12.2 | 87.0 | 137.0 | 1.33 ± 0.20 |
| `backhand-dist-gnu` | 75.9 ± 3.1 | 71.2 | 82.5 | 1.06 ± 0.09 |
| `backhand-dist-gnu-native` | 79.2 ± 11.1 | 72.7 | 121.9 | 1.10 ± 0.17 |
| `squashfs-tools` | 71.8 ± 5.2 | 63.7 | 82.9 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 105.6 ± 3.8 | 99.8 | 115.7 | 1.18 ± 0.06 |
| `backhand-dist-musl` | 109.8 ± 11.6 | 99.1 | 149.0 | 1.23 ± 0.14 |
| `backhand-dist-musl-native` | 112.0 ± 16.5 | 99.6 | 154.8 | 1.25 ± 0.19 |
| `backhand-dist-gnu` | 89.5 ± 3.4 | 84.5 | 97.6 | 1.00 |
| `backhand-dist-gnu-native` | 90.4 ± 3.8 | 83.4 | 96.7 | 1.01 ± 0.06 |
| `squashfs-tools` | 112.3 ± 7.2 | 101.5 | 130.6 | 1.25 ± 0.09 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 64.4 ± 13.0 | 55.1 | 105.3 | 1.17 ± 0.36 |
| `backhand-dist-musl` | 62.4 ± 11.0 | 55.3 | 109.1 | 1.13 ± 0.33 |
| `backhand-dist-musl-native` | 61.5 ± 9.2 | 55.3 | 102.5 | 1.12 ± 0.31 |
| `backhand-dist-gnu` | 55.2 ± 12.9 | 44.5 | 95.3 | 1.00 ± 0.33 |
| `backhand-dist-gnu-native` | 55.1 ± 12.9 | 46.8 | 97.5 | 1.00 |
| `squashfs-tools` | 86.3 ± 7.6 | 76.5 | 107.0 | 1.57 ± 0.39 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 163.5 ± 15.1 | 153.1 | 195.9 | 1.94 ± 0.31 |
| `backhand-dist-musl` | 157.7 ± 7.7 | 151.0 | 181.0 | 1.88 ± 0.26 |
| `backhand-dist-musl-native` | 159.3 ± 14.1 | 150.9 | 202.9 | 1.89 ± 0.30 |
| `backhand-dist-gnu` | 141.8 ± 11.1 | 133.0 | 174.4 | 1.69 ± 0.25 |
| `backhand-dist-gnu-native` | 142.7 ± 11.3 | 133.8 | 176.5 | 1.70 ± 0.26 |
| `squashfs-tools` | 84.1 ± 10.8 | 74.2 | 121.0 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 5.9 ± 0.9 | 3.7 | 6.8 | 1.00 |
| `backhand-dist-musl` | 6.9 ± 0.8 | 5.0 | 7.8 | 1.18 ± 0.22 |
| `backhand-dist-musl-native` | 6.1 ± 0.7 | 3.9 | 7.0 | 1.04 ± 0.20 |
| `backhand-dist-gnu` | 7.6 ± 1.1 | 5.1 | 8.8 | 1.30 ± 0.28 |
| `backhand-dist-gnu-native` | 7.1 ± 1.2 | 4.6 | 8.9 | 1.21 ± 0.27 |
| `squashfs-tools` | 8.4 ± 6.8 | 3.1 | 52.3 | 1.43 ± 1.19 |
### `airootfs.sfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 1.219 ± 0.021 | 1.178 | 1.259 | 1.17 ± 0.03 |
| `backhand-dist-musl` | 1.226 ± 0.023 | 1.183 | 1.266 | 1.18 ± 0.03 |
| `backhand-dist-musl-native` | 1.211 ± 0.021 | 1.172 | 1.247 | 1.16 ± 0.03 |
| `backhand-dist-gnu` | 1.043 ± 0.019 | 1.006 | 1.067 | 1.00 |
| `backhand-dist-gnu-native` | 1.106 ± 0.099 | 1.003 | 1.260 | 1.06 ± 0.10 |
| `squashfs-tools` | 1.279 ± 0.016 | 1.255 | 1.309 | 1.23 ± 0.03 |

## Heap Usage: `backhand/unsquashfs` vs `squashfs-tools/unsquashfs-4.6.1`
```
$ cargo +stable build -p backhand-cli --bins --locked --profile=dist
```

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 46.3MB |
| `heaptrack unsquashfs -quiet -no-progress -d $(mktemp -d) backhand-test/test-assets/test_re815_xev160/870D97.squashfs` | 79.2MB |

| Command | Peak Heap Memory Consumption |
| :------ | ---------------------------: |
| `heaptrack ./target/dist/unsquashfs-backhand --quiet -f -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 63.8MB |
| `heaptrack unsquashfs -d $(mktemp -d) backhand-test/test-assets/test_tplink_ax1800/img-1571203182_vol-ubi_rootfs.ubifs` | 120.4MB |
