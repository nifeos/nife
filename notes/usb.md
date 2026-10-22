# A USB keyboard: the xHCI driver and what it is allowed to touch

Milestone 242 (USB host and HID, because on commodity hardware the keyboard is not a UART). How a
key pressed on a USB keyboard reaches the shell, what each piece holds, and how to prove it works.

## The path a key takes

```text
USB keyboard ──► xHCI controller ──DMA, confined by the IOMMU──► usb_keyboard_driver (EL0)
                                                                   │ OPERATION_BYTES, CALL
                                                                   ▼
                                                              line_editor ──► swish
```

The driver diffs each eight-byte boot report against the last, turns the difference into Linux
evdev key codes (`crates/usb`'s `boot_keyboard`), and feeds those through `video_terminal::keymap`,
the same layout table the virtio keyboard uses. The bytes go to the line discipline's endpoint in
the framing `input` (the UART) already uses, so the shell cannot tell the two apart and both work at
once.

## Who does what

| Piece | Where | Holds |
|---|---|---|
| Find the controller, place its BAR, route its interrupt (MSI-X, MSI, or INTx) | `kernel/src/pci.rs`, `find_xhci_device` | config space |
| Take it from the PC firmware (USB Legacy Support), draw the register window, confine its DMA | `kernel/src/extensible_host_controller_interface.rs` | EL1 |
| What the driver is handed and refused | `kernel/src/user/usb_keyboard_service.rs` | the `Spawn` literal |
| Reset, rings, enumeration, the keyboard | `components/src/usb_keyboard_driver.rs` | EL0 |
| Descriptors, setup packets, boot reports | `crates/usb` | host-tested, Kani |
| Register window, handoff, TRBs, rings, contexts, `PORTSC` | `crates/extensible_host_controller_interface` | host-tested, Kani |
| Where the line discipline's endpoint comes from | `crates/system_initializer`, step 3b | the progenitor |

The kernel starts the driver before the progenitor and waits for its one bring-up report, which it
prints as the boot's `usb       :` line. The progenitor is granted the driver's attach endpoint at
slot 27 and sends `WRITE` on the terminal endpoint through it once the line discipline exists.

## The confinement, in one sentence

The driver holds every register page of its controller except the page its interrupt message is
programmed through, and its controller can reach nothing but its own DMA region. VT-d also maps any
region the firmware reserved for it (an RMRR). The first half is
`register_window` (a Kani harness proves no withheld page is ever mapped). The second is the IOMMU.
The kernel refuses to start the driver at all on a machine whose IOMMU does not confine the
controller, because a process holding an xHCI register file names every physical address the
controller reads.

## Proving it

`script/swish-check` boots each architecture a fourth time with `qemu-xhci` and `usb-kbd` attached
and nothing else that could type, presses `echo hello` through the QEMU monitor, and requires
`hello` back. The keyboard is full speed (`usb_version=1`), as real keyboards are. On x86_64 the
controller has MSI and no MSI-X (`msix=off,msi=on`), as xenon's Intel PCH controller does.

To boot one by hand:

```sh
NIFE_USB_KEYBOARD=1 NIFE_SCREEN_MON=/tmp/mon.sock helpers/qemu-bounded.sh 120 \
    helpers/qemu-runner-aarch64.sh target/aarch64-unknown-none-softfloat/debug/kernel
# in another terminal, once the prompt is up:
echo 'sendkey e' | nc -U /tmp/mon.sock
```

`NIFE_USB_KEYBOARD_OPTS` and `NIFE_USB_CONTROLLER_OPTS` are appended to the two `-device` options.

## The keystroke stall: the PLIC enable is written first

`plic::enable`, which is also the driver's ACK, writes the enable bit before the priority. QEMU's
PLIC re-evaluates delivery on a priority, threshold or completion write and on a rising line, never
on an enable write. Written the other way round, an xHCI event raised while the driver is draining
is stranded until another device's line rises, a UART byte for instance. On riscv64 that stalls the
echo of `echo hello` mid-line about one boot in thirty, until a typed byte releases the held keys.

It looks like a lost wakeup of `line_editor` across harts, and it is not one. A thread dump at the
stall shows nobody sending to `line_editor`; the driver sits in `Irq::WAIT`, and its PLIC source is
pending, enabled, above threshold and undelivered. `kernel/src/drivers/plic.rs` has the reasoning,
and `sched::tests::an_interrupt_raised_while_its_line_is_masked_is_delivered_at_the_ack` holds it on
all three architectures.

Measured on patagonia with the swish-check keystrokes alone (riscv64, TCG, four harts): 10 stalls in
300 boots with the priority written first, 0 in 300 with the enable first. aarch64: 0 in 150. x86_64
was not counted.

## The ports settle before the scan

The driver's controller reset disconnects every device, and a root port reports its device again
only once it has re-detected it. QEMU does that the instant the controller runs; xenon's Intel
controller takes about ten milliseconds. So the boot-time scan waits until no port's connection has
changed for 100 ms (USB 2.0's `TATTDB`), reading every 25 ms and giving up after 2 s. Those are
Linux's hub debounce numbers, and no machine's timing is in them
(`extensible_host_controller_interface::port::Settle`). A scan that gave up says so in the boot
line instead of printing a count. The scan only names the keyboard early: a device that shows up
later arrives as a connect change and is attached then.

To see what the ports read on a machine, build with `--features xhci_port_probe`
(`cargo xtask uefi-image --features xhci_port_probe` on x86_64). The kernel prints every root
port's `PORTSC` as the firmware left it and over the five seconds after the driver's scan, as
`xhci-probe:` lines.

## Per architecture

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| Under QEMU | INTx, SMMUv3; green | INTx, riscv-iommu; green | MSI (and MSI-X), VT-d; green |
| Real silicon | argon: not tried | radon: refused, no IOMMU | xenon: a low-speed keyboard found at boot and typing; MSI, VT-d |

## BUGS

- Root ports only: a keyboard behind a hub is not found, and neither is a keyboard with a hub
  built into it. `crates/extensible_host_controller_interface`'s BUGS.
- One controller, one keyboard. The first xHCI on the bus, the first boot keyboard on it.
- No key repeat, no Caps Lock, no LEDs. `crates/usb`'s BUGS.
- No IOMMU, no USB keyboard. radon has none; a machine whose VT-d tables give the controller to
  a unit this kernel did not bring up gets the refusal line. This is the design, and it is a cost.
- A device refused after boot is refused silently: the report endpoint carries one message.
- The driver is mapped the USB Legacy Support capability (`USBLEGSUP`, `USBLEGCTLSTS`) whenever it
  sits on a page nothing else withholds. A compromised driver can then hand the controller back to
  the firmware's SMI handler. The kernel never needs the driver to touch it. Proposed as
  `the-xhci-driver-is-not-mapped-the-legacy-support-capability` in `design/roadmap/proposals/`.
- Silicon is one machine, xenon. Every timeout but the port settle is the specification's
  limit, not a measurement.
