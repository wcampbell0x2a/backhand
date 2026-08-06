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
| `backhand-dist-v0.24.1-musl` | 34.0 ± 1.7 | 30.7 | 39.6 | 1.31 ± 0.10 |
| `backhand-dist-musl` | 28.8 ± 1.2 | 26.7 | 31.3 | 1.11 ± 0.08 |
| `backhand-dist-musl-native` | 29.5 ± 1.3 | 27.0 | 32.5 | 1.13 ± 0.09 |
| `backhand-dist-gnu` | 26.6 ± 1.3 | 24.2 | 29.2 | 1.02 ± 0.08 |
| `backhand-dist-gnu-native` | 26.0 ± 1.6 | 21.9 | 29.6 | 1.00 |
| `squashfs-tools` | 57.5 ± 4.5 | 48.3 | 64.8 | 2.21 ± 0.22 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 35.0 ± 1.8 | 30.9 | 39.9 | 1.32 ± 0.09 |
| `backhand-dist-musl` | 30.0 ± 1.5 | 26.4 | 32.8 | 1.13 ± 0.08 |
| `backhand-dist-musl-native` | 29.4 ± 1.3 | 26.4 | 32.0 | 1.11 ± 0.08 |
| `backhand-dist-gnu` | 26.8 ± 1.5 | 24.2 | 31.4 | 1.01 ± 0.08 |
| `backhand-dist-gnu-native` | 26.5 ± 1.3 | 22.8 | 29.5 | 1.00 |
| `squashfs-tools` | 55.7 ± 5.4 | 43.4 | 68.2 | 2.10 ± 0.23 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 94.5 ± 2.6 | 90.3 | 98.7 | 1.37 ± 0.13 |
| `backhand-dist-musl` | 77.2 ± 1.7 | 74.0 | 81.1 | 1.12 ± 0.11 |
| `backhand-dist-musl-native` | 77.1 ± 1.9 | 74.6 | 80.8 | 1.12 ± 0.11 |
| `backhand-dist-gnu` | 72.0 ± 2.0 | 68.5 | 76.7 | 1.05 ± 0.10 |
| `backhand-dist-gnu-native` | 70.5 ± 1.6 | 67.7 | 74.1 | 1.02 ± 0.10 |
| `squashfs-tools` | 68.8 ± 6.5 | 58.4 | 80.5 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 108.7 ± 3.1 | 103.8 | 114.0 | 1.29 ± 0.06 |
| `backhand-dist-musl` | 88.3 ± 3.4 | 82.5 | 97.2 | 1.05 ± 0.06 |
| `backhand-dist-musl-native` | 87.3 ± 4.2 | 81.2 | 100.6 | 1.03 ± 0.07 |
| `backhand-dist-gnu` | 85.2 ± 4.5 | 80.4 | 99.4 | 1.01 ± 0.07 |
| `backhand-dist-gnu-native` | 84.4 ± 3.5 | 77.8 | 95.5 | 1.00 |
| `squashfs-tools` | 116.0 ± 6.7 | 99.1 | 128.3 | 1.37 ± 0.10 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 61.7 ± 3.5 | 56.7 | 69.2 | 1.31 ± 0.09 |
| `backhand-dist-musl` | 52.0 ± 2.3 | 47.8 | 58.6 | 1.10 ± 0.06 |
| `backhand-dist-musl-native` | 51.2 ± 1.9 | 47.1 | 55.1 | 1.09 ± 0.06 |
| `backhand-dist-gnu` | 47.2 ± 1.7 | 43.9 | 52.0 | 1.00 |
| `backhand-dist-gnu-native` | 47.4 ± 2.7 | 41.3 | 52.2 | 1.00 ± 0.07 |
| `squashfs-tools` | 90.5 ± 6.1 | 82.5 | 103.3 | 1.92 ± 0.15 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 135.5 ± 2.4 | 129.1 | 139.5 | 1.93 ± 0.08 |
| `backhand-dist-musl` | 129.0 ± 2.2 | 125.1 | 133.5 | 1.84 ± 0.07 |
| `backhand-dist-musl-native` | 130.7 ± 2.6 | 126.4 | 136.8 | 1.86 ± 0.08 |
| `backhand-dist-gnu` | 111.1 ± 2.7 | 107.4 | 117.3 | 1.58 ± 0.07 |
| `backhand-dist-gnu-native` | 110.8 ± 1.9 | 108.6 | 114.4 | 1.58 ± 0.06 |
| `squashfs-tools` | 70.2 ± 2.5 | 65.4 | 75.0 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 6.7 ± 0.7 | 4.2 | 7.8 | 1.32 ± 0.22 |
| `backhand-dist-musl` | 5.9 ± 0.6 | 4.1 | 6.9 | 1.16 ± 0.19 |
| `backhand-dist-musl-native` | 5.7 ± 0.7 | 3.4 | 6.8 | 1.12 ± 0.20 |
| `backhand-dist-gnu` | 5.5 ± 0.8 | 2.9 | 6.4 | 1.08 ± 0.21 |
| `backhand-dist-gnu-native` | 5.1 ± 0.7 | 2.5 | 5.8 | 1.00 |
| `squashfs-tools` | 7.0 ± 1.0 | 3.5 | 8.0 | 1.38 ± 0.27 |
### `airootfs.sfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 1.183 ± 0.049 | 1.150 | 1.318 | 1.26 ± 0.06 |
| `backhand-dist-musl` | 0.956 ± 0.018 | 0.930 | 0.990 | 1.02 ± 0.03 |
| `backhand-dist-musl-native` | 0.965 ± 0.019 | 0.944 | 1.006 | 1.03 ± 0.03 |
| `backhand-dist-gnu` | 0.955 ± 0.064 | 0.912 | 1.132 | 1.02 ± 0.07 |
| `backhand-dist-gnu-native` | 0.939 ± 0.021 | 0.909 | 0.973 | 1.00 |
| `squashfs-tools` | 1.249 ± 0.006 | 1.241 | 1.262 | 1.33 ± 0.03 |

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
