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

## Wall time: `backhand/unsquashfs` vs `squashfs-tools/unsquashfs-4.7.5`
### `openwrt-22.03.2-ath79-generic-tplink_archer-a7-v5-squashfs-factory.bin`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 32.8 ± 1.8 | 29.2 | 37.5 | 1.24 ± 0.09 |
| `backhand-dist-musl` | 28.3 ± 1.2 | 25.6 | 30.5 | 1.07 ± 0.07 |
| `backhand-dist-musl-native` | 29.1 ± 1.1 | 27.3 | 31.4 | 1.10 ± 0.07 |
| `backhand-dist-gnu` | 27.3 ± 1.2 | 25.4 | 30.7 | 1.03 ± 0.07 |
| `backhand-dist-gnu-native` | 26.4 ± 1.2 | 24.5 | 28.7 | 1.00 |
| `squashfs-tools` | 46.2 ± 5.2 | 36.6 | 56.9 | 1.75 ± 0.21 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 33.5 ± 2.0 | 30.0 | 38.6 | 1.26 ± 0.09 |
| `backhand-dist-musl` | 28.5 ± 1.4 | 26.5 | 31.9 | 1.07 ± 0.07 |
| `backhand-dist-musl-native` | 28.8 ± 1.5 | 26.4 | 34.3 | 1.09 ± 0.07 |
| `backhand-dist-gnu` | 27.1 ± 1.0 | 25.1 | 29.9 | 1.02 ± 0.06 |
| `backhand-dist-gnu-native` | 26.5 ± 1.1 | 24.4 | 29.3 | 1.00 |
| `squashfs-tools` | 47.4 ± 5.6 | 38.8 | 59.1 | 1.79 ± 0.22 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 92.0 ± 2.2 | 88.9 | 96.3 | 1.34 ± 0.10 |
| `backhand-dist-musl` | 76.6 ± 2.2 | 73.2 | 79.6 | 1.11 ± 0.08 |
| `backhand-dist-musl-native` | 75.5 ± 1.1 | 73.3 | 77.8 | 1.10 ± 0.08 |
| `backhand-dist-gnu` | 70.7 ± 2.0 | 67.1 | 74.6 | 1.03 ± 0.08 |
| `backhand-dist-gnu-native` | 70.0 ± 1.3 | 67.1 | 72.8 | 1.02 ± 0.07 |
| `squashfs-tools` | 68.9 ± 4.8 | 61.6 | 79.2 | 1.00 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 106.6 ± 3.9 | 100.5 | 114.3 | 1.27 ± 0.07 |
| `backhand-dist-musl` | 84.7 ± 1.6 | 81.8 | 87.4 | 1.01 ± 0.05 |
| `backhand-dist-musl-native` | 86.3 ± 3.0 | 82.5 | 92.1 | 1.03 ± 0.06 |
| `backhand-dist-gnu` | 84.6 ± 3.2 | 81.0 | 91.5 | 1.01 ± 0.06 |
| `backhand-dist-gnu-native` | 83.7 ± 3.7 | 79.5 | 93.2 | 1.00 |
| `squashfs-tools` | 98.7 ± 4.0 | 91.8 | 104.0 | 1.18 ± 0.07 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 60.4 ± 2.9 | 55.9 | 67.4 | 1.28 ± 0.10 |
| `backhand-dist-musl` | 50.4 ± 2.1 | 46.0 | 56.2 | 1.07 ± 0.08 |
| `backhand-dist-musl-native` | 50.4 ± 1.3 | 48.2 | 53.4 | 1.07 ± 0.07 |
| `backhand-dist-gnu` | 47.2 ± 1.8 | 43.7 | 51.5 | 1.00 ± 0.07 |
| `backhand-dist-gnu-native` | 47.1 ± 2.9 | 44.1 | 55.6 | 1.00 |
| `squashfs-tools` | 72.0 ± 4.3 | 63.4 | 80.9 | 1.53 ± 0.13 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 143.5 ± 2.4 | 140.7 | 148.0 | 1.83 ± 0.16 |
| `backhand-dist-musl` | 150.5 ± 2.0 | 146.7 | 154.2 | 1.91 ± 0.17 |
| `backhand-dist-musl-native` | 138.1 ± 2.1 | 134.5 | 142.6 | 1.76 ± 0.16 |
| `backhand-dist-gnu` | 131.8 ± 1.6 | 129.5 | 134.6 | 1.68 ± 0.15 |
| `backhand-dist-gnu-native` | 119.9 ± 2.4 | 116.3 | 125.3 | 1.53 ± 0.14 |
| `squashfs-tools` | 78.6 ± 7.0 | 70.1 | 92.2 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 5.7 ± 0.2 | 5.4 | 6.4 | 1.04 ± 0.04 |
| `backhand-dist-musl` | 5.8 ± 0.1 | 5.5 | 6.1 | 1.04 ± 0.04 |
| `backhand-dist-musl-native` | 5.5 ± 0.1 | 5.3 | 5.9 | 1.00 |
| `backhand-dist-gnu` | 6.4 ± 0.1 | 6.1 | 6.7 | 1.16 ± 0.03 |
| `backhand-dist-gnu-native` | 5.9 ± 0.1 | 5.6 | 6.3 | 1.07 ± 0.03 |
| `squashfs-tools` | 7.8 ± 0.1 | 7.3 | 8.0 | 1.41 ± 0.04 |
### `airootfs.sfs`
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.24.1-musl` | 1.172 ± 0.024 | 1.137 | 1.227 | 1.28 ± 0.04 |
| `backhand-dist-musl` | 0.946 ± 0.014 | 0.928 | 0.972 | 1.03 ± 0.03 |
| `backhand-dist-musl-native` | 0.949 ± 0.015 | 0.927 | 0.973 | 1.03 ± 0.03 |
| `backhand-dist-gnu` | 0.958 ± 0.088 | 0.902 | 1.129 | 1.04 ± 0.10 |
| `backhand-dist-gnu-native` | 0.918 ± 0.018 | 0.894 | 0.957 | 1.00 |
| `squashfs-tools` | 1.249 ± 0.005 | 1.237 | 1.254 | 1.36 ± 0.03 |

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
