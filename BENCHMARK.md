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
| `backhand-dist-v0.22.0-musl` | 41.6 ± 2.7 | 36.6 | 48.1 | 1.78 ± 0.15 |
| `backhand-dist-musl` | 28.5 ± 1.7 | 25.1 | 34.1 | 1.22 ± 0.10 |
| `backhand-dist-musl-native` | 28.5 ± 1.7 | 25.1 | 32.0 | 1.22 ± 0.10 |
| `backhand-dist-gnu` | 23.4 ± 1.4 | 20.7 | 27.1 | 1.00 ± 0.08 |
| `backhand-dist-gnu-native` | 23.4 ± 1.2 | 20.3 | 26.0 | 1.00 |
| `squashfs-tools` | 71.9 ± 8.4 | 50.5 | 86.5 | 3.07 ± 0.39 |
### `openwrt-22.03.2-ipq40xx-generic-netgear_ex6100v2-squashfs-factory.img`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 43.1 ± 2.9 | 36.4 | 48.9 | 1.82 ± 0.15 |
| `backhand-dist-musl` | 29.1 ± 1.5 | 26.5 | 32.5 | 1.23 ± 0.09 |
| `backhand-dist-musl-native` | 28.7 ± 1.5 | 25.8 | 34.7 | 1.21 ± 0.09 |
| `backhand-dist-gnu` | 23.8 ± 1.4 | 21.2 | 27.6 | 1.01 ± 0.07 |
| `backhand-dist-gnu-native` | 23.7 ± 1.1 | 20.8 | 25.6 | 1.00 |
| `squashfs-tools` | 64.3 ± 10.6 | 34.5 | 83.3 | 2.72 ± 0.46 |
### `870D97.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 229.2 ± 5.4 | 219.7 | 236.3 | 3.38 ± 0.11 |
| `backhand-dist-musl` | 84.6 ± 2.1 | 80.4 | 89.8 | 1.25 ± 0.04 |
| `backhand-dist-musl-native` | 83.4 ± 1.8 | 79.3 | 87.8 | 1.23 ± 0.04 |
| `backhand-dist-gnu` | 67.9 ± 1.6 | 65.1 | 71.5 | 1.00 ± 0.03 |
| `backhand-dist-gnu-native` | 67.9 ± 1.7 | 64.9 | 71.2 | 1.00 |
| `squashfs-tools` | 88.5 ± 12.0 | 67.8 | 111.3 | 1.30 ± 0.18 |
### `img-1571203182_vol-ubi_rootfs.ubifs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 144.9 ± 5.4 | 137.0 | 157.9 | 1.77 ± 0.12 |
| `backhand-dist-musl` | 98.4 ± 2.6 | 92.6 | 102.4 | 1.20 ± 0.08 |
| `backhand-dist-musl-native` | 97.0 ± 3.7 | 91.1 | 105.1 | 1.19 ± 0.08 |
| `backhand-dist-gnu` | 81.7 ± 4.7 | 76.6 | 95.0 | 1.00 |
| `backhand-dist-gnu-native` | 81.7 ± 3.9 | 77.0 | 91.6 | 1.00 ± 0.08 |
| `squashfs-tools` | 113.5 ± 6.6 | 95.6 | 126.6 | 1.39 ± 0.11 |
### `2611E3.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 76.7 ± 5.0 | 68.9 | 89.2 | 1.83 ± 0.16 |
| `backhand-dist-musl` | 52.0 ± 2.5 | 47.3 | 60.0 | 1.24 ± 0.09 |
| `backhand-dist-musl-native` | 51.6 ± 3.4 | 47.4 | 60.0 | 1.23 ± 0.11 |
| `backhand-dist-gnu` | 42.0 ± 2.5 | 38.1 | 47.7 | 1.00 |
| `backhand-dist-gnu-native` | 42.7 ± 2.8 | 37.8 | 49.7 | 1.02 ± 0.09 |
| `squashfs-tools` | 109.6 ± 9.8 | 88.1 | 123.5 | 2.61 ± 0.28 |
### `Plexamp-4.6.1.AppImage`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 288.4 ± 1.6 | 286.2 | 291.7 | 3.74 ± 0.37 |
| `backhand-dist-musl` | 123.3 ± 1.7 | 120.9 | 127.4 | 1.60 ± 0.16 |
| `backhand-dist-musl-native` | 122.7 ± 1.3 | 120.5 | 125.1 | 1.59 ± 0.16 |
| `backhand-dist-gnu` | 113.0 ± 2.7 | 109.3 | 117.7 | 1.47 ± 0.15 |
| `backhand-dist-gnu-native` | 109.9 ± 2.3 | 106.6 | 115.8 | 1.43 ± 0.15 |
| `squashfs-tools` | 77.1 ± 7.7 | 66.0 | 88.9 | 1.00 |
### `crates-io.squashfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 351.7 ± 1.6 | 349.3 | 354.4 | 1.00 ± 0.01 |
| `backhand-dist-musl` | 350.6 ± 2.1 | 347.9 | 354.6 | 1.00 |
| `backhand-dist-musl-native` | 355.3 ± 4.0 | 350.8 | 364.4 | 1.01 ± 0.01 |
| `backhand-dist-gnu` | 403.7 ± 4.0 | 398.0 | 408.7 | 1.15 ± 0.01 |
| `backhand-dist-gnu-native` | 412.5 ± 4.1 | 404.6 | 418.9 | 1.18 ± 0.01 |
| `squashfs-tools` | 754.3 ± 12.6 | 734.8 | 771.6 | 2.15 ± 0.04 |
### `airootfs.sfs`
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `backhand-dist-v0.22.0-musl` | 2.8 ± 0.2 | 2.0 | 3.6 | 1.02 ± 0.13 |
| `backhand-dist-musl` | 3.2 ± 0.3 | 2.0 | 4.1 | 1.17 ± 0.17 |
| `backhand-dist-musl-native` | 3.1 ± 0.2 | 1.9 | 3.5 | 1.15 ± 0.14 |
| `backhand-dist-gnu` | 2.7 ± 0.3 | 1.7 | 3.6 | 1.00 |
| `backhand-dist-gnu-native` | 3.2 ± 0.3 | 1.8 | 3.8 | 1.20 ± 0.17 |
| `squashfs-tools` | 3.4 ± 0.2 | 2.0 | 3.9 | 1.25 ± 0.16 |

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
