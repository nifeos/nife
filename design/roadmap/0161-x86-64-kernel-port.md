---
status: BUILT
raised: 2026-08-23
built: 2026-09-19
---
# 161. The x86_64 kernel port: bring up the HAL's third architecture

BUILT. [Closing the follow-on](#closing-the-follow-on-2026-09-19) has the last four items and their
measurements. What remains is recorded where a reader meets it rather than held here. The two-core
runner default belongs to [milestone 315 (a port revoke that reaches every
core)](0315-port-revoke-every-core.md), under [DECISIONS §153 (how a two-core x86_64 test earns its
place)](../decisions/0153-two-core-x86-test-sequencing.md). Whether to set `CR4.PGE` waits on a
machine with a real TLB (`kernel/src/arch/x86_64/mmu.rs`'s BUGS).

The kernel boots on QEMU's `q35`, runs real ELF programs at ring 3 from an initrd, and runs the
kernel's own suite, which meets the scope note of milestone 20 (a portable HAL, proven on a second
architecture): "enough of each ISA to boot, confine a ring-3/U process, and run the test suite".
`notes/x86-port.md` is the working record.

## The appendices

The page is complete without them; they hold the per-step working.

- [early-boot.md](0161-x86-64-kernel-port/early-boot.md): steps 1 to 10, boot path to IO APIC, with
  what each verified.
- [ring-3.md](0161-x86-64-kernel-port/ring-3.md): step 11 and item 3, the `syscall` pair, the
  `iretq` choice, the syscall ABI decision and the two `CR4` bits.
- [processes-and-userspace.md](0161-x86-64-kernel-port/processes-and-userspace.md): steps 12 and 13
  and item 4, the scheduler, real processes, `user_rt`, the archive, the bugs found and the 67
  skips.
- [open-list.md](0161-x86-64-kernel-port/open-list.md): items 0 to 2 as recorded, the port-I/O
  question and the two renames.
- [smp-and-vt-d.md](0161-x86-64-kernel-port/smp-and-vt-d.md): items 5 and 6, INIT-SIPI-SIPI, the
  three SMP failures, the NMI shootdown and the VT-d driver.
- [closing.md](0161-x86-64-kernel-port/closing.md): the 2026-09-19 lane's working, with the
  page-table, fastpath and SMP measurements.

## Why x86_64, and why second

[DECISIONS §19 (architectural parity is a tenet)](../decisions/0019-architectural-parity.md) declared
the targets aarch64, riscv64 and x86_64, and recorded that *"x86_64 is a declared target that does
not exist yet."* Milestone 20's deliverable named x86_64 second. But 20 is BUILT for the HAL split
plus RISC-V alone, and nothing tracked the x86_64 half. This milestone is that tracking.

x86_64 is the hard proof that the HAL abstraction is real rather than an accident of two similar
RISC ISAs. It is CISC with strong TSO ordering, GDT/TSS, ACPI and PCI instead of a device tree, port
I/O, the `syscall` + `swapgs` trampoline, and INIT-SIPI-SIPI instead of PSCI/SBI. Code proven
correct on a weak machine is correct on TSO, which is rule #4's weak-first bet paying out. Also the
reach: x86_64 is what most machines are.

Nothing gated it. It started under QEMU TCG, the way RISC-V's port did, and did not wait for
[milestone 87 (the x86_64 bare-metal machine)](0087-x86-machine.md). That machine, an OptiPlex
7050, is the eventual parity-proof hardware. `q35` emulates the same 16550 UART it carries, so the
serial driver spans both from the start.

## What this did not decide

The exact boot path (a minimal UEFI stub vs. a bootloader) and the SMP bring-up sequence's precise
shape were implementation judgment for whoever built this. §19 named the stress points, not the
mechanism.

## What it unblocks

Architectural parity (§19) reaching all three declared targets rather than two.
[Milestone 25 (cross-OS performance comparison)](0025-cross-os-comparison.md)'s `sel4bench`
comparison gets a third point once real x86 hardware (87) is behind it.

## What was built (2026-08-23 and 2026-08-24)

Ordered as it was built, because each step is what made the next one debuggable. The per-step
working and verification are in the appendices named under each group.

Early boot ([early-boot.md](0161-x86-64-kernel-port/early-boot.md)):

1. The boot path: a 32-bit trampoline into long mode and the high half. Multiboot 1 cannot boot a
   64-bit kernel and QEMU 11 has no Multiboot 2, so the image carries a PVH ELF note.
2. The console: the existing NS16550 driver, given a `RegisterSpace` type parameter so the same
   part works at an I/O port.
3. The trap path: GDT, TSS (double fault on its own IST) and an IDT. Proven by taking an `int3`.
4. The page-table format: `crates/paging/src/x86_64.rs`, sixty lines plus host tests and Kani
   harnesses, with no change to the generic walk.
5. The PVH handoff, parsed in `crates/machine_discovery` and host-tested.
6. ACPI (RSDP, MADT, MCFG), host-tested, every checksum verified, checked against q35's firmware.
7. The local APIC and a timer calibrated against the PIT: 20 ticks in ~0.2 s at 100 Hz, 0 spurious.
8. The frame allocator, behind a front end that takes RAM and forbidden slices from any source.
9. Fine-grained W^X page tables with Linux's two bases. `boot.s` installs the direct map's PML4
   entry itself, so `phys_to_virt` never changes meaning. 556 KiB of tables for 254 MiB at first.
10. The IO APIC, with a real PIT line routed through it. q35 rewires ISA IRQ 0 to GSI 2, and
    `machine_discovery::acpi::isa_irq_table` resolves that under host tests.

Ring 3 ([ring-3.md](0161-x86-64-kernel-port/ring-3.md)):

11. User address spaces, the CPL-guarded `swapgs` pair, the `syscall` entry, and an `iretq` return
    chosen over `sysretq` (the CVE-2012-0217 shape). A hand-assembled probe ran at CPL 3, reached
    the portable dispatcher, and was refused reading the kernel's `.text`. Two latent user-thread
    bugs were fixed on the way.

Processes and userspace ([processes-and-userspace.md](0161-x86-64-kernel-port/processes-and-userspace.md)):

12. The scheduler and real processes, almost all portable code compiled for a third target. Two
    children built from untyped memory ran at ring 3: one used a granted capability, one faulted to
    its supervisor, and the steady-state round cost zero frames. Four portable-code bugs were found,
    each an assumption true of two architectures. The suite ran 97 and skipped 7.
13. Every program in `user/` compiled for `x86_64-unknown-none`, and an initrd delivered as a PVH
    module. `now()`/`cntfrq()` became PIT-calibrated `rdtsc`, ratified as
    [DECISIONS §127 (x86_64's `now()`/`cntfrq()`: PIT-calibrated `rdtsc`)](../decisions/0127-x86-64-timer-rdtsc.md).
    Four more latent bugs and one cache-tag collision were fixed. The suite went to 170 pass and 67
    skip, every skip naming its fixture.

Plus the build wiring: `kernel/build.rs`, `.cargo/config.toml`, `rust-toolchain.toml`,
`helpers/qemu-runner-x86_64.sh` and an x86_64 pass in `script/lint`. The evidence that the `arch/`
split is real: the whole kernel compiled for a third architecture after 42 compiler errors, every
one a missing `arch::` name. `crates/paging` needed nothing.

## The open list, and where each item ended

The items were numbered in the order they had to be done, and other files cite them by number.

0. The device-discovery seam. The narrow half (`memory::bring_up_page_frames`) is built here. The
   wide half went to [milestone 176 (the x86_64 discovery seam's wide half)](0176-x86-64-discovery-seam-wide-half.md);
   its CMOS RTC piece, shaped by
   [DECISIONS §130 (how the kernel-resident CMOS RTC reaches the userspace clock service)](../decisions/0130-cmos-rtc-delegation.md),
   is now built. See [open-list.md](0161-x86-64-kernel-port/open-list.md).
1. Fine-grained page tables. BUILT 2026-08-24 (step 9). Its leftover, 2 MiB and 1 GiB leaves, was
   built by the closing lane on all three formats.
2. The IO APIC. BUILT 2026-08-24 (step 10). MSI went to
   [milestone 215 (a PCI function's interrupt reaches nothing on x86_64)](0215-x86-64-pci-interrupt-routing.md);
   INTx routing was refused and is recorded as milestone 459 (legacy INTx interrupt routing on x86_64).
3. Ring 3. BUILT 2026-08-24 (step 11). The syscall ABI went up as
   [DECISIONS §124 (ratify the x86_64 syscall ABI)](../decisions/0124-x86-64-syscall-abi.md), ratified
   by calef 2026-08-24. `CR4.PGE` and `CR4.PCIDE` stay off, now as a measured choice.
4. The scheduler, real processes and the suite. BUILT 2026-08-24 (steps 12 and 13). Its two new
   findings were the `fs_server`/`aes` toolchain failure, split to
   [milestone 164 (x86_64 userspace can't build `aes`)](0164-x86-64-fs-server-aes.md), and the
   marked foot gun of a device capability over physical page zero in slot 2.
5. SMP via INIT-SIPI-SIPI. Mechanism built 2026-08-25, with its two named bugs fixed. Three failures
   followed. (2), a missing cross-core TLB shootdown, was fixed the same day with an NMI shootdown,
   since a maskable IPI would deadlock under `IrqSafeMutex`. (1) and (3) were closed later (see
   the follow-on). See [smp-and-vt-d.md](0161-x86-64-kernel-port/smp-and-vt-d.md).
6. VT-d. BUILT 2026-08-25: root and context tables, default-deny, register-based invalidation and
   fault detection. Its scoped-out limits are listed under the follow-on.

A capability shape for port I/O was resolved rather than built.
[DECISIONS §121 (what a device capability is when the device has no page)](../decisions/0121-port-io-capability.md)
keeps x86's legacy port devices kernel-resident permanently, a recorded parity gap. The two
arch-contract names that did not stretch to a third architecture were renamed `arch::cpu_start` and
`boot_info_pointer`.

## Follow-on

- **Milestone 176.** Item 0's last piece, the CMOS RTC seam, is built rather than "DECIDED but not yet
  built": `kernel/src/arch/x86_64/rtc.rs` reads CMOS at boot and `clock_protocol`'s RTC seam carries
  the seed to the clock service.
- **Done.** `crates/paging` maps 2 MiB and 1 GiB leaves on all three CPU formats, and all three kernels'
  direct maps use them for RAM. See "Closing the follow-on" below.
- **Milestone 215.** Item 2's MSI half is built: an MSI intid is its vector, a `virtio-blk-pci`
  completion reaches a ring-3 driver, and the enable and acknowledge paths are correctly no-ops for
  one.
- **Refused.** Item 2's other half, PCI interrupt routing over INTx, was deliberately not taken by
  milestone 215: ACPI's `_PRT` is AML and this tree will not grow an interpreter, and hardcoding
  q35's swizzle would pass every gate here and might still fail on xenon. This refusal is
  [milestone 459 (legacy INTx interrupt routing on x86_64)](0459-legacy-intx-interrupt-routing.md),
  which carries it with the condition that would change it.
- **Recorded.** Measured 2026-09-19; both `CR4` bits stay off. The pinned QEMU flushes its whole TLB on
  every `CR3` write, so neither this tree's icount instrument nor plain TCG can see what PGE saves.
  `kernel/src/arch/x86_64/mmu.rs`'s `BUGS` and `notes/benchmarks/x86-instruments.md`'s 2026-09-19
  section hold the numbers and the trigger (KVM on cordoba, or xenon).
- **Milestone 186.** The bench tooling item 3 built has no caller: the x86 baseline and its check
  flag exist, and CI's bench job runs two legs of three.
  [Milestone 186 (derive the architecture list)](0186-architecture-list-derivation.md) tracks that
  as one of its eleven gaps.
- **Milestone 164.** `fs_server` builds for `x86_64-unknown-none`. One build flag turned exit 101 into
  exit 0; `redoxfs_server` and `mkfs` ride the x86_64 archive, and the 21 skips this item blamed on
  `aes` are gone.
- **Milestone 215.** The runner attaches a PCI device now: `helpers/qemu-runner-x86_64.sh` carries a
  `virtio-blk-pci` line with `iommu_platform=on` plus an `nvme` line. The 15 skips that wanted an
  enumerated bus with something on it no longer have that cause.
- **Done.** Item 6's "no PCI device is confined through it yet" is closed by that same disk. It goes
  through `-device intel-iommu` with `iommu_platform=on`, and milestone 215 records the IOMMU escape
  test running on x86_64 for the first time and passing.
- **Milestone 364.** The fixtures still missing from that runner (RedoxFS, GPT and blank disks, NIC,
  GPU, keyboard, RNG, and a transport-blind FS-server disk lookup) are
  [milestone 364 (most x86_64 tests take a "no RedoxFS disk attached" arm)](0364-x86-64-test-fixtures.md).
  [Milestone 303 (x86_64's FS service has a server and no disk it can find)](0303-x86-64-fs-disk.md)
  and a newer block have since taken it over; it is SUPERSEDED and says where each half went.
- **Milestone 184.** The skip that wanted an `x86_64-unknown-nife` target and a `std` farm is no
  longer the tail of [milestone 27 (Rust `std` on the native ABI)](0027-rust-std.md). It has its own
  block, [milestone 184 (extend the `std` port to x86_64)](0184-std-x86-64.md).
- **Recorded.** The five UART skips stay skipped by design: §121 keeps x86's legacy port devices
  kernel-resident permanently. The device capability init hands out in a positional slot stays a
  marked foot gun in `kernel/src/user.rs`.
- **Recorded.** VT-d's scoped-out limits hold as written in `kernel/src/arch/x86_64/iommu.rs`'s `BUGS`:
  one DRHD, no interrupt remapping, global-granularity invalidation only, `RWBF` never exercised, and
  only the first Fault Recording Register decoded.
- **Done.** SMP failure (1), fixed 2026-09-19, was a counting bug in `cpu_start`, not a core failing to
  start; `kernel/src/arch/x86_64/ap_boot.rs`'s `BUGS` #1 has the evidence.
- **Milestone 316.** SMP failure (3) was fixed in
  [milestone 316 (which core booted)](0316-x86-smp-two-cores.md) on 2026-09-17: `boot.s` stamps
  `BOOT_CPU_ID` once and `boot_cpu_id` reads it. Re-verified against this tree on 2026-09-19.
- **Milestone 315.** The x86_64 runner's `NIFE_SMP` default moves to 2 as that milestone's closing step
  (§153, answered 2026-09-18). Nothing in the SMP bring-up holds it at 1 any more.
- **Milestone 167.** The 1 GHz `cntfrq` fallback for children built through the supervision
  protocol is the scope of
  [milestone 167 (handing a computed page to a userspace-built child)](0167-timebase-page-delegation.md),
  and it is still NOT-STARTED.

## Closing the follow-on (2026-09-19)

Each item was re-checked against the tree first. One was already done by another milestone (SMP
failure 3, milestone 316); the other three were open as written. The working is in
[closing.md](0161-x86-64-kernel-port/closing.md).

1. Blocks in `crates/paging`. `PageFormat` grew `block_entry` and `is_block` with no defaults, and
   `Mapper` gained `map_block` and `map_span`. Nothing splits a block. x86_64's kernel page tables
   on 256 MiB went from 560 KiB to 60 KiB. Eight new Kani harnesses prove the leaf choice and each
   format's block encoding. The fastpath stayed byte-identical to `main`. `PageSize`, `map_block`,
   `map_span`, `InsideBlock` and `UnsupportedSize` are provisional names. Recorded, not fixed: the
   kernel does not read the MTRRs (`mmu.rs`'s BUGS).
2. `CR4.PGE` and `CR4.PCIDE`. icount tick counts were byte-identical with PGE on and off, and TCG
   timing overlapped within 1%, because the pinned QEMU flushes its TLB on every `CR3` write. The
   bits stay off and the question is not answered. KVM on cordoba, or xenon, would answer it.
3. SMP failure (1). 26 of 40 four-core boots reported a core "did not start". `cpu_start` re-read
   the online count after the STARTUP IPIs, so a core that checked in during the settle delay was
   counted absent. Reading it once, before the INIT, fixed it: 40 of 40 at `-smp 4`, 20 of 20 at
   `-smp 8` and at `-smp 3`. It plausibly explains the UEFI leg's one-in-three in
   [milestone 412 (`script/test`'s UEFI leg asserts two cores online)](0412-the-uefi-boot-gate-asserts-two-cores-that-do-not-always-start.md),
   not measured here.
4. SMP failure (3). Already fixed by milestone 316 (`BOOT_CPU_ID`); verified in the tree.

Can the x86_64 runner default to more than one core? Not from this milestone. Both bring-up bugs
are closed, and §153 makes the flip milestone 315's closing step, after its port-revoke broadcast
makes `a_revoked_holder_faults_on_its_next_port_write` reliable at two cores.

## Index row

Splits real work out of milestone 20's stale text: DECISIONS §19 declared x86_64 a target,
milestone 20's own "Deliverable, in two parts" named it, but 20 is BUILT for the HAL split and
RISC-V alone (`kernel/src/arch/` has no `x86_64/`). BUILT 2026-09-19. The fine map, IO APIC,
ring 3, real processes, VT-d, a real userspace and SMP bring-up are built; the last four items closed
together: 2 MiB and 1 GiB leaves in `crates/paging` on all three formats, proved, taking x86's
direct map from 560 KiB of tables to 60 KiB; SMP failure (1), which was `cpu_start` counting a
started core as absent (26 of 40 four-core boots before, 80 of 80 at three to eight cores after);
failure (3), fixed by milestone 316; and `CR4.PGE`/`PCIDE`, measured and left off because the pinned
QEMU flushes its TLB on every `CR3` write, so no instrument here can see them. The two-core runner
default is milestone 315's closing step.
