# The boards' screen: `ramfb`, and why it is the other way round

An appendix to [`notes/serial-less-output.md`](../serial-less-output.md). It holds how QEMU's `virt`
boards got a screen on aarch64 and riscv64, the gate that reads it back, and what the first red CI
run settled.

Built 2026-09-19, closing the second of milestone 243's two outstanding items.

`x86_64` gets a screen because UEFI already lit one and the loader only has to measure it and say
where it is. QEMU's `virt` boards have no such stage: they are entered from `-kernel` with nothing
configured, so there is no framebuffer to discover and the arch-neutral halves of this path sat
unused on two of three architectures.

What `virt` can present is `ramfb`, and it inverts the arrangement: the guest owns the pixels and
hands the device their physical address, after which QEMU scans them out continuously. So there is no
flush, no doorbell and no interrupt between a `println!` and the picture, which is exactly the
property a console wants and exactly what a display *device* would not give.

The pieces, and note that only the first two are new:

| Piece | Where |
|---|---|
| The `fw_cfg` wire format, host-tested | `crates/firmware_configuration` (**name provisional**): every field big-endian, encoders that take native values and return bytes |
| The register poking | `kernel/src/drivers/ramfb.rs`: two DMA transactions, before `arch::mmu::init`, through the coarse boot map |
| The memory and the wiring | `kernel/src/screen.rs` (**name provisional**): 800x600 of `.bss`, the device-tree lookup, and the same `console::attach_screen` call `x86_64` makes |
| Painting text | `crates/screen_console`, unchanged |
| The gate | `cargo xtask screen-boot <arch>` (**name provisional**), `uefi-boot`'s twin, decoding with `serial_console::screen` unchanged |

Measured 2026-09-19, `cargo xtask screen-boot`, inside `script/test`'s two board legs:

```text
--- the boot tour on a screen, aarch64 (QEMU virt + ramfb) ---
screen-boot: read 33 non-blank row(s) of the aarch64 tour back off a ramfb, ending
screen-boot:   |   self-test       : scheduler  ok  thread 5 ran and carried its captured state
screen-boot:   | nife self-test: 5 of 5 passed

--- the boot tour on a screen, riscv64 (QEMU virt + ramfb) ---
screen-boot: read 63 non-blank row(s) of the riscv64 tour back off a ramfb, ending
```

It is a boot of its own rather than a stage of the suite, and that is forced rather than chosen.
`ramfb` adds a QEMU *console*, `screendump` with no device argument writes console 0, and
the suite's machine already has a virtio-gpu there. A boot carrying both would be photographing
whichever QEMU happened to order first, and the gate would mean something different depending on
QEMU's version.

Two things the first red CI run settled, both worth knowing before touching this gate.

The archive is chosen per architecture at the spawn. `cargo()` exports `NIFE_INITIAL_RAMDISK` pointing at
the aarch64 archive, and every riscv64 caller in `xtask` has to override it. `screen_boot` did not,
at first, which on a machine with the aarch64 archive already on disk produced
`MEASURED BOOT REFUSED` inside an otherwise passing run, and in a CI job that never built one
produced `could not load ramdisk` and no boot at all. A refusal message inside a green gate is still
a refusal.

The screen legs do not run under `--cpu`. `script/cpu-matrix` runs the riscv64 suite five times
to narrow the ISA, and nothing on this path varies with `-cpu`: byte moves, MMIO stores and integer
arithmetic, all of it already executed on that model by the suite above. The leg runs on every
ordinary `script/test`, `--arch riscv64` included.

The gate says which channel failed, and that is the part worth copying. Its second diagnostic
line checks the *serial* transcript for the same marker it could not find on the screen:

```text
screen-boot: nothing decodable was ever on the screen (did QEMU get a ramfb and a monitor?)
screen-boot: the tour never reached the SERIAL line either, so this is a boot failure and not a screen one
```

Without that line a blank screen sends the next reader into the framebuffer path. It costs one
`contains` and separates "this mechanism is broken" from "the machine did not boot".

What it does not claim. `ramfb` is QEMU's; no real board has one. It proves the arch-neutral
console and the arch-neutral discovery *type* on all three architectures, and it proves nothing about
the DC8200 on the VisionFive 2. That is milestone 157 (real display output on the board), and the shape of the change it needs is
one
branch above `screen::attach` and nothing below it.

## The driver's constraints

`kernel/src/drivers/ramfb.rs` talks to `fw_cfg` twice, before `arch::mmu::init`: once to walk the
file directory and once to write the configuration. The coarse boot map covers every device window
on both boards, so nothing is mapped for it, and the fine map does not cover `fw_cfg` at all. After
boot there is nothing more to say to the device, and a window nobody can reach is a window nobody
can be wrong about. The pixels are the kernel's own `.bss`, mapped at the same address by both maps,
so the console keeps its pointer across the switch.

The DMA address register takes two 32-bit halves, high first, and the device starts the transfer
on the low write, so the pair is atomic without a doorbell. A `SeqCst` fence sits before that
write. The command bytes are normal memory and the register is device memory, and on a weakly
ordered machine the store that starts the transfer may be observed before the stores that say what
it is. No other kernel code writes a structure a device reads outside the virtio ring's barriers.
The fence has no pair in this tree: the reader is QEMU's `fw_cfg` model. It is the device-facing leg
of the shape `components/src/compositor.rs`'s `flush` has, one step further out.

`screen::is_kernel_memory` answers by address range rather than by a flag on the screen, so it
stays true for a framebuffer from anywhere. Milestone 157's U-Boot aperture lies outside the kernel
image and passes.

## BUGS

- A `ramfb` screen costs 1.9 MB of `.bss` on every board boot, including the VisionFive 2 boots
  where the device cannot exist and the region is never written. `kernel/src/screen.rs`'s own BUGS
  has the reason (the frame allocator does not exist yet and cannot serve 469 contiguous pages
  anyway) and the exit (157's handoff needs no buffer at all).
- A `ramfb` screen cannot be handed to a userspace terminal. Milestone 400's handover maps the
  screen's physical range into a driver, which is right for a display adapter's BAR and is a hole
  for a framebuffer that is the kernel's own `.bss`. `user::boot_screen_terminal` refuses a screen
  inside the kernel image, so on the two boards the shell's prompt is on the serial line and the
  kernel's tour is what stays on the screen. Milestone 157's aperture will pass that test without
  anything being changed.
- `ramfb` is QEMU's and needs no cache maintenance, which is the one shortcut in
  `kernel/src/drivers/ramfb.rs` that would be wrong on silicon. The device reads guest RAM while the
  guest has it mapped cacheable; an emulator has no cache to be stale. Nothing else in this tree may
  copy that pattern.
