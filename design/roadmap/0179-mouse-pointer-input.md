---
status: NOT-STARTED
raised: 2026-08-26
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 179. Pointer input: a mouse in text mode

`notes/glyphs.md` names this gap as an honest limit rather than an unmentioned one: **"No mouse."**
The compositor and terminal milestones (29, 33, 142) attach only a keyboard today, deliberately, not
for want of a device.

Nothing here is known to be blocked on a design fork the way milestone 178 is, but
the "what this needs" section below has not been checked against the compositor's actual event
model, and a fork may be hiding there (see "What is not yet checked").

## What this is, in brief

A pointer device attached the same way the keyboard is (a confined virtio driver behind the IOMMU,
DECISIONS §18's PCIe transport), producing motion and button events the compositor can route to
whichever window sits under the pointer, and the terminal can use for text selection (this
milestone's own scope) or hand to a client program (milestone 180's scope, if a clipboard exists to
carry the selection anywhere).

## Why this is not a driver-writing problem, and what it actually is

**The device exists and is already partly discovered.** `crates/pci::VIRTIO_INPUT_MODERN` (0x1052)
is virtio's input device class, and QEMU's `virtio-tablet-pci` presents the identical PCI id
`virtio-keyboard-pci` does; `crates/pci`'s own doc comment already names this ("the id names the
device class, not the keyboard") because milestone 29's driver had to know it to avoid attaching the
wrong one. **Today we attach only a keyboard, by choice, not by inability to tell them apart at all**:
the two are distinguished by the device's own configuration space (`virtio-input`'s `EV_KEY` vs.
`EV_ABS`/`EV_REL` capability bits it advertises), which the existing driver's discovery path has
never needed to read.

So the device-level work is: read that configuration space to tell a tablet from a keyboard, attach a
second `virtio-input` instance behind its own IOMMU domain (same shape as the keyboard's, DECISIONS
§18), and parse `virtio-input`'s absolute-position event stream (a tablet reports `EV_ABS`, not
`EV_REL`, which is the easier of the two to reason about: no relative-motion accumulation state to
get wrong, just "here is where the pointer now is").

## What it needs

- Device discovery: read `virtio-input`'s configuration space to select a tablet over a keyboard
  when both are present, rather than the current "attach only a keyboard" default. A third
  `NIFE_`-prefixed test-leg flag, alongside the existing `NIFE_GPU`/`NIFE_KBD` pair, is the obvious
  shape once a lane picks its name, matching how each
  device milestone before this one gated itself.
- A pointer capability and its confinement: the identical shape milestone 29's keyboard driver
  already proves (a confined EL0 driver, an event rendezvous, an IOMMU domain), applied to a second
  device rather than invented fresh.
- Routing through the compositor: milestone 33's own multiplexing model picks which window (or
  which pane of a windowed terminal) a click or a motion event belongs to. This is the part most
  likely to need design work the driver side does not, because nothing in the compositor's event
  model has had to answer "which client owns this coordinate" before; input has so far only ever
  meant keystrokes, routed to whichever client currently holds the terminal's input side
  (`DECISIONS §21`'s line-discipline contract), with no notion of screen position at all.

## What is not yet checked

Whether the compositor's client model has anywhere to put a pointer event at all. Milestone 33's
own text should be read against this before any driver work starts, because if the answer is "the
compositor has no per-window hit-testing today," that is a real design fork (does pointer routing
piggyback on the existing window list, or does it need a coordinate-to-client index milestone 33
never needed to build) and belongs in this file rather than being discovered mid-implementation.
Not checked here because it is genuinely a separate read, not because it is assumed trivial.

## What this unblocks

Text selection in the terminal (drag to select, the mechanism milestone 180's clipboard would need a
source for) and, eventually, anything resembling a windowed UI reaching past keyboard-only
interaction. Independent of milestone 142's type-and-scanout work: a pointer needs no font decision
to land, and a font decision needs no pointer to land.

## Prior art

`virtio-input`'s own spec already answers the wire-protocol half (event codes, absolute vs. relative
reporting) the same way it answered the keyboard's. The genuinely new part is entirely this system's
own: no compositor here has ever had to route anything by screen position before.

## BUGS

Not started; nothing built yet to carry its own BUGS section.

## Index row

`notes/glyphs.md` already names "no mouse" as an honest limit; `virtio-tablet-pci` shares the same
PCI device id as milestone 29's keyboard (`crates/pci::VIRTIO_INPUT_MODERN`), so device discovery
already half-knows this and simply attaches only a keyboard by choice. The open part is routing a
pointer event through the compositor, which has never had to answer "which client owns this
coordinate" before.
