# The shell on the firmware's screen

*Name: provisional, after the parent's "The shell on the screen, too".*

An appendix to [`notes/serial-less-output.md`](../serial-less-output.md). It holds how milestone 400
(the shell on the firmware's screen) put the shell's prompt on the screen beside the serial console:
the pipeline, the three pieces, and the gate.

The kernel's screen console is the kernel's voice. Since milestone 299 (the x86 port-range
capability) the console is a userspace process writing COM1. So until milestone 400 the tour reached
the screen and the shell's prompt did not. Now it does, beside the serial console rather than
instead of it:

```text
  swish ─► line_editor ─► console ──out──► COM1
                             └──OPERATION_WRITE──► display_terminal ──FLUSH──► framebuffer_driver ──copy──► the aperture
```

- `framebuffer_driver` serves the same framebuffer contract `gpu_driver` does, over the screen
  the firmware left running. A flush is a CPU copy from the surface into the aperture
  (`screen_console::Aperture`, host-tested). It holds only the aperture rows its surface can reach,
  as a spawn-time mapping.
- The console server tees. Every byte it writes to COM1 it then hands `display_terminal`. Both
  surfaces say the same thing, and every serial gate is unchanged.
- The kernel hands the screen over once (`console::yield_screen`). It clears it under the console
  lock and stops painting. Only then is the driver spawned. After that the kernel's own lines reach
  the UART only, except a panic, which takes the screen back to be read.

The gate is `cargo xtask uefi-boot`'s second and third stages. They check the prompt on the screen,
and the answer to `echo typed on the wire` typed on the serial line. The block has the decisions,
what lost, and xenon's bench step:
[0400-the-shell-on-the-firmware-screen.md](../../design/roadmap/0400-the-shell-on-the-firmware-screen.md).
