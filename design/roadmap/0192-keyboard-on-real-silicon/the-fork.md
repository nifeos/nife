# The fork: what a keystroke arrives on

An appendix to [milestone 192 (a keyboard on real silicon)](../0192-keyboard-on-real-silicon.md).
It holds the three options. The page carries the decision on their order and the reasoning behind
it.

Option A: serial input, framebuffer output. Display on the board's own framebuffer (milestone
157, U-Boot's `simple-framebuffer` handoff), keystrokes over the existing UART. On the OptiPlex this
is already wired: milestone 87 (the x86_64 bare-metal machine) bought a Dell C4PDJ serial module and
an RS-232 chain precisely so a host can drive the box. Zero new drivers. The cost is that the story
becomes "a person at a laptop typing into a machine across a null-modem cable", which is a real
demonstration of a real system and is not the story journey 3's title claims.

Option B: a USB HID stack. An xHCI driver, enough USB core to enumerate and configure a device,
and a HID keyboard driver, on three architectures. This is the honest version of "sit down at the
machine", and it is large: xHCI alone is a substantial driver against a substantial spec, so plan in
months rather than weeks. It also lands in the most attacker-adjacent position in the system, a bus
that enumerates whatever a stranger plugs in, which is either a problem or the best confinement
demonstration available depending on how it is built.
Note the peer project Atom keeps xHCI in the kernel, which is exactly the contrast DECISIONS §31
(the foreign-language seam) already draws for FAT32.

Option C: split the difference by architecture. Serial input on the boards, a cheaper native
path where one exists. This had exactly one candidate and it is closed. A PS/2 port on the
OptiPlex would have been a few dozen lines against xHCI's spec, but calef confirmed (2026-08-30)
that the 7050 Micro has no PS/2 port and that he owns neither a PS/2 keyboard nor an adapter. So
there is no cheap native shortcut on any of the three machines, and the fork is genuinely binary.
Recorded rather than deleted, because the next person will have the same idea.

On xenon B has started: it is built, and its bench step is the next 192 action (calef, 2026-10-21).
On radon and argon, when B starts is not decided here, only that it is coming. The trigger is the fatal-risk list
thinning out enough that months of bus-driver work is a reasonable thing to spend.
