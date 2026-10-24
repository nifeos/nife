# The window between the firmware and the kernel

*Name: provisional, after the parent's "The window between the firmware and the kernel".*

An appendix to [`notes/serial-less-output.md`](../serial-less-output.md), which keeps the table of
what a person at a monitor can see. This file holds why the window cannot be narrated, how the
loader's banner bounds it, the experiment that proved the banner, and why the boards get nothing
equivalent.

## It cannot be narrated

This is problem 3 of the block's three, early boot, and the first thing to say about it is that it
cannot be narrated. From `ExitBootServices` to the kernel's own `attach_screen` there is no console.
The firmware's is gone by specification, and the kernel's is not up. The code in between is the
loader's mode-switch trampoline, `boot.s`'s 32-bit half, the page tables and the long-mode jump. It
is a 32-bit instruction stream with no idea where the screen is and no IDT. So a fault there is a
triple fault and a silent reset. Nothing a person could write would make that window speak.

## It can be bounded

It can be bounded, and since 2026-09-19 it is. `uefi_loader` clears the screen and writes two
lines into the framebuffer as its final act, after `ExitBootServices` and before the jump:

```text
nife loader: firmware released, entering the kernel.
If this line is still here, the kernel stopped before its console came up.
```

The screen is therefore never blank during that window, and the five things a person at a monitor can
be looking at are now distinguishable rather than four of them being "black". The parent's table,
under "The window between the firmware and the kernel", lists the five.

The third row, "the two lines above, still there", is the one that did not exist before, and it is
the whole of what this buys. It was
proved rather than reasoned: with a temporary halt in place of the jump, `cargo xtask uefi-boot`'s
screendump has ink in exactly the top sixteen pixel rows of a 1280x800 screen and they read those two
lines.

It is painted after `ExitBootServices` on purpose. Before that call the screen is the firmware's
console, and writing the aperture underneath it would race the firmware's own scrolling. The aperture
survives the call, which is this whole milestone's founding observation: what ends is the firmware's
*console*, not the *display*.

## The boards get nothing equivalent

The reason is the boot chain rather than effort. Under QEMU the boards are entered from `-kernel`
with no stage before the kernel at all, so there is nobody to paint a banner. And the kernel's own
`ramfb` framebuffer is in `.bss`, which `boot.s` has not finished zeroing at the point this would
have to happen. On the VisionFive 2 the stage that could say something is U-Boot, and that is
milestone 157 (real display output on the board)'s `simple-framebuffer` handoff. The board window is therefore
`boot.s` plus one `fw_cfg` conversation, which is a few hundred instructions.
