# The bench procedure for option A, in order

An appendix to [milestone 192 (a keyboard on real silicon)](../0192-keyboard-on-real-silicon.md).
It holds the bench procedure for whoever runs option A on a board.

It cannot be run yet, and the reason is milestone 157 (U-Boot's `simple-framebuffer` handoff) rather
than this milestone. Option A is framebuffer output plus serial input, and the framebuffer half is
[157](../0157-uboot-framebuffer-handoff.md) (U-Boot's `simple-framebuffer` handoff), which is
NOT-STARTED. Until a board can put a pixel on a monitor there is nothing for a serial keystroke to
echo onto. 157's lane inherits it rather than rederiving it.

Needs radon powered on (the Kasa plug), a serial console attached before power, a microSD card, and
a monitor on the board's HDMI. `notes/visionfive2.md` carries the board facts and the
failure-triage ladder.

1. Follow milestone 159's steps 1 to 3 verbatim (`script/board-image`, copy all three files from
   one run, DIP switches to QSPI, serial at 115200 8N1, interrupt U-Boot, the five `load`/`booti`
   lines). Nothing about this milestone changes the boot recipe.
2. Start graphics from the prompt: since milestone 632 (graphics on demand) they start when a
   `graphical_terminal` session launches, not at boot, and no boot prints a `keystrokes:` line.
   Outstanding: this step needs the launch command and what a board should print written down
   before a bench run.

3. Look at the monitor. A prompt on the screen is 157's claim, not this one; record it either
   way, because everything below depends on it.
4. Type a character on the serial console. It must echo on the monitor, not on the serial
   line: `input` forwards it to `line_editor`, which echoes through `display_terminal`. If it
   echoes on the serial console instead, the session did not take the display path.
5. Type `echo hello` and press return. `hello` on the monitor is option A complete on a real
   machine: a person at a laptop typing across a null-modem cable into a system whose output is on
   its own screen.
6. Failure modes, in the order they are worth checking. No echo on the monitor means one of two
   things. Either
   the UART receive interrupt is not routed, or the display path hung. The first is the same
   PLIC-source hazard `uart_irq_and_source`'s own doc records for the JH7110, where the QEMU
   constant armed the wrong source. The second looks like the second-`FLUSH` hang of milestone 177
   (wire the graphical terminal stack into the real interactive boot): a character echoing once
   and then nothing.
