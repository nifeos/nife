---
status: PARTIAL
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: a PC with an xHCI controller behind an IOMMU, a USB keyboard and a monitor
specific_machine: xenon (calef types on a real USB keyboard there)
needs_person: yes
---
# 242. USB host and HID, because on commodity hardware the keyboard is not a UART

How does nife reach hardware that has no serial port?

Built under QEMU on all three architectures 2026-10-04; what is left is the machine. A
confined EL0 driver holds the whole xHCI controller less its MSI-X page, enumerates a boot
keyboard on a root port, and feeds the line discipline. `script/swish-check` types `echo hello` on
a USB keyboard and gets `hello` back on aarch64, riscv64 and x86_64. The status stops at `PARTIAL`
for 192's reason (calef's bar is literal): nobody has yet sat at xenon and typed on a real
keyboard. That is one evening of calef's, written out below. notes/usb.md is the task-oriented
page.

In brief. Milestone 192 (a keyboard on real silicon: the input half of every graphical story,
which nothing owns) is `PARTIAL`. Its option A landed on 2026-09-02 and did the structural half well:
the keystroke source is now one `match` in the kernel, the board's UART and virtio-input are
interchangeable, and `crates/system_initializer` cannot tell which it got. Option B is a third arm
of that match and nothing else in the guest is downstream of the choice.

But that third arm is USB, and nothing owns it. The string `xhci` appears in exactly one roadmap
block, 192's, and only as a word. There is no host-controller milestone, no HID stack, no hub
enumeration, no transfer-ring code. On commodity hardware the keyboard is USB, so option B is a
large unbuilt subsystem wearing the word "option", and this block exists to stop it being priced as a
line item in someone else's plan.

calef's own bar for 192, set the hour it was minted: *"192 isn't done until we can sit down at the
keyboard connected to the computer and display the OS on a monitor plugged into the machine."* That
sentence is this milestone.

## Why it is on the customer path rather than the risk path

AGENTS.md records that the customer path is vacant, and its own guidance is that
a first customer should be something nife can plausibly be adequate at within a milestone or two.
The most obvious thing that would fill that path is "Runs on a machine someone already owns", and
a machine someone already owns has USB and no serial header.

It is also the precondition AGENTS.md attaches to the ranking function: no third party sees this
until there is a package manager and a trivial install process, and an install process assumes a
keyboard.

## What it needs, and what makes it big

- A host controller. xHCI is the one commodity hardware has, and it is a ring-based DMA interface
  rather than a register poke: command ring, event ring, transfer rings per endpoint, a device
  context base array. This is the first driver in the tree whose data structures the device walks
  on its own.
- Enumeration. Reset, address assignment, descriptor reads, configuration selection, and hubs,
  because a keyboard is often behind one.
- HID. Boot protocol is the small mercy here: a keyboard in boot protocol sends an eight-byte
  report with a fixed layout, which is far less than a full HID report-descriptor parser.
- And the confinement question, which is the interesting part. The driver of milestone 159 (a real
  hardware entropy source: the JH7110's TRNG) needed one device page and two endpoints. A DMA-driven
  controller needs memory the device writes into, so this is the first real test of what
  `crates/dma_validator` and the IOMMU work are for, on a device a person can unplug.

## What was built, 2026-10-04

No new syscall, no new capability object, no new method. The driver is a kernel-spawned EL0
process in exactly `non_volatile_memory_express_service`'s shape: spawn-time mappings it holds no
name for, three capabilities, three spawn words. One new boot grant: the progenitor's slot 27, the
driver's attach endpoint, taken the way every other conditional boot grant is (explicit slot, probed
by `system_initializer`).

| Piece | Where |
|---|---|
| Find the controller (class 0c/03/30), route its interrupt: MSI-X, else MSI (new: xenon's Intel PCH xHCI has no MSI-X), else INTx | `kernel/src/pci.rs`, `crates/pci` (`msi_cap`, `MsixCap`'s pending-bit array) |
| Take it from the firmware (USB Legacy Support), draw the register window, refuse without IOMMU confinement, confine the DMA region | `kernel/src/extensible_host_controller_interface.rs` |
| The `Spawn` literal: what the driver holds and is refused | `kernel/src/user/usb_keyboard_service.rs` |
| Halt, reset, rings, port reset, Enable Slot, Address Device, descriptors, Evaluate Context, `SET_CONFIGURATION`, `SET_PROTOCOL`, `SET_IDLE`, Configure Endpoint, interrupt reads, stall recovery, hot-plug | `components/src/usb_keyboard_driver.rs` |
| Descriptor walk, setup packets, boot-report diffing, usage to evdev | `crates/usb` (19 host tests, 3 Kani harnesses) |
| Register window, handoff, TRBs, rings, contexts, `PORTSC`, the report words | `crates/extensible_host_controller_interface` (21 host tests, 3 Kani harnesses) |
| Control chords (`^C` from a keyboard) | `video_terminal::keymap`, shared with the virtio keyboard |
| The attach handshake | `crates/system_initializer` step 3b, `components/src/progenitor.rs` slot 27 |
| The gate | `script/swish-check`'s fourth boot, `xtask/src/swish_check.rs` `usb_keyboard_boot` |

