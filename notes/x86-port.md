# Porting to x86_64

Milestone 161, the third architecture, and the one milestone 20 named as the real test of whether
`arch/` is a hardware abstraction layer or an accident of two similar RISC machines. aarch64 and
RISC-V share a device tree, weak memory ordering, a boot handoff in the width the kernel runs in,
and a page-table shape. x86_64 shares none of those.

This note is what the port learned, in the order a reader needs it: how the machine is entered, what
had to change above `arch/` (very little, and the exceptions are listed), the three things that
genuinely do not fit the existing seam, and an honest account of what is built.

## Status, and it is a partial port

Built, running, and gated: the boot path, the console, the GDT/TSS, the IDT and trap frame, the
page-table format, the boot-handoff parser, the ACPI tables (the RSDP scan, the root-table walk with
checksums, the MADT and the MCFG), the local APIC and a calibrated periodic timer, the frame
allocator, the fine-grained W^X kernel page tables, the IO APIC and a routed device line,
user address spaces, the `syscall` pair and ring 3, the scheduler, preemption, kernel threads
and real ring-3 processes, the kernel's own test suite and a `script/test` leg, the address
arithmetic, interrupt masking, the context switch, and the test exit. A boot under QEMU's `q35`
prints a tour, takes real hardware interrupts from the CPU's own timer *and* from a device, builds
and installs its own page tables, brings up the scheduler, runs a kernel thread, builds two
processes out of untyped memory and runs them at CPL 3 (one invokes a capability and exits, one
faults and is delivered to its supervisor), and halts.

And since 2026-08-24, userspace: every program in `user/` compiles for `x86_64-unknown-none`,
`xtask` packs the same archive RISC-V's leg does, QEMU's PVH loader hands it over as a module, and
`cfg(initial_ramdisk)` is on. `script/test --arch x86_64` runs 170 tests and skips 67, where it ran 97
and skipped 7 the day before.

**Not built:** VT-d and SMP bring-up, both loud `unimplemented!()`s in `arch/x86_64/` that name
themselves and why. What bounds this architecture now is not userspace but **devices**: no PCI bus
is enumerated, so no virtio function of any kind is found, and the console UART is in an I/O port
space with no capability shape yet (DECISIONS §121). See "What a userspace still does not have here"
below, and design/roadmap/0161-x86-64-kernel-port.md for the order the rest comes in.

## Reproducing it

```sh
cargo build -p kernel --target x86_64-unknown-none
helpers/qemu-bounded.sh 20 qemu-system-x86_64 \
    -machine q35 -cpu max -smp 1 -m 256M -display none -serial stdio -no-reboot \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -kernel target/x86_64-unknown-none/debug/kernel
```

or, equivalently, `cargo run -p kernel --target x86_64-unknown-none` (the runner in
`.cargo/config.toml` builds the same command line; bound it, because the kernel halts rather than
exiting).

