# How much of the ARM plan runs under HVF

*Name provisional (`arm-plan-under-hvf`), per the rule that a lane ships a provisional name and says
so. Measured 2026-10-21 (UTC) on patagonia, QEMU 11.1.2, `accel=hvf`, an Apple M-series core. The
logs, probe sources and seL4 build recipe are in
[bench/patagonia-hvf-2026-10-21/](../bench/patagonia-hvf-2026-10-21/README.md).*

Before any ARM board arrives, three questions decide what this Mac can carry: a seL4 comparison on
identical silicon, development against modern CPU features, and SMMU work.

| question | answer |
|---|---|
| Does seL4 boot under HVF, and can its IPC be compared with nife's on the same core? | **Yes, with one patched line and a changed instrument.** Unmodified seL4 refuses the guest's 8-bit ASIDs, and unmodified sel4bench refuses the PMU it finds. A loop-averaged generic-timer measurement is stable to about 10%. |
| Which modern features does HVF expose to a guest? | **Most of Armv8.6, minus most of what the first-machine survey wants.** PAC, BTI, LSE, PAN3 and SB are present. MTE, SVE, SME, VHE, hardware access and dirty flags, 16-bit ASIDs and an ITS alongside EL2 are not. |
| Does `virt,iommu=smmuv3` start with `accel=hvf`, and can a guest program it? | **Yes.** A bare probe reads `IDR0`. nife's own SMMUv3 driver passes the whole HVF leg, IOMMU fault tests included. |

## 1. seL4 under HVF, and the IPC comparison

### What boots

seL4's `qemu-arm-virt` platform was built in the Foundation's image at the sel4bench manifest the
performance page names (`11d0fc2c`), with `QEMU_GIC_VERSION=3` and the device tree HVF's own machine
dumps.

- Unmodified seL4 refuses to start: `ERROR: kernel requires 16-bit ASIDs`. HVF reports
  `ID_AA64MMFR0_EL1.ASIDBits` as 0, which means 8 bits. seL4 skips the check only in hypervisor mode.
- With the check removed (one `if`, nothing else), and `ARM_CPU=cortex-a53` so that seL4 assumes
  the 40-bit physical range HVF gives, sel4test passes: 122 tests, 54 disabled by its own
  configuration. With 8-bit hardware ASIDs, seL4's 16-bit ASIDs alias above 255 address spaces.
  sel4test and sel4bench create far fewer, so the runs are sound, but the patched kernel is not a
  correct seL4 in general.
- The cortex-a57 configuration is wrong for this guest, not HVF. It sets a 44-bit physical
  range, hands user space a frame above 2^40, and takes an address-size fault (ESR `0x96000143`).
- Hypervisor mode does not work. With `virtualization=on`, HVF really does start the guest at
  EL2, and seL4's kernel boots there. Its root task dies on its first exception, because HVF's guest
  EL2 ignores `HCR_EL2.TGE`: an EL0 `svc` is taken to EL1, with or without `E2H`. The same probe
  under TCG is taken to EL2. So a guest hypervisor can host EL1 guests here, but no kernel can run at
  EL2 with its programs directly beneath it. That bounds what
  [milestone 926 (Linux as a guest of nife)](../design/roadmap/0926-linux-runs-as-a-guest-of-nife.md)
  can try on this machine.

### Why sel4bench's own method cannot run, measured

The guest has a PMUv3 (`ID_AA64DFR0_EL1.PMUVer` 1), but QEMU synthesizes it. `PMCCNTR_EL0` advances
at exactly 128 times `CNTVCT_EL0` (307,221,760 counts over 2,400,205 ticks; nife's own
`cycles_per_tick` probe prints 128.05). Every read is a trapped exit: 10,000 reads take 190,436 ticks
(about 790 ns each), against 1,907 ticks for 10,000 reads of `CNTVCT_EL0`. So the "cycle counter" is
the 24 MHz generic timer at 128 times the grain and about a hundred times the cost.