The confinement decision, and why it differs from NVMe's. §86 (whether an NVMe driver can leave
the kernel) kept NVMe's admin plane at EL1 because creating a queue is where a ring's address is
named. An xHCI names physical addresses in every structure it has, and enumeration means parsing
what a device sent, which is the last thing to do at EL1. So the kernel keeps only what is not a
driver's: the firmware handshake and the decision of which pages to map. The driver gets every
register page except the MSI-X table's and pending-bit array's (a Kani harness proves no withheld
page is ever in the mask). The kernel refuses to start it on a machine whose IOMMU does not own
the controller's requester id. That refusal is stricter than NVMe's, which runs unconfined on a
machine with no IOMMU, and the reason is that this process names every ring.

The descriptor parsing is the untrusted-input surface, and it is `crates/usb`: total parsers
that check every length before reading through it. Kani proves the configuration walk panic-free
and that anything it accepts is a real interrupt IN endpoint, for every configuration of up to 24
bytes. The host tests cover QEMU's keyboard byte for byte, composite keyboards, a keyboard behind a
mouse interface, a storage stick, zero and overlong lengths, and truncated reads.

Two things the first boots found. The driver's one-page stack overflowed in enumeration on its
first run (a data abort eight words below it), so the service maps three more. And on riscv64 an
interrupt enabled on the boot hart's PLIC context was moved by the `Irq` capability's first ACK to
the source's round-robin hart, and no key after the first arrived; enabling through
`arch::irq::enable`, as `keyboard_service` does, fixed it. Both are in the code's comments.

## Scope, per architecture

| | Under QEMU | On silicon |
|---|---|---|
| aarch64 | green: INTx through the GIC, behind SMMUv3 | argon: not tried; it has an SMMU, so it should be confined, and nobody has read its scope |
| riscv64 | green: INTx through the PLIC, behind riscv-iommu-pci | radon: refused by design. The JH7110 has no IOMMU, so the kernel will not hand a process the controller. A keyboard on radon needs an IOMMU, or a doorbell validator of the kind §86 (whether an NVMe driver can leave the kernel) calls option 4, and is not this milestone's |
| x86_64 | green: MSI with no MSI-X, behind intel-iommu (the gate's configuration, modeled on xenon's controller); MSI-X also passes | xenon: calef's bench step, below |

## The bench step, which is calef's

This is the next action on milestone 192, ahead of option A's board run (calef, 2026-10-21:
*"Yes"*). One evening. Steps 1 to 3 are all 192 needs; steps 4 and 5 are this milestone's and
milestone 198's and do not hold it up.

Needs xenon, its monitor on the display port, a USB keyboard plugged into a rear port directly
(not through a hub, and not a keyboard with a hub built in), and the serial cable for the log.

