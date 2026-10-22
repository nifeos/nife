# A machine with no serial port

Milestone 243 (a machine with no serial port). Every word nife had ever said, it said down a UART.
That covers the boot tour on all three architectures, the console server and the shell, the
kernel's fault reports, and every automated gate that reads any of them (`script/board-console`, the soak's heartbeat, `script/swish-check`,
`crates/serial_console`'s stage judging).

A commodity machine does not have one. xenon does, and that was chosen rather than lucky: milestone
87 picked a Dell with a C4PDJ serial module and a null modem. Meanwhile the same milestone made a
much larger fleet reachable without anyone noticing, because `\EFI\BOOT\BOOTX64.EFI` on a FAT32
stick is the removable-media fallback every UEFI firmware looks for with no configuration:

> if I can boot a nife system off of a USB drive, then that opens up a lot of different hardware in
> our house that I can use for testing: [a family member's] laptop, his desktop, cordoba, two
> MacBooks, [another family member's] desktop
>
> -- calef, 2026-09-03

Not one of those machines has a serial port. The arguments
behind each choice are in [`serial-less-output/`](serial-less-output/README.md).

## The two halves, and they are not equally hard

|  | a **human** watching a machine boot | a **gate** reading a machine |
|---|---|---|
| needs | pixels on a monitor, live | text, after the fact, judged by a program |
| has today | the framebuffer console below | a screendump under QEMU, and **nothing on real hardware** |
| fails when | the screen is not yet initialized | the machine is not a virtual one |

One mechanism serves both under QEMU and only one of them on a real machine, and pretending
otherwise would be the dishonest half of this note. Under OVMF a gate can ask the emulator for a
picture of the screen; on a real laptop nobody can. That gap is real, it is named in `BUGS` below,
and the follow-on proposals are where it goes.

## What was chosen: the firmware's framebuffer, carried across the handoff

UEFI's `EFI_GRAPHICS_OUTPUT_PROTOCOL` reports a *linear framebuffer*, and its address survives
`ExitBootServices`. That is the fact the whole design rests on. The aperture is a BAR on the display adapter, not firmware memory, so ending the boot phase
takes away the firmware's *console* and not the *display*. The pixels stay where they were and
anyone holding the address can keep writing to them.

So:

```text
  firmware ──GOP──► uefi_loader ──"screen=0x80000000,1280,800,5120,bgrx"──► kernel ──glyphs──► monitor
                         │                  in hvm_start_info's                 │
                         │                  cmdline_paddr                       │
                    machine_discovery::framebuffer  (one crate, both parties)   screen_console
```

Four pieces, and the split is the tree's usual one: what two programs agree on is a crate, the logic
is on the host, and only the address arithmetic is in the kernel.

| Piece | Where | What it is |
|---|---|---|
| the sentence | `crates/machine_discovery/src/framebuffer.rs` | `Framebuffer` and its one command-line token, **written** by the loader and **parsed** by the kernel, round-tripped in host tests |
| the question | `uefi_loader::find_screen` | one `LocateProtocol` call and three field reads |
| the painter | `crates/screen_console` | a cursor over a `&mut [u8]`, with `bitmap_font`'s glyphs |
| the seam | `kernel::console`, `arch::x86_64::machine::attach_screen` | tee `print!` to the screen; record the aperture so `mmu::init` carries the mapping |
| the gate | `serial_console::screen`, `cargo xtask uefi-boot` | a screendump, decoded back into text, judged by `serial_console::progress` |

The address rides PVH's existing command line (`cmdline_paddr`) as one `screen=` token, rather than
a field appended to Xen's `hvm_start_info`. That is cheap to change because the loader embeds the
kernel, so writer and reader ship as one file. The console is `ScreenConsole`, a cursor and a
geometry, not `video_terminal`: the thing being reported is often why the machine is broken, so it
wants the least state that could work. They share the font, which is what makes the gate below
possible. [`framebuffer-handoff-choices.md`](serial-less-output/framebuffer-handoff-choices.md) has both arguments.

## What the alternatives cost, priced rather than argued

The block listed candidates and endorsed none. Keeping the firmware's console alive is not a
candidate: boot services *are* that console. A postmortem to the boot medium is the gate's real
answer on hardware, and it needs a USB mass-storage driver nife does not have. A network console
cannot report a network failure, and a photograph is not a screendump. So the two halves want
different mechanisms, and the framebuffer is the human's answer and the right first increment.
[`the-alternatives.md`](serial-less-output/the-alternatives.md) has each priced against both
halves.

## The gate: a program reading a screen

`serial_console::screen` turns a screendump back into text. It is not optical character recognition
and the difference is the whole reason it is trustworthy: `bitmap_font` is a constant table of
monochrome 7x8 glyphs, so a cell either matches a glyph bit for bit or matches nothing. There is no
threshold to tune and no confidence to report.

That gives `cargo xtask uefi-boot` an assertion nothing else in this tree could make. The serial
transcript it already checked would read identically if the screen were black. The screen check
fails if the loader's `LocateProtocol` regressed, if the pixel order flipped, if the stride were taken as
the width, or if `mmu::init` stopped carrying the aperture's mapping.

Measured 2026-09-04 under OVMF on QEMU 11.x with `-display none`, it read 96 non-blank rows of text
back off the framebuffer, ending at the tour's last lines; the transcript is in
[`framebuffer-handoff-choices.md`](serial-less-output/framebuffer-handoff-choices.md#the-gates-first-green-run-in-full).

`-display none` suppresses the host window, not the emulated adapter, which is why any of this
works headlessly: OVMF finds a GOP here for the same reason a real machine's firmware finds one.

## The shell on the screen, too

Since milestone 400 (the shell on the firmware's screen) the shell's prompt reaches the screen too,
beside the serial console rather than instead of it. The console server tees every byte to a
userspace `display_terminal`, which `framebuffer_driver` copies into the aperture after the kernel
yields the screen once. [`the-shell-on-the-screen.md`](serial-less-output/the-shell-on-the-screen.md)
has the pipeline and the gate.

## The boards' screen: `ramfb`, and why it is the other way round

Built 2026-09-19. QEMU's `virt` boards are entered from `-kernel` with nothing configured, so there
is no framebuffer to discover. What they can present is `ramfb`, which inverts the arrangement: the
guest owns the pixels in its own `.bss` and hands the device their address over `fw_cfg`, and QEMU
scans them out continuously. `cargo xtask screen-boot <arch>` (name provisional), `uefi-boot`'s twin,
reads the aarch64 and riscv64 tours back off the screen inside `script/test`'s board legs. When the
screen is blank, its second line says whether the serial transcript reached the same marker, which
separates a broken screen from a machine that did not boot. `ramfb` is QEMU's and proves nothing
about the VisionFive 2's display; that is milestone 157 (real display output on the board).
[`ramfb.md`](serial-less-output/ramfb.md) has the pieces, the transcripts, and what the first red CI
run settled.

## The bench: booting a serial-less machine

This has not been done. Everything above is QEMU with real firmware in the loop, which is as far
as a lane can get; the machines are calef's. This section is the procedure, written to be followed
rather than interpreted, and it is deliberately the same shape as `notes/x86-uefi-boot.md`'s.

### What you need

- Any x86_64 machine with UEFI firmware and a monitor. From the fleet calef named: a family laptop,
  two family desktops, cordoba, or an Intel MacBook. The Apple Silicon MacBooks are
  not in this fleet: they boot non-Apple kernels legitimately (Asahi's permissive-security mode)
  but they have no UEFI, so a `BOOTX64.EFI` stick will not start one. That is a port, not a boot.
- A USB stick, formatted FAT32 with a GPT or MBR partition table. macOS Disk Utility: *Erase*,
  format MS-DOS (FAT), scheme GUID Partition Map.
- No serial cable, no adapter, nothing else. That is the milestone.

### Build and copy

```console
$ cd /path/to/nife
$ cargo xtask uefi-image
wrote .../target/esp/EFI/BOOT/BOOTX64.EFI (the loader, the kernel and the archive)

$ mkdir -p /Volumes/NIFE/EFI/BOOT
$ cp target/esp/EFI/BOOT/BOOTX64.EFI /Volumes/NIFE/EFI/BOOT/BOOTX64.EFI
$ diskutil eject /Volumes/NIFE
```

One file. The path and the capitalization are the interface.

### Firmware settings

1. Secure Boot: off. This image is unsigned and nothing in this tree signs it. Expect to have to
   do this; a Secure Boot machine refuses the stick with a security-violation message and no other
   explanation.
2. Boot from UEFI, not Legacy/CSM. Legacy boot looks for an MBR boot sector, which this stick
   does not have.
3. Leave everything else alone on the first attempt. A bring-up has enough variables.

The one-time boot menu is usually F12; on a Mac, hold Option at the chime.

### What you should see, in order

Everything is on the monitor. Nothing else is connected.

1. `nife uefi_loader: milestone 87 (x86_64 bare-metal)`, then three or four more `uefi_loader:` lines. This is the
   firmware's own console, so seeing it proves the firmware found the stick, Secure Boot did not
   refuse it, and the loader started. It prints the screen it found.
2. The screen clears, which is the kernel's console arming.
3. The boot tour, beginning `nife on x86_64 (long mode, ring 0, 4-level paging)`, with a
   `screen      :` line naming the geometry.
4. The screen clears a second time and the shell's banner and `$ ` fill it at the tour's
   scale (137x67 on 1920x1080). A USB keyboard types into the shell under QEMU on all three
   architectures (milestone 242 (USB host and HID)), and nobody has yet typed on a real one. If
   the keyboard answers here, that is the first record; `notes/usb.md` is the page.

Photograph the screen at that point. That is the record, and it is the only record this machine
can produce today (see `BUGS`).

### Triage

Each row rules out everything above it.

| What you see | What it means | What to do |
|---|---|---|
| Nothing at all, machine boots its own OS | the firmware did not see the stick | check `/EFI/BOOT/BOOTX64.EFI`, the case, and FAT32 rather than exFAT |
| "Security violation", or the stick is skipped | Secure Boot | turn it off |
| `nife uefi_loader:` then a message and a halt | the loader refused, and it says why | every string is a literal in `uefi_loader/src/main.rs` |
| `uefi_loader: no linear framebuffer` | the adapter is `PixelBltOnly` or `PixelBitMask` | this machine cannot use this milestone; record the model, it is the first one |
| `wanted 0x...` and `in the way:` lines | the firmware will not give up the kernel's 32 MiB load range | milestone 195's `BUGS`: the image is not physically relocatable. Record the descriptors printed |
| The screen clears and shows `nife loader: firmware released, entering the kernel.` and nothing more | the kernel never reached its first statement: the trampoline, `boot.s`, the page tables or the long-mode jump | build with no archive (`NIFE_UEFI_INITIAL_RAMDISK` unset); a triple fault here reboots instead, so a *stuck* banner is a hang rather than a fault |
| The loader's banner clears and then **nothing** | the kernel armed its console and died after | the window below `attach_screen`; on a machine with a serial port the transcript is the diagnosis |
| Text, but sheared or in the wrong colors | the stride or the pixel order | the `screen :` line says what the loader read; compare against the machine's real mode |
| The machine reboots in a loop | a triple fault | build with no archive (`NIFE_UEFI_INITIAL_RAMDISK` unset) to halve what is copied |
| The tour, the second clear, then **nothing** | the userspace terminal took the screen and drew nothing | milestone 400's defect; on a machine with a serial port its two `screen    :` lines say whether the driver came up |

## The window between the firmware and the kernel, and what can be bought in it

This is problem 3 of the block's three, early boot, and it cannot be narrated. From
`ExitBootServices` to the kernel's own `attach_screen` there is no console, and the code in between
is a 32-bit instruction stream with no idea where the screen is and no IDT. It can be bounded, and
since 2026-09-19 it is. `uefi_loader`'s last act, after `ExitBootServices` and before the jump, is
to clear the screen and write two lines into the framebuffer:

```text
nife loader: firmware released, entering the kernel.
If this line is still here, the kernel stopped before its console came up.
```

So the five things a person at a monitor can be looking at are distinguishable rather than four of
them being "black":

| The screen shows | What ran, and what did not |
|---|---|
| the firmware's own splash or menu, untouched | the firmware never started this loader |
| `uefi_loader:` lines and a halt, over the firmware's console | the loader started and refused, and said why |
| the two lines above, still there | the kernel never reached its first statement |
| black | the kernel armed its console and died after |
| the boot tour | the window is behind us |

The banner was proved by the halted-loader experiment: with a temporary halt in place of the jump,
`cargo xtask uefi-boot`'s screendump reads those two lines in the top sixteen pixel rows. The two
boards get nothing equivalent, because nothing runs before their kernel to paint one.
[`the-early-boot-window.md`](serial-less-output/the-early-boot-window.md) has why the window cannot
speak, the experiment, and the boards' reason in full.

## BUGS

- **Nothing here has run on real silicon.** It is proved on the host and under OVMF. A framebuffer
  that works under QEMU's emulated adapter is not a framebuffer that works on a real laptop. The
  aperture may be above 4 GiB (this loader does not check, see below), the mode may be `PixelBitMask`,
  and the firmware may hand over a mode the monitor is not showing.
- **A gate cannot read a real machine.** The screendump path is QEMU's. On the fleet, the record is a
  photograph and a person, which is exactly the state milestone 216 got the VisionFive 2 *out* of.
  Postmortem to the boot medium is the answer and it needs a USB mass-storage driver; see the
  proposals.
- **Early boot is bounded, not narrated.** The loader's handoff banner (above) makes "the kernel
  never started" a distinguishable outcome; it does not make the window *say* anything about what
  went wrong there, and nothing can, for the reasons in that section. A fault in `boot.s` is still a
  triple fault and a reset, and what a person sees afterwards is the firmware starting over.
- **The banner is unobservable in a healthy boot**, which makes it a diagnostic nothing routinely
  exercises. `attach_screen` clears within milliseconds of it being painted, so no gate can assert it
  on a working machine. What is gated is that the loader still compiles it and that `uefi-boot` is
  green, and what proves it is the halted-loader experiment recorded above. Rung four, honestly.
- **The framebuffer must be below 4 GiB**, because everything the loader hands the kernel has to be
  nameable by a 32-bit instruction stream running with paging off. `uefi_loader` does not check
  that for the framebuffer: `find_screen` in `uefi_loader/src/arch/x86_64/mod.rs` bounds the span
  by the firmware's `framebuffer_size` and never the base (read 2026-10-11 UTC). The aperture is a
  BAR the firmware placed, and a machine that puts it high would be handing the kernel an address
  its early direct map does not cover. Nothing has been seen doing this and nothing would notice if
  one did.
- **The screen shows a grid instead of text on xenon**, the one real machine that has run it.
  [serial-less-output/the-xenon-grid.md](serial-less-output/the-xenon-grid.md) has what rules out
  scrolling and the halt path, and the experiment that would measure a pitch mismatch.
- **Only ASCII appears.** A byte outside `0x20..0x7f` is drawn as a space, so `§121` in the tour's
  last line reads as two blanks and a number on the screen. The serial console shows it correctly and
  the two transcripts therefore differ by exactly the non-ASCII characters in them.
- **The other two architectures have a screen under the emulator and none on silicon.** The
  arch-neutral halves (`machine_discovery::framebuffer`, `screen_console`) were written for exactly
  this and needed no change: what was missing was the discovery, and on QEMU's `virt` boards there is
  nothing to discover, because nothing lit a display. `ramfb` is what those boards can present
  instead. On the VisionFive 2 there is no `ramfb` and
  the answer is milestone 157's U-Boot `simple-framebuffer` handoff. Rule 5's scope note, recorded
  here: the *console* has parity, the *discovery* does not, and the gap is one board's firmware.
- `ramfb` has three limits of its own, kept beside it in [`ramfb.md`](serial-less-output/ramfb.md).
  It costs 1.9 MB of `.bss` on every board boot, its screen cannot be handed to a userspace
  terminal, and it skips cache maintenance, a shortcut that would be wrong on silicon.
