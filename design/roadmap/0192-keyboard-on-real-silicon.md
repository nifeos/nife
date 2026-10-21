---
status: PARTIAL
raised: 2026-08-30
milestone_dependencies: 242
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 192. A keyboard on real silicon: the input half of every graphical story, which nothing owns

Journey 3 (login to `kilo` on real silicon) needs a keyboard on real hardware, and nothing else on
the roadmap owns one. Option A's wiring is built; "What option A built" below says what it is,
and why the status stops at PARTIAL rather than BUILT.

**Decided 2026-08-30 by calef:** *"we will do Option A first and follow with Option B
later."* Both options are in scope; the decision is their order, not a choice between them.

**Decided 2026-10-21 by calef**, asked whether milestone 242's xenon bench step should be the next
192 action, ahead of option A's board run: *"Yes"*. So the next action on this milestone is
[milestone 242](0242-usb-host-and-a-keyboard-that-is-not-a-uart.md)'s bench step on xenon, where
option B is already built and needs only a bench evening. The order above stands for radon and
argon, where B is large and A goes first.

## What done looks like, and option A does not meet it

**calef, 2026-08-30:** *"192 isn't done until
we can sit down at the keyboard connected to the computer and display the OS on the dedicated
monitor."*

So the completion criterion is literal and it is option B: **a keyboard plugged into the machine, and
the OS on a monitor plugged into the machine.** Option A is a first increment that reaches `PARTIAL`
and nothing more. A block that let serial input close this milestone would be claiming a story the
system cannot tell, which is what §39 says a name must never do and what a completion criterion must
never do either.

Why A still goes first. It gets pixels onto the real monitor and a usable keystroke path with no
new driver. So every other step of journey 3 (login to `kilo` on real silicon, on all three
architectures) can be exercised on real hardware while the USB work is still unwritten. It buys
sequencing, not completion.

What that requires of A's implementation, and this is the part that is easy to get wrong: nothing
in A may assume keystrokes arrive on a UART. Both paths feed the same `DECISIONS §21`
line-discipline contract, and A is finished when the keystroke's source is the only thing B has to
change.

In brief. Every graphical story this project tells assumes a keyboard, and on real hardware there
is no way to press a key. Milestone 29 (a display terminal: framebuffer, virtio-gpu, and a foreign
component) gets keystrokes from virtio-input, which is a QEMU device. None of the three
boards has one. A search of the entire roadmap for `usb`, `xhci`, `hid` and `ps/2` returns
nothing, so this is not a milestone that exists and is unscheduled; it is work nobody has written
down at all.

## Why it went unnoticed for so long

Because every consumer of input so far has been either a test harness or a serial console, and both
work. The plain UART `console`/`input` pair the real interactive boot spawns today reads keystrokes
over the UART, which is real on all three boards. Nothing broke, so nothing asked.

It only becomes visible when the story is *a person sitting at the machine*, which is what journey 3
is and what journey 1 is a QEMU-shaped rehearsal for.

## What option A built, and why the status is not BUILT

The status is `PARTIAL` and stays there, by calef's completion criterion above. Nothing option A
built lets anyone sit down at a keyboard, and flipping this block to BUILT would claim exactly the
story he said it does not tell. Milestone 159 (a real hardware entropy source) stops short of BUILT
for the same reason: a lane may advance a milestone without being allowed to close it.

Input already had an owner:

- `components/src/input.rs` is the UART receive driver. It holds `WRITE` on one terminal endpoint,
  `READ` on the UART receive `Irq`, and one device-typed page of registers. It forwards bytes as
  `line_editor::proto::OPERATION_BYTES`, up to eight per `CALL`.
- `components/src/keyboard_driver.rs` is a virtio-input driver holding `WRITE` on one fixed
  endpoint, sending byte for byte the same `OPERATION_BYTES` framing.
- So the two sources are interchangeable at DECISIONS §21 (the terminal is a userspace
  component)'s line-discipline contract.

Option A puts them behind one choice, made when a `graphical_terminal` session launches, in
`crates/system_initializer`'s `build_graphical_terminal_session` (milestone 632 (graphics on
demand)). With a keyboard the session gets `keyboard_driver` behind its own line discipline;
without one it reads the boot line discipline's endpoint raw, which on a board is the UART. The session
cannot tell which source it got, and that it cannot is the property this block asked option A to be
finished by.

