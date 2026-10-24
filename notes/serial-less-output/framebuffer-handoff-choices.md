# Two design choices in the framebuffer handoff

*Name: ratified 2026-10-18 (calef: "Approve all five").*

An appendix to [`notes/serial-less-output.md`](../serial-less-output.md), which holds what was
chosen: the firmware's framebuffer, carried across the handoff. This file holds why the address
rides PVH's command line rather than a new structure field, and why the kernel's screen console is
not `video_terminal`, and the gate's first green run in full.

### Why it rides a command line and not a new structure field

The x86_64 handoff is PVH's `hvm_start_info`, and PVH already carries a command line:
`cmdline_paddr` at offset 24, which `machine_discovery::x86_64::BootInfo` has decoded since milestone
87 and which nothing had ever read. `uefi_loader`'s own `BUGS` called that a gap in as many words:
*"there is nowhere yet for a boot argument to come from or go to."*

The alternative was appending a field to `hvm_start_info`. That structure is Xen's, versioned by
Xen, and a field added below the last one is a fork of somebody else's layout that looks exactly like
the real thing to whoever reads it next. A `key=value` in the field the format already provides for
exactly this is smaller, more reversible, and what Linux does with `video=`.

Is it a wire format, and therefore calef's? The tenets say anything two programs agree on is
expensive. This one is unusually cheap and the reason is structural rather than an argument for
leniency: `uefi_loader` embeds the kernel inside itself (`uefi_loader/build.rs`), so the writer
and the reader ship as one file and are rebuilt together. There is no version of this system in which
one side has the new spelling and the other does not. The token is recorded here and in the crate,
and renaming it costs one commit.

### Why the console is not `video_terminal`

`video_terminal::Vt` is the real terminal and it is the right engine for milestone 177's interactive
one. It is deliberately not this, for three reasons that all point the same way:

- It is a value of several hundred kilobytes (its own documentation warns readers off putting one
  on a stack), which in a kernel means a `.bss` static of that size on every architecture for a
  diagnostic path.
- It would put an escape-sequence parser in the TCB, over bytes, when the kernel's own `println!`
  emits no escape sequences to parse.
- The thing being reported is often the reason the machine is broken. That is the block's own
  constraint, and it argues for the console with the least state that could work. `ScreenConsole`
  holds a cursor and a geometry: five `u32`s and no buffer.

What *is* shared is the font, so the letters on an early boot screen and the letters in the
graphical terminal are the same letters, which is what makes the gate below possible.

### The gate's first green run, in full

Measured, 2026-09-04, OVMF on QEMU 11.x, `-display none`:

```console
$ cargo xtask uefi-boot
nife uefi_loader: milestone 87
uefi_loader: screen at 0x0000000080000000..0x00000000803e8000
uefi_loader:   1280x800, stride 5120, bgrx
uefi_loader: kernel placed, exiting boot services

nife on x86_64 (long mode, ring 0, 4-level paging)
  screen      : 1280x800 bgrx at 0x80000000, 182x100 cells (boot cmdline)
  ...
nife x86_64: boot complete, halting.
uefi-boot: read 96 non-blank row(s) of text back off the framebuffer, ending
uefi-boot:   |   next        : real ELF user programs (user_mode_runtime has no x86_64 arms), ...
uefi-boot:   | nife x86_64: boot complete, halting.
uefi-boot:   |   capability slots: 2 of 24 at peak
uefi-boot: booted under OVMF from \EFI\BOOT\BOOTX64.EFI
```