sel4bench sees exactly that. Its overhead calibration reads 2,688 to 2,944 counts for an empty
counter pair, never the same twice, and the run stops: `Benchmarking overhead of a call is not
stable! Cannot continue`. Built with `AllowUnstableOverhead`, it prints every IPC figure as 128 or
256 counts, one or two timer ticks after subtracting the minimum overhead. Those are not
measurements.

### The loop-averaged instrument

sel4bench's ipc app was changed in exactly two ways. Its counter reads `CNTVCT_EL0`, which the
kernel exports to EL0 with `KernelArmExportVCNTUser`. Its warm-up loop runs 10,000 passes, stamped
before the first and after the last. Kernel, fastpath, threads, priorities, address spaces and
message lengths are sel4bench's. A "call" sample then spans 10,000 calls and 9,999 replies, and a
"reply" sample the reverse, so their sum over 19,999 is one call plus one reply.

The split into call and reply is not recoverable this way. The two samples come from separate runs,
and their difference is smaller than the run-to-run spread. Only the round trip is reported.

### The two kernels on one core

Eight boots of each, five back to back and three alternating seL4 with nife, at host loads of 3 to
8. nife's figure is `ipc_rtt_el0` from `script/bench --real --release`, on `main` at `f87c9b5c4`.

| | median of boots | range of boot medians |
|---|---|---|
| seL4, `Call` + `ReplyRecv`, different address spaces, no message words | **96 ns** | 90 to 99 ns |
| seL4, the same with FPU context | 118 ns | 110 to 121 ns |
| seL4, the same with 10 message words | 146 ns | 136 to 149 ns |
| nife, `ipc_rtt_el0`, two address spaces | **442 ns** | 395 to 444 ns |
| nife, `null_syscall`, for scale | 30 ns | 27 to 33 ns |

