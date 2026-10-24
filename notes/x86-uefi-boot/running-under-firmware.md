# Running under firmware: the two gates, the suite's cost, and SMP

An appendix to [notes/x86-uefi-boot.md](../x86-uefi-boot.md), which keeps the commands. This file
holds what `uefi-boot` and `uefi-test` assert and why, what running the suite under firmware cost,
and how secondary cores come up.

## The two images

`uefi-boot` stages its own copy at `target/esp-screen`, and that one is not for a stick. Milestone 445
(the screen check stops sampling and starts asking) gave the screen check a handshake instead of a
race. The loader there is built with the `screen_hold` feature. It writes one extra word on the
kernel's boot command line, and the kernel stops at the screen handover until the gate has
photographed the framebuffer. A machine booting that image with nobody listening waits ten seconds
and carries on, which is a pause nobody asked for. `target/esp` is the one the bench procedure copies
from, and `cargo xtask uefi-image` is what fills it. The kernel in the two directories is
byte-identical; only the loader differs.

Both gates run inside `script/test --arch x86_64`, after the PVH suite. They are two boots rather
than one because they carry two different kernels.

## What `uefi-boot` asserts

`uefi-boot` boots the tour: the same kernel and archive `uefi-image` stages for the USB stick, and
that calef carries to the bench, behind the screen-hold loader. It runs at two cores, which no other
x86_64 boot in this tree does; see SMP below.

What it asserts, chosen so it cannot pass for the wrong reason:

- `(xsdt)` in the ACPI line, which only the firmware path can print.
- No `rsdp 0x0`, which is PVH's own tell.
- The tour's completion line, which is everything in between: the fine W^X page tables, the APIC,
  the timer, the scheduler, and two ring-3 processes, on a memory map from firmware.
- `smp: 2 core(s) online`, and a PIT interrupt still reaching the boot core with two local APICs on
  the machine.
- The tour's tail on the SCREEN, read back glyph by glyph while the kernel is holding it there
  (milestone 445). Its tail rather than its banner, because the tour is taller than OVMF's 1280x800
  console and scrolls. Held still, the screen's first row is the middle of the firmware memory map.

## What `uefi-test` asserts

`uefi-test` boots the kernel's test binary. That is the same kernel with `test_main()` on the end of
the same tour, so the boot prints every line above and then runs the suite. Milestone 195 (finish the
UEFI boot path) added it. Until it existed, *"it boots under real firmware"* and *"it passes under
real firmware"* were different claims, with only the first one made. It asserts the harness's verdict
and QEMU's exit status, because a transcript scan alone would pass a run that printed its verdict and
then faulted on the way out.

The two boots are the same machine except for their devices. The suite gets the PVH runner's
`virtio-blk-pci` disk and NVMe controller; the tour gets neither. That is what makes the numbers
comparable. On 2026-09-02 they were identical: 192 passed and 68 skipped under both PVH and OVMF,
with the same 68 test names skipped on each side.

## What running the suite under firmware cost, and what it found

It was not the two-line change to `uefi_image` this was scoped as. The reason is the one thing a tour
boot cannot show: the test build is bigger. Its physical span reaches 10 MiB, where the tour's
reaches 2.3. OVMF keeps ACPI NVS at 8 MiB, and its own boot-services allocations from 9 to 23.5. So
`AllocatePages(AllocateAddress)` refused the whole range, and the firmware printed nothing more useful
than `Load Error`.

Two things came out of that, and the second matters more than the first.

`PHYS_START` is 32 MiB, not 1 MiB (`kernel/link-x86_64.ld`). 1 MiB is the *lowest* address
multiboot permits, never the only one, and under a hypervisor's loader nothing else is in low memory
to say so. This is a larger gap rather than a fix. The image is still placed at one address chosen at
link time, so a firmware that wants 32 MiB refuses the boot exactly as OVMF refused 1 MiB. The real
answer is a physically relocatable image. That is a milestone rather than a constant, because `.boot`
holds 32-bit absolute references to its own labels.

And the loader now names what is in the way. `AllocatePages` reports one status and no address. So on
that failure `uefi_loader::say_conflict` walks the memory map, and prints every descriptor overlapping
the range that is not free RAM, with its UEFI type. On the bench that is the difference between a
`Load Error` and a sentence:

```text
uefi_loader: wanted 0x0000000000100000..0x0000000000add000
uefi_loader:   in the way: 0x0000000000800000..0x0000000000808000
uefi_loader:   memory type 0x000000000000000a
uefi_loader:   in the way: 0x0000000000900000..0x0000000001780000
uefi_loader:   memory type 0x0000000000000004
```

## SMP under firmware