Option B is a third arm of that choice. Nothing else in the guest is downstream of it.

## What authority the serial source holds, which is the risk-6 claim

Stated precisely, because precision is what makes a confinement claim evidence rather than a
slogan, as milestone 159 shows (its own driver: one page of device memory and two
rendezvous capabilities, no DMA page and no `Irq` cap).

`input` holds two capabilities and one mapping:

| What | Rights | Why that and no more |
|---|---|---|
| the terminal endpoint (slot 0) | `WRITE` | it may `CALL` exactly one destination, fixed at spawn by the kernel, and can name no other |
| the UART receive `Irq` (slot 1) | `READ` | WAIT and ACK. No `GRANT`, so it cannot hand the interrupt on |
| one page of UART registers | device-typed | the receive side of a PL011 or an NS16550 |

It holds no DMA page, no `Virtio` transport, no budget, no report endpoint, and no
capability naming any other process. It cannot print, cannot spawn, and cannot read what anyone
else typed. This is `components/src/input.rs`'s documented authority.

The honest comparison with option B, since risk 6 is what this is evidence for. A UART is a
memory-mapped register file with an interrupt. Confining one proves considerably less than
confining a bus that enumerates whatever a stranger plugs in. An xHCI driver needs DMA, and on a
board with no IOMMU (`notes/verification.md` on the JH7110) that is the whole question. Option A
is not evidence about xHCI. It is evidence that the *seam* holds: a real, non-virtio input source
reaches the line discipline through a capability naming one endpoint.

## What is proven, and where

In QEMU on aarch64 and riscv64, `script/swish-check`'s graphical legs launch a
`graphical_terminal` session from the normal UART boot, with a virtio-gpu and no keyboard, and a
keystroke reaches the screen over the UART path. Both legs are green in CI. They prove option A at
session launch rather than at boot.

On x86_64 the session reads the boot's line discipline raw, fed by `input` over a COM1 `PortRange`
(milestones 299 and 505), so the serial source needs no new driver. x86_64 has an interactive boot,
`swish-check-x86_64` in CI, but no virtio-gpu in its runners (milestone 270 (wire `virtio-gpu-pci`
and `virtio-input` into the x86_64 test runner)), and milestone 632's BUGS lists what a graphical
leg needs.

## The bench procedure, and the fork

Two appendices hold what the page has no room for:

- [`bench-procedure.md`](0192-keyboard-on-real-silicon/bench-procedure.md): the ordered bench
  procedure for option A on radon, and why it cannot run yet.
- [`the-fork.md`](0192-keyboard-on-real-silicon/the-fork.md): options A, B and C, with why option C
  is closed and what would start option B.
- [`pricing.md`](0192-keyboard-on-real-silicon/pricing.md): what each option costs on argon, radon
  and xenon, with the controllers each board wires to its ports and the basis of every estimate.

## What option B will still need