1. Build the stick: `cargo xtask uefi-image`, then copy `target/esp/EFI/BOOT/BOOTX64.EFI` to a FAT32
   stick as `/EFI/BOOT/BOOTX64.EFI` (milestone 198's rung 1a recipe, unchanged). VT-d must be on in
   setup, as it already is for milestone 261 (the NVMe driver leaves the kernel).
2. Boot it, and read the `usb       :` line on the screen or the serial log. It is one of:

   | Line | Means |
   |---|---|
   | `a keyboard on port N (full speed, vvvv:pppp); its keys reach the shell` | Go to step 3. |
   | `no keyboard yet (...; port N: <why> (<detail>))` | The controller is up and a device was refused. The `<why>` names the step. A "not a keyboard that speaks the boot protocol" with detail 4 is a keyboard with no boot interface, or a hub. Try another keyboard; photograph the line either way. |
   | `REFUSED. The IOMMU does not confine the USB controller` | xenon's DMAR gives the xHCI to a VT-d unit this kernel did not bring up: milestone 594 (every VT-d unit translates its own devices). |
   | `REFUSED. A register the driver needs shares a page with the controller's MSI-X table` | Photograph it: the register window refusal, with the offset. |
   | no `usb` line | No function with the xHCI class code was found: look for the controller in the boot's PCI survey. |

3. At the `$` prompt, on the USB keyboard, type `echo hello` and press Enter. The output that
   closes milestone 192's exit criterion (a keyboard plugged into the machine and the OS on a
   monitor plugged into it) is both of these in one photograph of the monitor: the step 2 line
   `a keyboard on port N (full speed, vvvv:pppp); its keys reach the shell`, and `hello` printed
   under the typed `echo hello`. With the serial cable still attached, the same `hello` in the log
   is supporting evidence, not the criterion. Keep the photograph and the log with the bench
   record off-tree, then flip this milestone and 192 to BUILT.
4. Unplug the keyboard, plug it into another port, and type again: it should be found again with no
   line printed (hot-plug is silent by design; see the driver's BUGS).
5. Then pull the serial cable and repeat step 3, which is rung 1c's literal exit criterion
   (milestone 198 (a package manager, and the trivial install)).

## BUGS

- Never run on silicon. Every timeout is the specification's limit rather than a measurement,
  and QEMU models no low-speed device. Full speed and MSI-only are modeled, because those are what
  xenon's keyboard and controller are.
- Root ports only, one controller, one keyboard. A hub, or a keyboard with one inside, is not
  found. `crates/extensible_host_controller_interface`'s and the driver's BUGS.
- No key repeat, no Caps Lock, no LEDs. `crates/usb`'s BUGS.

- This block does not price it in hours, and should not: xHCI is the largest single driver this
  project would have written, and the honest first step is a survey rather than an estimate.
- Boot protocol may not survive contact with real keyboards. Some report protocol only, some
  need a `SET_PROTOCOL` they then ignore, and the ones that misbehave are exactly the cheap ones a
  stranger owns.
- It says nothing about USB storage, hubs beyond the first, or anything but a keyboard, and
  should not: the deliverable is a keystroke, not a USB stack.
- Nothing here helps a headless machine, which is the other half of the same question and is
  milestone 243 (a machine with no serial port has no way to say anything).

## Follow-on

- **Outstanding.** calef's bench step on xenon, above: the only part of this milestone that is about
  real hardware, and the part that closes milestone 192. Checked 2026-10-04.
- **Recorded.** Hubs: a keyboard behind a hub, or with one built in, is not found. In
  `crates/extensible_host_controller_interface`'s and `components/src/usb_keyboard_driver.rs`'s
  `BUGS`.
- **Recorded.** Typematic repeat, Caps Lock and the keyboard LEDs. In `crates/usb`'s `BUGS`.
- **Recorded.** A second xHCI controller, and a second keyboard. In
  `kernel/src/extensible_host_controller_interface.rs`'s and the driver's `BUGS`.
- **Recorded.** radon, and any machine whose IOMMU does not own the controller, gets a refusal and
  no USB keyboard, by design. In `kernel/src/extensible_host_controller_interface.rs`'s `BUGS` and
  notes/usb.md.
- **Recorded.** A device refused after boot is refused silently, and the attach handshake is a
  blocking send. In `kernel/src/user/usb_keyboard_service.rs`'s `BUGS`.
- **Recorded.** No replayable falsification of the confinement claim (a driver role that aims a
  ring outside its region), the gap milestone 261 (the NVMe driver leaves the kernel) also carries.
  In notes/usb.md's `BUGS` by way of "never run on silicon"; the register-window half is proved by
  Kani.

## Radon's storage depends on this, 2026-10-07

§260 (radon's block server serves a USB drive) puts radon's USB 3 ports, which sit behind a VL805
controller on PCIe, on this milestone's critical path. The booted system's storage on radon
cannot be read until this milestone's xHCI driver runs on radon too, after milestone 163 (the JH7110's PCIe root complex).

## Index row

Milestone 192's option B. Built under QEMU 2026-10-04 on all three architectures: a confined EL0
xHCI driver, denied its MSI-X page and refused without IOMMU confinement, enumerates a boot keyboard
and feeds the line discipline; `script/swish-check` types `echo hello` on it. What is left is
calef's bench step on xenon, which also closes 192 and is 192's next action (calef, 2026-10-21).
