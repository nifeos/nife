---
status: PARTIAL
raised: 2026-09-19
milestone_dependencies: none
decision_dependencies: none
machine_requirements: an x86_64 UEFI machine with a display attached
specific_machine: xenon (the same stick, with a monitor attached)
needs_person: yes
---
# 400. The shell on the firmware's screen, because on a PC with no serial port the prompt went nowhere

Built 2026-09-19 on `milestone/400-the-shell-on-the-firmware-screen` (pull request
#985): under OVMF the shell's prompt is on the screen, a command typed on the serial line answers
there, and `cargo xtask uefi-boot` reads both back off the framebuffer. What remains is the bench
half of the exit criterion, the same stick on xenon with a monitor, which only a person at xenon
can do (the procedure is below). *(Number provisional until the merge queue lands it; this block
was `design/roadmap/proposals/the-shell-on-the-firmware-screen.md`, found by milestone 198 (package)'s rungs
lane, and that file is deleted by the commit that wrote this one.)*

calef at xenon with a monitor attached; everything a lane can do is done.

In brief. Milestone 243 (machine) put the *kernel's* boot tour on a UEFI machine's framebuffer. Since
milestone 299 the console is a userspace process that writes COM1 by port I/O and nothing else, so on
a PC with no serial port (every laptop, most desktops) the tour scrolled past and the prompt was
never seen. This is rung 1b of milestone 198's trivial install (DECISIONS §157 (trivial)): the output half of
"a PC with a monitor and a keyboard, no serial cable". Milestone 242 (USB host and HID) is the input
half; keystrokes stay on the serial line until it lands.

## What was built

```text
  swish ─► line_editor ─► console ──out──► COM1                       (unchanged: every gate reads this)
                             │
                             └──OPERATION_WRITE──► display_terminal ──FLUSH──► framebuffer_driver ──copy──► the firmware's aperture
                                             (unchanged)                 (new)
```

| Piece | Where | What it is |
|---|---|---|
| A driver behind the framebuffer contract | `components/src/framebuffer_driver.rs` (name provisional) | serves `graphics_contract`'s `INFO`/`FLUSH` over a screen the firmware already set up: a flush is a copy of the damage rectangle from the surface (RAM) into the aperture, at the screen's stride and in its byte order |
| The copy's arithmetic, host-tested | `screen_console::Aperture` (name provisional) | the surface clipped to the screen, the span a driver needs mapped, two words that carry it across a spawn, and the copy; five host tests, one doctest |
| The kernel's wiring | `display_service::start_screen_terminal`, `user::boot_screen_terminal`, `user::run_with_device_run` and `user::DeviceRun` (all provisional) | spawns the driver and `display_terminal`, and grants the progenitor slots 10 and 11 with slot 12 empty |
| The handover | `console::yield_screen`, `console::reclaim_screen_for_panic` | see below |
| The tee | `components/src/console.rs`'s `MODE_SCREEN`, `crates/system_initializer`'s `has_screen` | the console server writes every byte to COM1 and then to the terminal |
| The gate | `cargo xtask uefi-boot`'s `screen_watch` | three stages read off the screen: the tour, the prompt, and the answer to a serial command |
| Arch-neutral proof of the driver | `display_tests::a_firmware_screen_shows_the_terminal_through_the_framebuffer_driver` | a pretend screen in RAM the OVMF gate cannot produce (21x352: narrower than the surface, taller, padded, rgbx, not page-aligned); runs on all three architectures |

Measured under OVMF, 2026-09-19 (`cargo xtask uefi-boot`, inside `script/test`'s x86_64 leg,
exit 0; abridged to the lines this block is about):

```text
  screen    : handed to a userspace terminal; the kernel writes the UART alone
  screen    : 924x344 pixels of it served by framebuffer_driver, a 132x43 terminal on it
uefi-boot: read 97 non-blank row(s) of the tour back off the framebuffer, ending
uefi-boot:   |   the kernel is fine.
uefi-boot:   |   userspace   : a process built from untyped ran at cpl 3 and sent 0x1610004 on a granted cap
uefi-boot:   |                 thread 12884901891 died at pc 0x400005 on addr 0xa5000?
uefi-boot: the shell is on the screen, and `echo typed on the wire` typed on the serial line answered there:
uefi-boot:   | progenitor: construction budget dropped; retype answers NoSuchSlot
uefi-boot:   | ...
uefi-boot:   | $ echo typed on the wire
uefi-boot:   | typed on the wire
uefi-boot:   | $ ?
uefi-boot: booted under OVMF from \EFI\BOOT\BOOTX64.EFI
```

`serial_console::screen` reads a cell that matches no glyph as `?` rather than refusing the picture,
so the tour's last row ends in `?` (the screendump caught the kernel mid-line) and so does the last
prompt (the cursor block).

## The decisions this made, and what lost

S1 was the proposal's recommendation, and the premise check changed where its client sits. The
proposal priced three shapes: S1, a pre-set-buffer driver behind the framebuffer contract serving
`display_terminal`; S2, the console server painting pixels itself with `screen_console`; S3, the
kernel painting for the shell. S1 was built. It is the one terminal engine for every screen (a
full VT, so `line_editor`'s editing escapes land correctly, which S2's `screen_console` has no parser
for), and the driver is the one milestone 157 describes for the boards. S3 stays refused on §121's
reversal. Would S1 still win if S2 cost the same? Yes, for those two reasons; this was not decided on
effort.

What the proposal assumed and the code did not support: that S1 plugs into milestone 192 (keyboard)'s
graphical seam (`boot_graphical_terminal`, with the UART as the keystroke source). It cannot, without
breaking the constraint the brief put first. A graphical boot has no console server:
`line_editor` prints to `display_terminal` alone, so the shell's output would leave COM1, and every
xenon gate and serial transcript reads COM1. So the terminal's client is the console server
rather than `line_editor`: it writes each byte to the UART, then hands the same bytes to the terminal
with one `OPERATION_WRITE`. The alternatives, and why each lost:

- `line_editor` with two outputs (the display terminal and the console). Two output paths in the
  line discipline where one exists, and a second capability pair in the program whose job is editing
  rather than delivery. Lost on elegance: the console server is already the thing whose job is "put
  this stream on the machine's output devices", and the kernel's own console (`kernel/src/console.rs`,
  "both, not either") is the precedent for one console feeding two surfaces.
- The graphical path as it is, serial output dropped. Lost outright: it breaks xenon's gates.
- `display_terminal` or the driver holding COM1 too. Lost: a device in a program whose authority
  today is pixels and an endpoint.

The console server gains no device for this, one endpoint and one page, the authority any
program printing to that terminal holds, which is also why this is not S2 wearing another name.

How the aperture reaches the driver: a spawn-time mapping, not a capability. Rule 2 is honored
(nothing reaches into a kernel global; the geometry arrives in its argument registers and the pages
are mapped before `_start`). A capability was not available: `DeviceFrame` names one page and a
screen is a thousand, `PageFrame` names RAM the allocator owns and would free, and a `DeviceFrame`
run would be new syscall surface, which is an architect's. The NVMe server of milestone 261 (the
NVMe driver leaves the kernel) and the TRNG of milestone 159 (a real hardware entropy source) made
the same choice for the same reason. It has a least-authority upside: the driver holds no name
for the screen, so it cannot map it twice, delegate it, or hand it on. Only the rows the surface
can reach are mapped (`Aperture::span`), not the whole screen. `user::DeviceRun` exists because
the kernel has no heap to build a thousand-entry `Mapping` slice in; it is a kernel-internal struct,
not ABI.

