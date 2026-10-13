---
status: BUILT
raised: 2026-09-20
built: 2026-09-20
---
# 445. The screen check stops sampling and starts asking

The ruling is
[§199 (the screen check asks instead of sampling)](../decisions/0199-the-screen-check-asks-instead-of-sampling.md);
this block is what was built against it.

## The defect, measured

`cargo xtask uefi-boot` asserts the claim of milestone 243 (a machine with no serial port has no
way to say anything, and no gate can read it), by photographing the guest's framebuffer through
QEMU's monitor. It used to do
that by sampling: `screen_watch` polled `screendump` every 50 ms and had to catch the kernel's
marker on a screen that does not stay that way.

On 2026-09-20 a full `script/test` run caught zero rows and reported *"the tour was never
readable on the screen"*. The same leg, run alone a minute later, read 56. Under load the
dump-write-read-decode round trip stretches; the guest's window does not stretch with it, because it
closes in **guest** time. No host-side deadline widens it.

And the verdict was worse than the flake. `tour: None` is the same value a genuinely broken
framebuffer produces, so the message sent the reader after the loader's `LocateProtocol`, the pixel
order, the stride and the mapping surviving `mmu::init`, none of which was wrong.

## What calef chose, and what he refused

Three options were put to him on 2026-09-20, and §199 records the ruling in his own terms:

- A, a handshake, so the window closes when the host says it has seen the screen. **Chosen.**
- **B, an xtask-only mode where the handover simply does not clear.** Refused: it makes the
  screen-check boot differ from a real boot at exactly the moment under test.
- C, keep sampling and only fix the verdict's honesty. Not taken: sampling a transient state and
  hoping is rung four of `AGENTS.md`'s ladder, and a longer deadline does not help for the reason
  above.

## What was built

### The kernel holds the screen, when it is asked to

A boot-time knob, off by default, on the same command line the framebuffer description already
travels on (`machine_discovery::framebuffer::Framebuffer::KEY`, `screen=`). The new token is
`SCREEN_HOLD`, the bare word `screen-hold`. With it present:

1. `kernel::console::yield_screen` calls `hold_screen_for_host` before it takes the console lock.
2. That prints `boot_ladder::SCREEN_HELD` on the serial line and waits for one byte back.
3. `cargo xtask uefi-boot` sees the line in the transcript it is now streaming, takes its dump with
   the tour guaranteed to be on the framebuffer, decodes it, and writes a byte.
4. The kernel drains that byte and clears and hands over exactly as it does today.

A boot without the token is untouched: one relaxed load of an `AtomicBool` on a path taken once
per boot, no serial read, no added latency. The doc comment on `hold_screen_at_handover` says in as
many words that it is a debugging affordance.

The token rather than a build of the kernel, deliberately. The kernel `uefi-boot` boots is
byte-identical to the one on a stick; only the loader differs, by one word it writes. That is the
narrowest form of calef's refusal of option B that still lets a gate ask a question.

### The wait is bounded twice, and both bounds have a reason

A knob that can wedge a machine forever is a worse defect than the one it fixes.

- Ten seconds of scheduler ticks (`TICK_HZ` is 100 on all three architectures). The host's round
  trip is a monitor command, an asynchronous PPM write, a read and a glyph decode, about 50 ms idle.
  Ten seconds is two orders of magnitude of headroom for the loaded machine that broke the old gate,
  and short enough that a knob set by mistake on a bench is a pause somebody waits out.
- A flat poll count, paid down only while the tick counter has not yet moved. Ticks come from
  the timer interrupt, so a clock that has stopped would otherwise turn the first bound into no
  bound at all.

### Two things had to be found by building it, and both are corrections

A spinning guest starves the emulator's own monitor. The first working version polled the UART
with `core::hint::spin_loop`. Every `screendump` taken during the ten-second hold came back a file
that would not decode, and the gate failed for a reason with nothing to do with the kernel. Parking
the core with `arch::wait_for_interrupt` between polls fixed it outright, and the `clock_alive`
guard above is what makes parking safe. Worth carrying: anything in this tree that busy-waits inside
a guest while a host is trying to talk to QEMU has this failure available to it.

The tour is taller than the screen, so it scrolls. `UEFI_SCREEN_MARKER` was
`boot_ladder::BANNER`, the tour's *first* line, moved there hours earlier by milestone 243's second
lane on a measurement saying the tour "tops out at 98 non-blank rows" against a screen of 100, so
nothing scrolls. Held still and photographed, the OVMF console's first row is

```text
                0x00007ea8a000..0x00007eab4000  ram
```

the middle of the firmware memory map, with 95 rows below it and no banner anywhere. So what the old
sampler was catching was the banner early in the boot, before the scroll, which is a window at
the other end of the tour from the one its own comment described, and which is why a loaded machine
read zero: the first dump landed after the scroll rather than after the clear. The marker is the
self-test verdict again, the tour's tail, which the handshake makes deterministic.
`boot_ladder::BANNER` is now the *deep* marker, reported when a screen is tall enough to still hold
it and never on this one.

### The verdict distinguishes three outcomes