`arch::x86_64::ap_boot` copies its real-mode trampoline to physical `0x8000`, because a STARTUP IPI
can only name a page below 1 MiB. Until milestone 195 the loader never mentioned that page. So
secondary cores under firmware worked or did not by luck: OVMF happens to leave the first 640 KiB
conventional. The loader asks the firmware for it by name now. A refusal is a printed warning rather
than a failed boot, because a single-core boot on a machine whose firmware wants that page is worth
more than no boot at all.

Two cores come up under OVMF, and `uefi-boot` gates it on every boot with `smp: 2 core(s) online`
(zero misses across the dedicated measurement,
[uefi-boot-two-core-rate.md](../uefi-boot-two-core-rate.md)). The second local APIC also answers a
question for xenon from the BUGS of milestone 215 (a PCI function's interrupt reaches nothing on
x86_64). It asks whether device interrupts still reach the boot core's id, and the tour's `device
irq` line is that answer.

The suite under OVMF keeps the runner's one-core default. Two defects made two cores flake, and both
are fixed: `boot_cpu_id`, by milestone 316 (which core booted), and `cpu_start` re-reading its count
after the STARTUP IPIs. The PVH runner defaults to two per §153 (how a two-core x86_64 test earns
its place). The whole suite at two cores under OVMF is not measured, so the default stands until it
is.

`NIFE_OVMF_CODE` and `NIFE_OVMF_VARS` name the firmware images on a machine that keeps them somewhere
other than Homebrew's QEMU share; `NIFE_UEFI_TIMEOUT` moves the bound.

## Inside the two gates

`uefi_loader` embeds the kernel ELF and the archive with `include_bytes!`. Its build script takes
both paths from the environment and refuses to build without them, rather than guessing at
`target/`, so a stale `.efi` cannot be produced by hand. The file lands at `\EFI\BOOT\BOOTX64.EFI`,
the removable-media path every UEFI implementation looks for with no configuration. Installing to a
machine's own ESP with a boot entry of its own (`efibootmgr`'s job on Linux) is not done, and a
machine that boots nife by default would need it.

The archive is packed before the kernel is built. Packing regenerates
`target/init-measure-x86_64.txt`, which `kernel/build.rs` compiles in as the measured-boot trust
root. A kernel built first vouches for the previous archive, and the handover refuses the pair:

```text
MEASURED BOOT REFUSED: no measurement for the archive entry 'progenitor'
```

QEMU rarely shows this, because one working tree usually has both fresh. It bites when the kernel was
built earlier in the session. The VisionFive 2 refused the pair on 2026-08-15 through
`script/board-image`, and xenon on 2026-09-17, after its self-test passed.

A release image embeds a kernel with its DWARF removed (calef, 2026-10-03 UTC, on the install boot
slots). The release ELF is about 7.5 MB, 5.8 MB of it `.debug_*` that nothing on the machine reads.
Symbols are resolved offline, against the unstripped file left in `target/`. The strip uses the
archive packer's tool and flag.

The ACPI assertion works because the two loaders find different roots. Under PVH the kernel scans
the BIOS area for the RSDP and finds an ACPI 1.0 pointer with an RSDT. Firmware hands over a
revision-2 RSDP with an XSDT. The runner boots at 2 GiB rather than PVH's 256 MiB, which puts
firmware's tables above 1 GiB, where a real machine keeps them. Asserting the ECAM window, the end
of that chain, makes the whole chain one gate.

The screen is read through the QEMU monitor's screendumps. Everything the framebuffer path can get
wrong shows only there: the loader's `LocateProtocol`, the byte order, the stride, the mapping
surviving `mmu::init`, and the glyphs. The shell's half proves the console server writes both
surfaces while keystrokes still arrive over COM1. Holding the screen is a handshake, calef's choice
among three options on 2026-09-20. If the hold line never comes, nothing falls back to sampling. A
boot that was not asked to hold is a wiring failure (the `screen_hold` feature, the token, the
kernel's reader), and the gate says so instead of blaming working framebuffer code. Each held dump
costs about 650 ms, so four retries sit well inside the kernel's ten-second hold.

The held screen's first row is the middle of the firmware memory map:

```text
                0x00007ea8a000..0x00007eab4000  ram
```

`uefi-test` gives the system image the PVH runner's devices. A PCI function whose BARs firmware
placed is milestone 215's second question for xenon; under `-kernel` the kernel places them itself.
The boot's 300 s bound is a hang detector, the bound `disk_throughput`'s boot uses. In CI the
`system_tests` boot takes about 156 s at the default scanout and about 190 s at 1120x624, whose
display and compositor tests compare 2.2 times the pixels. A passing boot exits as soon as the suite
ends, so the margin costs nothing. The exit protocol's readings rest on one adjacency: the summary
print and the success write are neighboring statements in `kernel/src/testing.rs`.
