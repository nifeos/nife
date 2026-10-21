# patagonia under HVF, 2026-10-21 (UTC): seL4, the guest's ID registers, and the SMMUv3

The evidence behind [notes/arm-plan-under-hvf.md](../../notes/arm-plan-under-hvf.md), which carries
the answers. Every run here is QEMU 11.1.2 with `accel=hvf` on an Apple M-series core, macOS 26.6.
Unit-specific details are kept off-tree by the maintainers.

## The machine string

seL4 and nife ran on the same one:

```
qemu-system-aarch64 -machine virt,accel=hvf,gic-version=3,iommu=smmuv3[,secure=off] -cpu host -smp 1
```

`secure=off` is `virt`'s default, so the two strings start the same machine. They differ only in
memory: seL4 had `-m 1024` and nife's runner gives `-m 256M`. Neither IPC loop touches more than a
few pages.

## guest/: a bare-metal probe

Freestanding programs loaded with `-kernel` that print to the PL011 and touch nothing else. Each one
builds with the host's clang and the toolchain's `rust-lld`:

```sh
clang --target=aarch64-none-elf -c idregs.S -o idregs.o
rust-lld -flavor gnu -T link.ld idregs.o -o idregs.elf
clang --target=aarch64-none-elf -march=armv8.1-a -O2 -ffreestanding -fno-builtin -c probe.c -o probe.o
clang --target=aarch64-none-elf -c start.S -o start.o
rust-lld -flavor gnu -T link.ld start.o probe.o -o probe.elf
```

| program | reads | log |
|---|---|---|
| `idregs.S` | `CurrentEL`, `MIDR_EL1`, the `ID_AA64*` registers, `CNTFRQ_EL0`, SMMU `IDR0`, `IDR1`, `IDR5`, `AIDR` | `idregs-el1.log`, `idregs-el2.log` |
| `probe.c` | at EL2 (if started there): `HCR_EL2.E2H`; then at EL1: the PMU cycle counter's rate and read cost against `CNTVCT_EL0`, `GICD_TYPER`, `GITS_TYPER`, `GICR_TYPER` | `probe-hvf-vgic.log`, `probe-hvf-qemu-gic.log` |
| `tge.S`, `tge-e2h.S` | at EL2: set `HCR_EL2.TGE` (and `E2H`), `eret` to EL0, `svc`, and print which vector caught it | `tge.log` |

`refusals.log` holds QEMU's own refusals: an ITS or MTE with the hardware vGIC, and EL2 with
QEMU's GIC.

## sel4/: seL4 for qemu-arm-virt, built in the Foundation's image

Everything was built in `docker.io/trustworthysystems/sel4:latest` under podman, at:

- sel4bench-manifest `11d0fc2c`, the manifest sel4.systems/performance.html names. seL4 `d97d707b`,
  sel4bench `18f9d5f0`. The checkout is the one `bench/xenon-sel4bench/build.sh` syncs.
- sel4test-manifest `master` as of 2026-10-20 (`3a84d147`), seL4 `7dd984a7`. sel4test does not
  publish against 11d0fc2c, and here it answers only whether the platform boots.

Every build is `-DPLATFORM=qemu-arm-virt -DAARCH64=TRUE -DQEMU_GIC_VERSION=3` with
`-DQEMU_DTB=` set to the device tree that HVF's own machine dumps. The dump is the line above with
`,dumpdtb=hvf-virt.dtb` (or `hvf-virt-el2.dtb` with `virtualization=on`). That way seL4 describes the
machine it boots on, not the TCG one its configure step would dump.

`build.sh` is the first build, with `ARM_CPU=cortex-a57`. `build4.sh` is the last, with
`cortex-a53`. Between them, three edits were made by hand, in this order:

1. **The 16-bit ASID check.** In each non-hyp copy of `kernel/src/arch/arm/kernel/boot.c`:
   `if (!config_set(CONFIG_ARM_HYPERVISOR_SUPPORT) && !check16BitASID())` became
   `if (0 && ...)`. Nothing else in the kernel was changed.
2. **The loop-averaged IPC app.** `loop-patch.sh` was run in the copy that builds the loop variant.
   It makes sel4bench's ipc app read `CNTVCT_EL0` and stamp before the first and after the last of
   10,000 passes. The kernel gets `-DKernelArmExportVCNTUser=ON` so EL0 may read the counter.
3. **IPC only.** `-DHARDWARE=OFF -DFAULT=OFF -DIRQUSER=OFF -DSCHED=OFF -DSIGNAL=OFF -DMAPPING=OFF
   -DSYNC=OFF -DAllowUnstableOverhead=ON`. The IRQ benchmark aborts the whole run on this platform
   ("Platform does not support ltimers") before the JSON is printed.

`boot.sh` boots one image on the machine string and stops at a verdict or a bound. `analyze.py`
reads the JSON and does the arithmetic the note describes.

## logs/

The seL4 logs have ANSI color codes stripped. They also drop allocator retry lines ("Failed to
allocate object of size ...", printed while the root task probes untyped sizes downward) and
QEMU's own "terminating on signal" line. Nothing else is removed. The nife logs are the summary
lines of `script/bench --real --release` (`benchmark`, `ipc_rtt`, `null_syscall`, the
`cycles_per_tick` probe).

| log | what ran | outcome |
|---|---|---|
| `sel4test-a57-el1-unpatched.log` | sel4test, cortex-a57, unmodified | `ERROR: kernel requires 16-bit ASIDs` |
| `sel4test-a57-el1-asid-patched.log` | with edit 1 | 19 tests pass, then a kernel data abort in CACHEFLUSH0005: ESR `0x96000143`, an address-size fault, because the a57 configuration hands out frames above HVF's 40-bit IPA |
| `sel4test-a57-el2.log` | `ARM_HYP=ON`, `virtualization=on` | `Processor does not support a 44 bit PA` |
| `sel4test-a53-el1-asid-patched.log` | edit 1, cortex-a53 (40-bit PA) | `Test suite passed. 122 tests passed. 54 tests disabled.` |
| `sel4test-a53-el2.log` | `ARM_HYP=ON`, cortex-a53, `virtualization=on` | kernel boots; the root task's first exception lands at `0x400` (`tge.log` says why) |
| `sel4bench-a57-pmu-refused.log` | sel4bench, edit 1, PMU timing, overhead check on | `Benchmarking overhead of a call is not stable! Cannot continue`, overheads 2688 to 2944 counts |
| `sel4bench-a53-pmu.log` | edits 1 and 3, PMU timing | every figure 128 or 256 counts |
| `sel4bench-a53-loop-{1..5}.log`, `-pair-{1..3}.log` | edits 1, 2 and 3 | eight boots, round trip 89 to 99 ns |
| `nife-bench-{1..5}.log`, `-pair-{1..3}.log` | `script/bench --real --release` on `main` at `f87c9b5c4` | eight boots, `ipc_rtt_el0` 395 to 444 ns |
| `nife-test-hvf.log` | `script/test --hvf` on the same commit: its result lines and every IOMMU or DMA test | 140 and 387 passed, 14 skipped |

The `pair` logs were taken alternately: seL4, then nife, three times, at a host load of 5 to 7.
The other five of each were taken back to back at a load of 3 to 8.
