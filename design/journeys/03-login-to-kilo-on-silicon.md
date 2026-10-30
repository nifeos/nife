---
status: PARTIAL
raised: 2026-08-30
---
# 3. The same story, on real silicon, on all three architectures

calef, 2026-08-30. Journey 1 (login to `kilo`) told end to end, with no emulator anywhere: boot a
board, log in, land in `swish` on a terminal worth using, create and edit a file with `kilo`. On the
VisionFive 2, on the Dell OptiPlex 7050 Micro, and on the Jetson TX1.

**This is not a follow-up to journey 1. It is the experiment journey 1 rehearses.**
design/fatal-risks/README.md lists nine things that would kill this project if true, and this journey is the
decisive test of two of them at once: risk 9 (the HAL is a fiction and each architecture costs a
restructure) and most of risk 6 (a capability-confined driver cannot drive real hardware). Journey 1
proves the software composes. Only this proves the system exists.

## The story's own bar

calef's bar for journey 1 applies unchanged: *"I want a full size usable terminal."* What this
journey adds is that the pixels are on a monitor, the machine is on a desk, and nothing in the path
is emulated.

| step | milestone | decision | what this step needs |
|---|---|---|---|
| 1 | 177 | | journey 1's own remaining gap: attach the GPU/keyboard devices to the real interactive boot and swap `console`/`input` for `display_terminal`/`compositor`. On x86_64 this also needs a real interactive-boot entry point, which does not exist on that architecture at all |
| 2 | 49 | | `login` wired into the real interactive boot |
| 3 | 87 | | **x86_64 first light**: the OptiPlex prints a byte over its serial port. The machine, the C4PDJ serial module and the RS-232 chain have been installed since 2026-08-23; nothing has ever been booted on it |
| 4 | 161 | | the x86_64 kernel port far enough to reach a console prompt, which is what step 3 is downstream of |
| 5 | 166 | | one boot orchestrator with a consistent meaning across architectures, reached three inconsistent ways today |
| 6 | 16 | | riscv64 silicon: the board has booted the full tour on three harts since 2026-08-14; what remains of 16a is the on-board test-suite exit and the DTB-driven UART IRQ |
| 7 | 157 | | **real display output**: U-Boot's `simple-framebuffer` handoff, so there are pixels on a monitor without a mode-setting driver. Gate: HARDWARE, verifiable only on a real board |
| 8 | 192 | | **a keyboard**: nothing in this system can read a keystroke on real hardware, because milestone 29's driver is virtio-input, a QEMU device. Discovered by tracing this journey. **Decided 2026-08-30**: option A first (keystrokes over the board's own UART, display on its framebuffer, no new driver) so the rest of this journey can be exercised on real hardware; but **192 is not done until a keyboard is plugged into the machine**, which is option B, so this step gates the journey's own completion |
| 9 | 159 | | RISC-V hardware entropy for the login step. Milestone 162 (RDSEED and RNDRRS) already covers x86_64 and aarch64; §120's virtio-rng stopgap is QEMU-only and does not exist on a board |
| 10 | 127 | | aarch64 silicon: the Jetson TX1, bought 2026-08-15. Bought for the seL4 comparison, and it is also the only aarch64 board this project has. Corrected 2026-10-06: not in hand; the seller shipped a TK1, which is going back (`notes/bench-runbook.md`) |
| 11 | 169 | | `kilo` and the raw-keystroke input primitive |
| 12 | 142 | | a full-size terminal: the 924x344 scanout and the 132x43 grid |

## What tracing it found, which is why the directory exists

**Nothing in nife can read a keypress on real hardware, and no milestone owned that.** A search of
the whole roadmap for `usb`, `xhci`, `hid` and `ps/2` returns nothing. Milestone 29's keyboard is
virtio-input, a QEMU device that none of the three boards has, and every graphical story in the
project has quietly assumed it. Milestone 192 (a keyboard on real silicon) is the block that now
owns it, and its fork is an architect's: serial input with framebuffer output, which needs no new
driver at all, or a USB HID stack on three architectures, which is weeks of work and is what the
title of this journey actually claims.

That is the same discovery journey 1 made about milestone 177, one layer down, and it is the second
time this format has found unowned work that scanning the roadmap milestone by milestone did not.

## The order I would run it in, and why

**x86_64 first, against instinct.** Steps 3, 4 and 5 are the risk. riscv64 has already booted on
silicon, which retires the strong form of risk 9 for that ISA; aarch64 is the development
architecture and its board is well documented. x86_64 is the newest port, it has never touched its
hardware, and milestone 177's own text says it has no real interactive boot entry point at all.
**The cheapest decisive experiment in the whole project is step 3**, because the hardware is already
installed and a byte over serial either arrives or does not.

**Then the display and input fork (7 and 8), then the two remaining boards.** Steps 7 and 8 are what
turn three booting kernels into the story in the title, and step 8 is blocked on a decision rather
than on work.

## What "done" looks like

A person sits at each of three machines in turn, logs in, opens `kilo`, types, and saves a file.
Nothing in the path is emulated on any of them, and **the keyboard they type on is plugged into the
machine they are sitting at** (calef, 2026-08-30, setting milestone 192's completion criterion).
Option A's serial input is what lets every other step be proved on hardware first; it does not
satisfy this paragraph. At that point risk 9 is dead, risk 6 is mostly dead,
and the claim that this is a portable capability core is a demonstration rather than an argument.
