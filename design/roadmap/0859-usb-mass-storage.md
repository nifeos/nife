---
status: NOT-STARTED
raised: 2026-10-07
promoted_from: usb-mass-storage
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 859. A USB mass-storage driver: bulk-only transport and SCSI, as a confined server

*(The title and slug are provisional.)*

It is filed under §260 (radon's block server serves a USB drive): nothing else in
`design/roadmap/` owns mass storage, UAS or bulk-only transport.
Milestone 618 (USB keyboards behind the console) lists mass storage as explicitly out, and
milestone 242 (USB host and HID) builds the host controller and input only. Name provisional.

calef ruled on 2026-10-07 (UTC) that radon's block server serves a USB drive, "even though it is
more work. I need that work done anyways." This is the part of that work no milestone held.

Reuse: take first, write second. Unread so far. The lane that builds it searches crates.io
(`usb-storage`, `usbd-storage`, `scsi`), OpenBSD's `umass` and `sd` (ISC) and FreeBSD's `umass`
(BSD-2-Clause) first, and records what it took in this line. Linux's `usb-storage` is GPL and is
read for hardware facts only.

## What it is

A driver for USB mass-storage devices using the bulk-only transport (BOT) with the SCSI transparent
command set: INQUIRY, TEST UNIT READY, READ CAPACITY, READ(10 or 16) and WRITE(10 or 16), with
REQUEST SENSE after a failed status. It runs as a confined EL0 server in the shape of the NVMe and
virtio block servers, speaking `filesystem_protocol::blk` to its client. It sits on top of the
xHCI driver milestone 242 builds and asks it for bulk endpoints; it does not touch the controller.
USB Attached SCSI (UAS) is out of scope until a drive that needs it exists, since BOT works on
every drive and UAS drives fall back to it.

## Parity (§19 (architectural parity is a tenet))

The protocol layer (command blocks, status wrapper, SCSI parsing, the error and reset paths) is
architecture-neutral and goes in a host-tested crate. The driver ships on all three architectures,
proven by the same suite against a simulated device, and under QEMU against `usb-storage` on its
xHCI controller.

- xenon (x86_64): first real use, behind milestone 242's xHCI.
- radon (riscv64): first use for the booted system's storage, behind milestone 163 (the JH7110 PCIe
  root complex), because the VL805 controller on radon's USB 3 ports is a PCIe device, and behind
  the same xHCI driver.
- aarch64: under QEMU until a board arrives.

## What proves it

A drive's sector read and write round-trip under QEMU on all three ISAs. A hostile or short device
(wrong tag, short data stage, stall) is refused without wedging the server. The confinement claim is
tested as the NVMe driver's is: the driver can reach its endpoints and its DMA window and nothing
else. On silicon, a read on xenon, then on radon once 163 lands.

## Index row

No milestone owned the driver that turns a USB port into a disk. radon's storage target is a USB
drive (§260), and xenon will want one too. Bulk-only transport and SCSI, as a confined server on all
three architectures.
