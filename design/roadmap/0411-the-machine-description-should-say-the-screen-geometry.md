---
status: NOT-STARTED
raised: 2026-09-14
promoted_from: the-machine-description-should-say-the-screen-geometry
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 411. The machine description should say the screen's geometry, not just its address

It reads the tree and touches one function.

**Premise re-checked 2026-09-19 and still true, to the line.** `console::print_summary` still prints
`and a screen, {len} bytes of framebuffer at {pixels}` out of the two fields `KernelConsole::screen`
keeps. The width, height and pixel order still appear only in the x86_64 arm's own `screen`
line.

## In brief

The machine description answers "what console does this machine have" on all three architectures,
and where there is a framebuffer it says where it is and how many bytes it is:

```
  console         : 16550 at i/o port 0x03f8, interrupt line 4
                  : and a screen, 4096000 bytes of framebuffer at 0xffff8880c0000000
```

That is what the kernel's console holds. It is not what a person bringing up a board needs. The
width, the height and the pixel order arrive in the boot handoff and are printed by the `x86_64`
arm's own `screen` line. They are the three numbers that decide whether a picture will be legible or
scrambled:

```
  screen      : 1920x1080 bgr8 at 0xc0000000, 274x135 cells (boot cmdline)
```

So the fact exists, on one architecture, in the arm rather than in the description.

## Why it matters more than it sounds

**A board with a monitor and no serial port is exactly the machine the machine description was
written for.** `xenon` is the recorded case: at first light there was no serial console this project
could read, so these lines *were* the transcript, photographed off the monitor. A description that
cannot say what geometry it is drawing at cannot diagnose the one thing a
photograph is ambiguous about.

The other two architectures grow a display path at milestone 157, and this should be there for them
rather than added twice.

## Roughly what to do

`crate::console` already holds the framebuffer address and length inside `KernelConsole::screen`.
What it does not keep is the `machine_discovery::framebuffer::Framebuffer` the geometry came from,
nor the `ScreenConsole`'s cell grid. Either keep the geometry beside the pointer, or ask
`ScreenConsole` for its own grid. Print both in the description's console answer. Then the
`x86_64` arm's `screen` line can say only what it says *early* (that a screen was found before
anything else was up). That is the split
milestone 409 (design/roadmap/0409-one-machine-description-not-two.md) is about.

## Index row

The machine description says whether a machine has a framebuffer and where it is, but not the
width, the height or the pixel order, the three numbers that decide whether a picture is legible or
scrambled. Those arrive in the boot handoff and only the x86_64 arm prints them. A board with a
monitor and no serial port is the machine this description was written for, and xenon's first light
was photographed off the screen. The other two architectures grow a display path at milestone 157,
so this wants doing once.