| what happened | what it says |
|---|---|
| held, tour on the screen | how many rows, and whether the tour scrolled |
| **held, tour not on the screen** | the framebuffer path broke, and it says the boot was stopped while the dump was taken, so timing is not available to blame |
| **never held** | the handshake did not reach the kernel: the `screen_hold` feature, the token, or a boot that never got that far. Not the framebuffer |

The middle row is what the milestone bought. The bottom row is what cost a reader an afternoon.

## Evidence, under the condition that broke it

Three consecutive `cargo xtask uefi-boot` runs on patagonia (8 cores) against fourteen detached
spinners, at one-minute load averages of 17, 32 and 38:

```text
uefi-boot: read 95 non-blank row(s) of the tour back off the framebuffer (its last page; ...), ending
uefi-boot: booted under OVMF from \EFI\BOOT\BOOTX64.EFI
```

Exactly 95 rows every time, which is the point: the reading is now a property of the framebuffer
rather than of the host's spare capacity.

## Architectural parity

§19 (architectural parity is a tenet; the targets are aarch64, riscv64, and x86_64) makes parity a
gate, so the answer is stated rather than implied.

The mechanism is arch-neutral. `yield_screen` consults the flag on all three architectures, the
wait is one piece of code, and both console UART drivers grew the receive half it needs: the NS16550
pair came out from behind `reboot_soak_test`, and the PL011 gained `rx_waiting` and `discard_rx` of
its own.

**Only `x86_64` arms it, and the other two have nothing to arm it for.** The race is the window
between the tour being painted and `yield_screen` clearing it. On aarch64 and riscv64 the only
screen is `kernel::screen`'s `ramfb`, whose pixels are the kernel's own `.bss`, and
`user::boot_screen_terminal` refuses to hand that to a userspace driver rather than grant a process
a window onto kernel statics. So `yield_screen` is never reached there, nothing clears the tour, and
`cargo xtask screen-boot` photographs a screen that will show the same thing an hour later.

**`script/boot-check` does not have this race either**, which §199 left to this lane and which was
worth checking rather than assuming: it asserts its rungs on the serial transcript, which is a stream and not a state, and it
takes no screendump at all.

That changes when milestone 157 (real display output on the board) gives those two a firmware
aperture outside the kernel image. The refusal stops firing, the handover starts happening, and the
window appears. What is needed then is a reader for `/chosen/bootargs`, which this kernel does not
parse today, and one call beside it. The scope note lives on `hold_screen_at_handover`, where the
next person meets it.

## BUGS

- **The gate's loader is not the stick's loader.** `cargo xtask uefi-boot` stages
  `target/esp-screen` with `screen_hold` on; `cargo xtask uefi-image` stages `target/esp` without
  it, and that is the one the bench procedure copies from. A stick made from the wrong directory
  waits ten seconds at the screen handover for a host that is not there. Three ESP directories now,
  for the reason `uefi_test_esp_dir` gave for the second: the alternative is a build that can reach
  a bench by accident.
- **Nothing gates the two directories apart.** The separation is a function name and two comments,
  which is rung three. A lint that knew which directory a `cp` came from would be guessing.
- The release byte is any byte, so a person typing at a held boot releases it. That is the
  intended behavior for somebody at a bench and is worth knowing before it surprises anyone.
- `hold_screen_for_host` takes the console lock once per poll, at 100 Hz while parked. It is a
  boot-time path taken once and never on a machine anybody runs, so the cost is a fact rather than a
  concern; it is recorded because the lock is an `IrqSafeMutex` and somebody will want to know.
- The deep marker is dead on QEMU's OVMF console. `boot_ladder::BANNER` cannot be on a 1280x800
  screen at the handover, so that branch of the report has never been exercised here. It fires on a
  taller screen or a shorter tour and is kept because a tour that stops scrolling should say so.

## Follow-on

- **Recorded.** The parity scope note is on `kernel::console::hold_screen_at_handover`, and names
  milestone 157 as what makes the other two architectures need the knob.
- **Recorded.** `notes/qemu.md`'s BUGS entry carries the starved-monitor finding, which is
  general.
- **Recorded.** The three-ESP-directory hazard, in `notes/x86-uefi-boot.md` beside the commands and
  in `uefi_loader/Cargo.toml` beside the feature.
- **Recorded.** Every name below is provisional (`AGENTS.md`: calef names these). `screen-hold` the token and `SCREEN_HOLD`
  the constant; `SCREEN_HELD` the announcement in `boot_ladder`; `hold_screen_at_handover` and
  `hold_screen_for_host` in `kernel::console`; `screen_hold` the `uefi_loader` feature;
  `uefi_screen_esp_dir` and `uefi_kernel` in `xtask`; `Pl011::rx_waiting` and `Pl011::discard_rx`.

## Index row

`cargo xtask uefi-boot` asserted milestone 243's claim by sampling a window that closes in guest
time, so a loaded machine read zero rows and blamed the framebuffer. The kernel is asked to hold the
screen instead: `screen-hold` on the boot command line makes `yield_screen` announce itself on the
serial line and wait up to ten seconds for a byte before it clears, bounded twice so no knob can
wedge a machine. Three runs at load 17, 32 and 38 each read exactly 95 rows. A spinning guest
starves QEMU's own monitor, so the wait parks the core, and the tour is taller than the screen and
scrolls, so the marker sits at its tail.