The tour it prints on 2026-08-24, with how to read it (the `mmu` line, the faulting child of each
round, what changes from boot to boot), is in
[boot.md](x86-port/boot.md#the-tours-expected-output-and-how-to-read-it). It ends
`nife x86_64: boot complete, halting.`

To run the kernel's own suite instead of the tour:

```sh
script/test --arch x86_64
```

To see a triple fault instead of a blank terminal, add `-d int,cpu_reset -D /tmp/x86.log`.

## What the gates cover, and what they do not

`script/lint` runs clippy over the x86_64 kernel binary at `-D warnings`, which covers every line of
`arch/x86_64/` and every portable file compiled with `target_arch = "x86_64"`. Three narrowings, each
argued where the gate is (see `script/lint`):

- **No `--all-targets`**: written when the test suite did not compile for this target. It does now
  (milestone 161's item 4), so this narrowing is stale and is the next thing to widen; the suite is
  gated by `script/test --arch x86_64` in the meantime, which is the stronger check of the two.
- **No feature loop**: all eight boot-mode features fail on x86_64, measured, because each selects a
  boot path needing an arch layer this port has not written.
- **`-A dead_code`**: partly stale for the same reason. The tour no longer halts before the
  scheduler, so much of what was unreferenced now is not; what remains dead here is what needs a
  userspace (the services, the drivers). Code dead on *all three* architectures is still caught by
  the other two passes; what can hide is code dead on x86_64 alone.

There is a `script/test` leg: `--arch x86_64`, in `xtask`'s `test`, and it runs by default
alongside the other two. That sentence used to end "it builds nothing before it boots, because there
is no userspace archive to pack and the runner attaches no disks", and every clause of it has since
stopped being true: milestone 161 packed an archive, 164 added the FS server, 215 attached the first
`virtio-blk-pci` disk and 303 the RedoxFS fixture. The leg now builds the FS server, the archive, and
the nifefs, RedoxFS and NVMe images before it boots.

## What had to change above `arch/`

This is the part that matters for the milestone-20 claim, so it is stated as a list rather than as a
conclusion. Making the entire kernel compile for a third architecture took 42 compiler errors,
every one of them "this `arch::` name does not exist yet", and:

- **`crates/paging` did not change at all.** `paging::x86_64::Ia32e` is sixty lines of bit encoding
  behind the existing `PageFormat` trait; `LEVELS = 4` and `SPLIT_SHIFT = 47` were the whole of the
  geometry, and the shared `Mapper` walk needed nothing.
- `drivers/ns16550.rs` gained a type parameter and no second driver. The same 16550 QEMU's
  RISC-V `virt` puts at physical `0x1000_0000` is, on every x86 machine, at I/O port `0x3f8`: a
  separate address space reached only by `in`/`out`. That is a difference in how eight registers are
  *reached* and in nothing else, so it is a `RegisterSpace` implementation (defaulting to `Mmio`, so
  every existing use means what it always did) with the port-space half under `arch/x86_64/`.
- `console.rs`, `user.rs`, `drivers/mod.rs` and `user/fs_service.rs` gained `cfg` arms, in the
  same places they already had two.

That is the whole diff above `arch/`. A new ISA was a new directory.

## The appendices

[`notes/x86-port/`](x86-port/README.md) holds the measurements and reasons behind each part of
milestone 161 (the x86_64 kernel port), one file each. None of them is needed to build, boot or test it.

| appendix | what it holds |
|---|---|
| [boot.md](x86-port/boot.md) | Why the kernel boots by PVH and not Multiboot, the real-firmware path through `uefi_loader`, and the 32-bit trampoline into long mode. |
| [acpi-and-pci.md](x86-port/acpi-and-pci.md) | The PVH memory map, the ACPI tables as q35 reports them, the discovery seam milestone 20 (a portable HAL) promised, and why a PCI function's interrupt is MSI-X rather than the legacy pin. |
| [interrupts-and-the-fine-map.md](x86-port/interrupts-and-the-fine-map.md) | The PIT-calibrated clock, the IO APIC and its legacy-IRQ trap, the W^X kernel page tables, and the direct map in blocks. |
| [ring-3.md](x86-port/ring-3.md) | Address spaces, `swapgs`, the `syscall` pair, and the hand-assembled probe that first ran at CPL 3. |
| [scheduler.md](x86-port/scheduler.md) | The trap-path split, `TSS.RSP0`, the self-IPI, and four places portable code was wrong for a third architecture. |
| [user-mode-runtime.md](x86-port/user-mode-runtime.md) | The ring-3 syscall stubs, why `rdtsc` is ambient here, the `CR4.PCE` door that was closed, and `cntfrq()`. |
| [userspace.md](x86-port/userspace.md) | How the archive arrives as a PVH module, the suite's skips by cause, and four latent bugs userspace found. |
| [what-does-not-fit.md](x86-port/what-does-not-fit.md) | The four places the `arch/` seam did not stretch, the segment-base bug, the SMP counting bug, and where TSO pays out. |

## BUGS

- Multiboot is still not an option, and nothing here added a header. The refusal in [boot.md](x86-port/boot.md) is a
  property of QEMU's loader rather than of this kernel, so it stands. GRUB Multiboot 2 remains the
  path for a BIOS-only machine and would cost a header plus a second handoff decoder; it was priced
  against UEFI and lost on testability (`brew info grub`: no formula on this machine at all). See
  notes/x86-uefi-boot.md's fork.
- The four places the `arch/` contract does not fit x86 are recorded in
  [what-does-not-fit.md](x86-port/what-does-not-fit.md). Two of them are names, and names are
  an architect's.
- The swapgs exit window is open to an NMI or a machine check, because there is no paranoid entry
  path. See [ring-3.md](x86-port/ring-3.md).
- `rdtsc` is readable from ring 3 by inheritance, not by decision. Closing it needs a coarse time
  source first. See [user-mode-runtime.md](x86-port/user-mode-runtime.md).

### Two boot-mode features that do not compile here

These are the scope notes §19 (architectural parity is a tenet) asks for, one for each gap
`helpers/lint_arch_parity.py` allowlists, written 2026-10-08 (UTC). Neither is planned. Under §71 (a limitation is promoted when it becomes a plan),
each stays a `BUGS` entry until one of that section's triggers fires, and each names the one that
would.

- `--features fastpath_pad` does not compile, because this port has no padding sled.
  `kernel/src/fastpath_pad.rs` calls `arch::fastpath_pad_body`. aarch64 and riscv64 each define it
  as a `global_asm!` run of `nop`s, and their linker scripts place its section first in `.text`;
  `kernel/link-x86_64.ld` has no such line. The sled exists for E3 in milestone 134 (the register
  of measures), a footprint experiment aimed at radon's 32 KB L1i (notes/footprint-perturbation.md). x86_64 joined
  `script/fastpath-footprint` on 2026-08-27, after the sled was written, and no x86 measurement has
  asked for one since. What the gap costs: milestone 796 (pin the hot trap path's placement) gets
  only its containment check on this ISA, not the shift check, and `script/lint` cannot clippy the
  feature here. The port is small, a third `fastpath_pad.rs` of about 50 lines sized against
  `bench/fastpath-x86_64.txt`, plus one linker-script line. It becomes a plan when someone proposes
  an E3 run on xenon, or wants 796's shift check on x86_64.
- `--features icount` does not compile, because nobody wrote the x86_64 timer half of the
  instrument from milestone 78 (the load-sensitive assertions). `kernel/src/icount.rs` reads `arch::timer::{ARRIVAL_BOUND,
  HANDLER_BOUND, calibration_loop, deadline, missed_ticks}`, and this port has none of the five.
  The cause is the timer. The instrument checks every tick against the deadline the kernel armed
  (notes/instruction-clock.md). Here the local APIC timer is a periodic hardware reload
  (`irq::arm_periodic_timer`), so there is no armed deadline to read and no missed tick to count.
  Porting it means first changing how this port ticks, to the APIC's one-shot or TSC-deadline mode.
  That is a timer redesign rather than a port, and neither mode has been tried under TCG here. A
  partial port to the periodic tick would have to redefine claim 2, whose span starts at the armed
  deadline. Nothing waits on it: `cargo xtask bench --x86` already gates instruction counts on this
  ISA, and milestone 161 found that `CR4.PGE`, the one question that wanted an icount leg here, does
  not move under icount at all. It becomes a plan if the tick moves to a deadline mode for its own
  reasons, or if a milestone needs an instruction-denominated bound on this port's handler.

### What a userspace still does not have here

The bound on everything above, listed because it is the next lane's brief rather than a caveat.
Every item is a device or a toolchain, and none is `user_mode_runtime` any more.

- No device a ring-3 process can reach. The console UART is in the I/O port space, so
  `user::UART_PHYS` is zero and `console`, `input`, `keyboard_driver` and `swapper` are packed but cannot run;
  their arms `trap()` rather than no-op, so a boot that reached one would say so on the first byte.
  That is DECISIONS §121, still PROPOSED. **One foot gun is marked rather than removed**:
  `spawn_hello` grants slot 2 a device capability over `UART_PHYS`, which on this architecture is
  *physical page zero*. The slot is positional, so declining to grant would renumber the interrupt
  capability and every role that names it, and there is nothing better to put there until §121 is
  answered. Nothing reaches it: every fixture that would map it asks
  `user::machine_has_no_device_page_for_the_console()` first.
- The PCI bus is enumerated, and one function is driven (milestones 165 and 215). ACPI's MCFG
  fills `memory::pci_regions()`, and a `virtio-blk-pci` disk is attached, confined behind VT-d, and
  read and written by a driver at ring 3. What is still not attached is a NIC, a GPU, a keyboard,
  an RNG, or a second disk: each is a line in `helpers/qemu-runner-x86_64.sh` and a wiring, not a
  mechanism.
- The ACPI walk reads the boot map, and the boot map ends at 4 GiB.
  `arch::x86_64::machine::BOOT_DIRECT_MAP_LIMIT` is that bound. Firmware puts its tables just under
  the top of RAM, so a bound below it finds no RSDP, no MADT, no MCFG and no DMAR, and comes up with
  no APIC, no timer, no PCI and no VT-d on a machine that describes all four. A 256 MiB QEMU run
  cannot show it, so `cargo xtask uefi-boot` boots at 2 GiB for exactly this reason (`NIFE_MEM` sets it back), and an unreachable table says so during the walk rather than
  being skipped with the checksum failures. A machine that put its tables above 4 GiB (none
  seen; firmware keeps ACPI low so 32-bit loaders can read it) needs `boot.s` widened, not the
  bound loosened.
- The 32-bit BAR window is a constant, and this kernel moves most of the bus into it.
  `arch::mmu::PCI_BAR_PHYS` is `0xc000_0000` with 2 MiB mapped, checked once against QEMU's
  `info mtree`, because the window a real machine wants BARs in is in its host bridge's `_CRS` and
  `_CRS` is AML. `place_bars` relocates every BAR outside it, which is correct and exercised;
  `pci::bar_census` prints how many that is on the boot line, and it is not a corner case:
  5 of 8 functions under PVH, 3 of 6 under OVMF, 4 of 7 with a `virtio-blk-pci` disk attached
  (2026-09-02). On xenon that number is the first thing to read: a machine whose RAM reaches above
  `0xc000_0000` would have this kernel move most of its bus on top of memory.
- No MCFG means no PCI, deliberately. There is no fallback to the legacy `0xcf8`/`0xcfc`
  configuration mechanism, which this kernel could reach and which would enumerate bus 0. Those
  ports see only the first 256 bytes of a function's configuration space, so a machine that fell
  back would enumerate a different set of capabilities than one that did not, every extended
  capability absent, and a driver that then failed would fail somewhere else entirely. Milestone
  215 refused the same shape one level down (a machine that wants MSI and meets a function without
  MSI-X fails loudly rather than falling back to a pin).
- An MCFG whose first bus is not 0 is refused rather than adjusted. `kernel/src/pci.rs`
  addresses a function as `base + (bus << 20 | ...)` with an absolute bus number, and the
  subtraction that looks like the fix names a base below the window `mmu::map_everything` maps.
  Every machine seen reports 0; none is required to.
- ~~No RedoxFS image is attached~~: closed by milestone 303. The runner attaches the
  `-redoxfs.img` fixture as a second `virtio-blk-pci` function, and `virtio::find_block_device_n`
  spans virtio-mmio and virtio-pci so a wiring on a machine with no mmio bus can find it. What is
  still missing is the rest of the fixture set (milestone 37's crash disk, milestone 57's GPT and
  blank disks); see design/roadmap/0420-the-rest-of-the-x86-64-fixture-set.md.
- ~~No `std`~~: closed by milestone 184. `x86_64-unknown-nife` and its farm exist, and
  `std_exerciser` passes here. `std::fs` runs since milestone 303 gave the FS service a disk;
  `std::net` is compiled and unexercised for the NIC reason above. See notes/std.md.
- **No second core** (item 5), and no ASID tags, because `CR4.PCIDE` is off (item 3, calef's
  call, and it wants a number rather than an argument).