An xHCI driver, enough USB core to enumerate and configure a device, and a HID keyboard driver,
on three architectures, plus the scoping pass that splits those into their own milestones. What it will not need is any change to
`line_editor`, `display_terminal`, `swish`, or the boot's capability layout: a HID driver that holds
`WRITE` on one endpoint and sends `OPERATION_BYTES` is one more keystroke source. That choice lives
in `crates/system_initializer`'s `build_graphical_terminal_session`, which knows two arms
today (`keyboard_driver` by name, or the boot's discipline read raw).

## Why A goes first, which was the recommendation

AGENTS.md's rule is to recommend on reversible forks and give options only on irreversible ones,
and this fork is reversible. Choosing
serial input forecloses nothing, because option B can be built at any point afterward, against the
same `DECISIONS §21` line-discipline contract that both the UART console and `display_terminal`
already speak identically.

The reasoning is about what journey 3 is for. That journey exists to test two of the nine
entries in design/fatal-risks/README.md: risk 9 (the HAL is a fiction and an architecture costs a
restructure) and most of risk 6 (a confined driver cannot drive real hardware). A framebuffer on
real silicon tests both of those. A keyboard adds almost nothing to the test. It adds to the
story, and the story matters, but not enough to spend months on a bus driver before the other seven
risks have been answered at all.

The honest cost, stated so it is a known trade rather than a hidden one: while only A exists the
demonstration is *a person typing into the machine across a null-modem cable* rather than *a person
sitting at the machine*. That is weaker, and anyone watching will notice. It is why A cannot close
this milestone.

What moves B up the queue: a customer, or an audience, for whom the difference between those two
sentences is the product. B is also the better answer the moment nife has survived enough of the
fatal-risk list that months of driver work is a reasonable thing to spend.

## What is true either way

- The framebuffer half is milestone 157 and is unaffected by this fork.
- Option A is a strictly smaller superset of nothing: it needs no new milestone at all, which is
  why it is worth pricing honestly against B rather than dismissing.
- If B is chosen, it is not one milestone. xHCI, USB core, and HID are three, and the scoping
  pass is the first piece of work.

## BUGS

- The pricing is line ranges scaled from Linux by a ratio two drivers set, never hours, and the
  JH7110 and Tegra210 reference manuals were not read (`pricing.md`'s BUGS).
- It says nothing about a mouse, which milestone 33 (a compositor: one screen, mutually distrusting clients)'s compositor will eventually want and which
  the same stack would carry.
- The decision is a sequencing call, not a claim that option B is wrong. A demonstration
  operating system that cannot accept a keyboard is limited in a way no scope note repairs, and this
  block should not be read as saying otherwise.
- Option A has never been run on a board, and that is where the unglamorous problems live. The
  wiring exists and comes up in QEMU, so "zero new drivers" is more than a reading of the code. But
  nobody has displayed a terminal on a *board's* framebuffer while taking keystrokes over that
  board's UART, and nobody can until milestone 157 lands.

## Follow-on

- **Milestone 242.** Option B, the xHCI driver, USB core and HID keyboard, plus the scoping pass
  this block asks for, is `design/roadmap/0242-usb-host-and-a-keyboard-that-is-not-a-uart.md`, so
  option B is not priced as a line item here.
- **Outstanding.** Next: milestone 242's xenon bench step, ahead of option A's board run (calef,
  2026-10-21: *"Yes"*). It closes this milestone when its step 3 prints `hello` on the monitor.
- **Outstanding.** Option A has still never been run on a board and cannot be until milestone 157
  puts a pixel on a board's framebuffer. The six-step bench procedure is written and unexecuted.
- **Outstanding.** No keystroke has reached a screen on real silicon; that waits on milestone 157.
- **Milestone 179.** A mouse is milestone 179 (pointer input: a mouse in text mode), rather than an unnamed gap
  this block has to carry.
- **Done.** Both options are priced per board in `pricing.md`. On xenon option B needs no known
  new code, only the bench step of milestone 242 (USB host and HID). On radon it needs milestone
  163 (the JH7110's PCIe root complex) and a way to confine an xHCI with no IOMMU. On argon it
  needs milestone 127 (the seL4 machine), Tegra's XUSB glue and firmware, and a Tegra SMMU driver.
- **Recorded.** The order is a sequencing call rather than a claim that option B is wrong. A system
  that cannot accept a keyboard is limited in a way no scope note repairs, and calef's completion
  bar for this block stays literal.
- **Recorded.** Option C is closed: the 7050 Micro has no PS/2 port and calef owns neither a
  keyboard nor an adapter, so there is no cheap native shortcut on any of the three machines.

## Index row

Nothing in nife can read a keypress on real hardware: milestone 29's driver is virtio-input, a
QEMU device none of the three boards has, and the roadmap has nothing for `usb`, `xhci`, `hid` or
`ps/2`. Decided 2026-08-30: both options are in scope and the decision is their order. Option A
(serial input, framebuffer output, no new driver) goes first so the rest of journey 3 can be proved
on hardware; option B (xHCI plus USB core plus HID, on three architectures, months) follows. Done
means a keyboard plugged into the machine and the OS on its own monitor, so A reaches PARTIAL and
cannot close this. The one cheap shortcut is closed: the OptiPlex has no PS/2 port. Option A's
wiring is built. The keystroke source is one choice made when a `graphical_terminal` session
launches (milestone 632): a virtio keyboard when the boot has one, and the boot's UART read raw when
it does not. Option B is a third arm of that choice. The next action is milestone 242's bench step on xenon,
where B is built (calef, 2026-10-21); option A's board half waits on milestone 157 (real display
output on the board).