The handover between the kernel's tee and the userspace terminal. Two painters on one aperture is
the defect milestone 230 found on the UART, with pixels in place of bytes. So:

1. The kernel console's screen carries a `Painter` state, `Kernel` or `Terminal`, and `print!`
   paints only while it says `Kernel`.
2. `console::yield_screen` is the only way to `Terminal`, it is taken once (a second caller gets
   `None`, so two userspace painters are unrepresentable rather than unlikely). It clears the screen
   under the same lock every `print!` takes, and it returns the `Framebuffer` the driver needs. The
   driver is spawned only after it returns, so there is no instant with two painters.
3. After the handover the kernel's own lines go to the UART alone, including the two announcing it
   (a line painted just before the yield would be cleared by it).
4. A panic takes the screen back (`console::reclaim_screen_for_panic`, first thing in the panic
   handler after the lock is broken): clear, and paint the panic. On a machine with no serial port the
   screen is the only place a panic can be read. The cost is recorded rather than avoided: the panic
   halts only its own core, so another core's flush can paint over the top of the panic text.

This is the shape `proposals/kernel-console-arbitration.md` (the kernel and the console server on
one UART, a `DECISION`) lists as one of its options ("a claim the server takes and the kernel respects
except in a panic"), applied to the screen only. It does not decide that proposal: the UART is
still written by both, and that is still calef's fork.

Why the kernel clears on yield rather than the driver. The driver maps only the rows its surface
can reach, so it cannot clear the rest of a taller screen; the kernel already has the whole aperture
mapped and is the party whose text is being removed.

The FLUSH hang milestone 177 (wire) recorded does not occur here. `notes/framebuffer-contract.md`'s BUGS
records a second `FLUSH` through the real boot's virtio-gpu driver that never returns, not
root-caused, with the driver's completion interrupt as the best hypothesis. Every prompt, echo and
answer on this path is a further flush through the same `display_terminal`, and all of them return.
That is evidence (not proof) that the hang lives in `gpu_driver`'s interrupt handling and not in the
contract or the terminal. Recorded there too.

## Parity (rule 5)

**x86_64 and UEFI only, as a scope note rather than a port.** Everything added is arch-neutral: the
driver, `Aperture`, the tee, the handover and `boot_screen_terminal` compile and run on all three,
and the driver's arithmetic is exercised on all three by the RAM-screen test. What aarch64 and
riscv64 lack is a screen the kernel console was told about, which is milestone 157's U-Boot
`simple-framebuffer` discovery; when that calls `console::attach_screen`, this path needs no change.

## The bench step: xenon, with a monitor

**Not done by this lane; no xenon result is claimed.** It is also the first real-hardware reading
of milestone 243's open xenon defect (below).

1. `cargo xtask uefi-image`, then copy `target/esp/EFI/BOOT/BOOTX64.EFI` to a FAT32 stick as
   `/EFI/BOOT/BOOTX64.EFI` (`notes/x86-uefi-boot.md`'s bench section; Secure Boot off).
2. Boot xenon from the stick with the **monitor attached and the serial cable connected**, and
   `script/board-console` (or any terminal at 115200) on patagonia.
3. What the monitor should show, in order: the loader's lines; the screen clearing and the
   kernel's tour; the screen clearing a second time. Then, in the **top-left corner only** (132x43
   cells, 924x344 pixels, whatever the panel's size), the shell's banner and `$ `.
4. What the serial console should show: the same tour, then two `screen    :` lines (the UART
   alone has them), and the same banner and `$ `. Type `echo typed on the wire` on the serial
   console. The monitor should show `$ echo typed on the wire`, then `typed on the wire`, then a
   fresh `$ `.
5. Photograph the monitor after step 4 and file it with the serial log under `bench/`.

**Reading the result against milestone 243's grid.** On 2026-09-04 and 2026-09-17 xenon's monitor
showed a regular leaning grid instead of the kernel's tour (`crates/screen_console`'s BUGS, a pitch
disagreement suspected and not measured). This path writes the **same** aperture at the **same**
stride through a **different** mapping: 4 KiB user pages, uncacheable, where the kernel writes
through its direct map. So:

| Tour | Shell | Reading |
|---|---|---|
| grid | grid | the geometry (the stride) is wrong, as 243 suspects; the mapping is not the cause |
| grid | readable | the kernel's mapping of the aperture is the cause, not the stride |
| readable | readable | this rung is done on xenon; 243's grid did not reproduce |
| readable | missing or garbled | this milestone's defect: the serial log's `screen    :` lines say whether the driver came up |

Also worth a stopwatch: how long `ls` of a full directory takes to scroll on the monitor, since every
scroll is a full-surface copy through an uncacheable mapping (BUGS).

## Follow-on

- **Outstanding.** The xenon bench step above; `HARDWARE`, calef's.
- **Outstanding.** A second PC (milestone 198's rung 1d), which is the same stick on a fleet machine
  and turns a claim about one Dell into a claim about PCs.
- **Recorded.** The terminal is 132x43 in the top-left corner on any screen: the surface is the
  contract's compile-time size (`components/src/framebuffer_driver.rs`'s BUGS).
- **Recorded.** Every flush is a CPU copy through an uncacheable mapping, not measured on silicon (the
  driver's BUGS; milestone 243's PAT entry).
- **Recorded.** After the handover the kernel's own output (a user fault report, the progenitor's exit
  line) reaches the UART only; a panic reclaims the screen and can be overdrawn by another core
  (`kernel/src/console.rs`, `reclaim_screen_for_panic`'s doc).
- **Milestone 445.** The tour stage of `uefi-boot` had a window: the kernel painted the tour only
  until the handover cleared it, and the watcher polled about every 200 ms. Since 2026-09-20,
  milestone 445 (the screen check stops sampling and starts asking) has the kernel hold the screen
  until the watcher has dumped it, so the stage is a handshake rather than a race (`xtask`'s
  `screen_watch`, `console::hold_screen_for_host`).
- **Recorded.** On an x86 boot that had a virtio-gpu, `boot_graphical_terminal` spawned the virtio
  stack and then returned `None`, and this path took the firmware screen as well. Gone since
  milestone 600 (the graphical terminal stack is built in userspace) retired that function on
  2026-09-26, and milestone 632 (graphics on demand) took graphics off the boot. The kernel now
  grants the gpu or the firmware-screen terminal, never both (`crates/system_initializer`'s
  `boot_sequence.rs`). Corrected 2026-10-12 (UTC).
- **Milestone 182.** That is milestone 182 (x86_64's own interactive-boot entry point).
  `script/swish-check` still has no x86_64 leg, and its `--arch` refusal says x86
  has no prompt, which milestone 299 made untrue. That is milestone 182's third leg, already
  tracked there.
- **Milestone 377.** Two screendump decoders. It was the existing proposal
  `one-screendump-decoder-not-two` when this block cited it and milestone 433's drain numbered it
  the same day, in another session.

## BUGS

- Nothing here has run on real silicon. It is proved under OVMF and, for the driver's arithmetic,
  on a RAM screen on all three architectures.
- The serial console now waits for the screen. The console server acknowledges a write only
  after the terminal has drawn it, so a wedged screen terminal would stall the serial console too
  (the UART gets each write's bytes first, so the wire still shows the line that stalled). Recorded
  in `components/src/console.rs`; decoupling it needs a second thread or milestone 151's
  notification objects.
  **Measured by milestone 182 (2026-09-19), and it is slow before it is ever wedged.** The x86_64
  `swish-check` leg types the same 60 lines the other legs type: 321 s under OVMF against 7 s on
  aarch64 and riscv64, slowest line 24.7 s (`xargs caps rm globmany/m-*.txt`) against 0.6 s, and
  90 s over PVH with no screen. So nearly all of it is the serial console waiting for the screen:
  `display_terminal` painting (the whole surface on each scroll) and `framebuffer_driver` copying
  into an uncacheable aperture, debug builds under TCG. That contradicts `framebuffer_driver`'s
  own BUGS line ("free under QEMU"), left unedited there only because pull request #991 is in that
  file. It turned 182's pull request red in CI on a slower runner, and 182 now carries a 90 s
  per-line bound for that leg, measured and recorded at `SWISH_CHECK_X86_LINE_SECS` in
  `xtask/src/main.rs`. A real PC pays native stores, not emulated ones; not measured on silicon.
  The paint lane (2026-09-30, PR #1471) took that measurement as its mandate. It built the three
  fixes the paragraph names: the scroll that moves pixels instead of re-rendering them, qword
  aperture stores, and the console's coalescing window (the notification-object retrofit this
  bullet said was missing). The ack's meaning changed with it: the wire draws immediately, the
  screen within one 20 ms window (`components/src/console.rs`'s module doc records the trade). The
  90 s bound and this paragraph's wall-clock figures are the *before*; the lane's PR carries the
  after once the legs have run, and this bullet stays as the record of what it cost before
  anybody fixed it. The lane's own block, the provisional roadmap 623, carries the work's shape;
  this bullet keeps only the before-measurement and the pointer.
- Keystrokes are the serial line's. A PC with no serial port shows the prompt and cannot type at
  it until milestone 242.
- **Whether the firmware left the screen in a mode the monitor is showing** is the firmware's choice
  (`uefi_loader` never calls `SetMode`), and a stranger's machine may differ from xenon.

## Index row

the shell's prompt and output on a UEFI PC's monitor, beside the serial console, through a second
driver behind the framebuffer contract. Proved under OVMF by a gate that types on the serial line and
reads the answer off the screen, and waiting on xenon with a monitor