On identical silicon, nife's EL0 round trip costs about 4.6 times seL4's. Even if the two extra
traps in nife's shape cost nothing, at about 30 ns each, the ratio stays near 4. It is the
same-silicon figure [benchmarks.md](benchmarks.md#against-sel4s-published-cycles) quotes.

What is not apples to apples:

- The shapes differ. seL4 issues two syscalls per round trip (`Call`, `ReplyRecv`) over one
  endpoint and takes its fastpath. `ipc_rtt_el0` issues four (`SEND`, `RECEIVE`, twice) over two
  endpoints, and no EL0 bench times nife's `CALL`/`REPLY` path.
- Both run as guests. Each user address-space switch also changes stage-2 state the host owns.
  Both kernels pay that, but it is not what seL4's published figures include.
- seL4 is patched, in the ASID check, and configured for a Cortex-A53 it is not running on. That
  configuration selects only the physical-address width and the I-cache model.
- The instrument is not sel4bench's. A 41.7 ns counter is averaged over 10,000 passes, which
  includes loop overhead and hides the per-call distribution sel4bench was built to report. nife's
  bench uses the same method, which is why the two compare.
- Memory size differs: seL4 had 1 GiB, nife's runner 256 MiB. Neither loop touches more than a
  few pages.

## 2. The CPU features a guest sees

Read by a bare probe at EL1, and identical at EL2 (`virtualization=on`). `MIDR_EL1` is
`0x610f0000`: Apple's implementer code, with the part number masked to 0.

| register | value |
|---|---|
| `ID_AA64ISAR0_EL1` | `0x0221100110212120` |
| `ID_AA64ISAR1_EL1` | `0x0010111110211402` |
| `ID_AA64ISAR2_EL1` | `0` |
| `ID_AA64PFR0_EL1` | `0x1101000010110011` |
| `ID_AA64PFR1_EL1` | `0x0000000100000001` |
| `ID_AA64ZFR0_EL1`, `ID_AA64SMFR0_EL1` | `0`, `0` |
| `ID_AA64MMFR0_EL1` | `0x000010000f100002` |
| `ID_AA64MMFR1_EL1` | `0x0000100011312000` |
| `ID_AA64MMFR2_EL1` | `0x1001001102001011` |
| `ID_AA64MMFR3_EL1` | `0` |
| `ID_AA64DFR0_EL1` | `0x0000000010305106` |

Present: LSE atomics (ISAR0.Atomic 2) and LSE2 (MMFR2.AT). PAC with an implementation-defined
algorithm and FPAC (ISAR1.API 4, GPI 1). BTI (PFR1.BT 1). PAN3 (MMFR1.PAN 3), UAO, E0PD. CRC32, AES,
SHA1/2/3 with SHA-512, RDM, DotProd, FHM, FlagM2, TLB range instructions, JSCVT, FCMA, LRCPC2,
FRINTTS, SB, SPECRES, BF16, I8MM, DPB2. FP16, RAS, DIT, CSV2 and CSV3. HPDS2, XNX, AFP, CnP, IESB,
TTL, IDS. 4 KiB and 16 KiB granules.

Absent:

- No MTE, SVE or SME: PFR1.MTE, PFR0.SVE and PFR1.SME are all 0, and QEMU refuses `mte=on` under
  HVF.
- No VHE: MMFR1.VH is 0. At EL2 the `E2H` bit sticks, but `VBAR_EL1` is not redirected to
  `VBAR_EL2`, and `TGE` does nothing (section 1).
- No hardware access or dirty flags (MMFR1.HAFDBS 0), so software manages the access flag.
- 8-bit ASIDs and VMIDs, and a 40-bit physical address range (MMFR0.PARange 2).
- No RNDR, so nife's `rndrrs` entropy test skips here.
- All of ISAR2 is 0: no WFxT, MOPS, CLRBHB or QARMA3.
- No 64 KiB granule, AMU, MPAM, Secure EL2, FGT, ECV or HCX.
- PFR0.EL2 reads 0 even when the guest is at EL2. PFR0.GIC reads 0 even though the GICv3 system
  registers work ([machine_discovery's gic.rs](../crates/machine_discovery/src/gic.rs) has that
  quirk).

The ITS is an either-or. QEMU's default under HVF is Apple's hardware vGIC: a GICv3 with no LPIs
(`GICD_TYPER.LPIS` 0) and no ITS (`its=on` is refused). With `-accel hvf,kernel-irqchip=off`, QEMU
emulates the GICv3 itself and an ITS appears (`GITS_TYPER` `0x1f0001efb1`). That machine cannot
have EL2: "HVF does not support GICv3 emulation". No combination gives a GICv4.

## 3. SMMUv3 with HVF

`virt,accel=hvf,gic-version=3,iommu=smmuv3` starts. A bare guest at EL1 reads the SMMU's page 0
directly:

- `IDR0` `0x0d44101b`: stage 1 and stage 2, AArch64 tables, coherent walks, 16-bit ASIDs and VMIDs,
  and a two-level stream table. It has no ATS, PRI, MSI or hardware access and dirty updates.
- `IDR1` `0x02730010` and `IDR5` `0x74`.
- `AIDR` 1, which is SMMUv3.1.

nife's own driver (`kernel/src/arch/aarch64/iommu.rs`) programs it end to end on this machine.
`script/test --hvf` passed 140 kernel tests and 387 system tests, with 14 skipped. Among them are
`the_iommu_faults_a_dma_that_escapes_the_domain`, an e1000e DHCP lease behind the IOMMU, and the
NVMe and display DMA refusals.

The SMMU is QEMU's model in either case, emulated in the host process, so this proves the driver
against the architecture rather than against silicon. Errata, a real stream-ID map from an IORT, and
the timing of real event and command queues all wait on a board.

## BUGS

- The seL4 round trip is from a patched kernel, built with the Foundation's `latest` image (an amd64
  image run under emulation here) rather than a pinned digest.
- sel4test ran at sel4test-manifest `master` (2026-10-20), not at a commit tied to `11d0fc2c`.
- No EL0 bench times nife's `CALL`/`REPLY` path, so the closest structural comparison is still
  missing.
- All of it is one core of one machine, under a desktop OS at load averages of 3 to 8.
