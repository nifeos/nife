---
status: NOT-STARTED
raised: 2026-09-04
milestone_dependencies: 242
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 369. A gate can read a serial-less machine's screen only under QEMU, and the fleet is not virtual

Filed as a proposal on 2026-09-04 by the milestone 243 lane, from its own
block's second problem; promoted by milestone 433 on 2026-09-19. Checked that day: the gate it names
is still QEMU-only, since `xtask`'s screen check reads a `screendump` PPM from QEMU's monitor and
there is nothing else that can read a serial-less machine, and the milestone this waits on is
unmoved. Milestone 242 (USB host and HID) is still `NOT-STARTED` with `Gate: NONE`. The gate line
below is still true and still points at work nobody has started.

The mechanism this needs is a write to the boot medium. The boot medium
on every machine in the fleet is a USB mass-storage device nife cannot yet talk to. Milestone 242 is
USB host and HID; the host controller is the half this waits on.

In brief. Milestone 243 (a machine with no serial port has no way to say anything, and no gate can
read it) answered the half of its block that a human needs. On a UEFI machine
with no serial port the boot tour is now painted into the firmware's linear framebuffer. A person
standing in front of the machine can watch nife boot. It did **not** answer the half a gate
needs on real hardware. The screen check it added (`serial_console::screen`, driven by
`cargo xtask uefi-boot`) works by asking QEMU's monitor for a screendump. Nobody can ask a Dell
for one.

So the state of the fleet after 243 is: six machines can be brought up by hand, with a person
reading a monitor and taking a photograph. That is precisely the state milestone 216 (nothing in
this tree can read a board, so every hardware milestone waits on a person) got the
VisionFive 2 *out* of for boards that do have a serial port.

## What it would be

Postmortem to the boot medium. The stick the machine booted from is a FAT32 EFI system
partition that a person is going to carry back to patagonia anyway. A kernel that appended its
console transcript to a file on it would turn "photograph the screen" into "plug the stick in and
run the gate", and `serial_console::progress` would judge the result unchanged, exactly as it judges
a screendump and a serial log today.

Three pieces, in the order they block each other:

1. USB mass storage, which is milestone 242's neighborhood rather than this proposal's.
2. A FAT32 writer, or a raw reserved region on the stick with a known offset, which is the much
   cheaper answer and is worth pricing first. The loader knows where its own image sits and could
   reserve a span at image time.
3. A transcript buffer in the kernel, which is new state on the diagnostic path and wants
   arguing about rather than assuming (`screen_console` deliberately holds no buffer at all).

## What it is not

**Not a network console.** Milestone 243's note priced that: it needs a NIC driver per machine and
says nothing until the stack is up. So it cannot report the failures that happen before it, and a
machine whose only voice is the network cannot report a network failure.

**Not a photograph read by a program.** `serial_console::screen` decodes a *screendump*: pixel-aligned
bytes with exact glyph matches and no threshold to tune. A phone camera produces none of those
properties, and the honest cost of making it work is optical character recognition, which is a
different project.

## Why it is worth a milestone rather than a `BUGS` entry

Because it is the difference between six machines being *usable* and six machines being *in the
test loop*. The project's own ranking function is the shortest path to a system a customer runs.
A machine that needs a person present to say anything cannot soak, cannot be woken on a schedule,
and cannot produce the boot-lottery samples milestone 249 wants. It is recorded in
`notes/serial-less-output.md`'s `BUGS` as well, where a reader meets the feature.

## Index row

Milestone 243 answered the half of its block a human needs: on a UEFI machine with no serial port
the boot tour is painted into the firmware's linear framebuffer. A person standing in front of
the machine can watch nife boot. It did not answer the half a gate needs. The screen check
works by asking QEMU's monitor for a screendump and nobody can ask a Dell for one. So six machines
can be brought up by hand, with a person reading a monitor and taking a photograph. That is
precisely the state milestone 216 got the VisionFive 2 out of for boards that have a serial port.
The shape proposed is a postmortem written to the boot medium, which is a FAT32 EFI system
partition somebody carries back to patagonia anyway, turning "photograph the screen" into "plug the
stick in and run the gate" with `serial_console::progress` judging the result unchanged. Three pieces
block each other in order: USB mass storage, then a FAT32 writer or a reserved span at a known
offset, then a transcript buffer in the kernel. That is new state on the diagnostic path and wants
arguing rather than assuming. Not a network console, which cannot report the failures that happen
before the stack is up. Not a photograph read by a program, which is optical character
recognition and a different project.
