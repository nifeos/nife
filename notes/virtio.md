# virtio-blk, driven from userspace

Milestone 9. A real block device, driven by an unprivileged process at EL0, reading a file off a
disk. It builds on everything: EL0 (7a), address spaces and shared mappings (7c/8), the capability
syscall surface (7d), IPC (7e), the userspace-driver pattern (8), and interrupt-as-message (9a).

## What virtio is, briefly

virtio is a standard for paravirtualized devices: the "hardware" is cooperating software (QEMU,
or a hypervisor) that agreed on an interface designed to be efficient across the guest/host
boundary, rather than emulating some real chip register-for-register. A virtio device is driven
through three things:

- **MMIO registers** for setup and notification (feature negotiation, queue addresses, "go").
- A virtqueue in shared memory: a descriptor table and two rings, through which the driver
  hands the device buffers and the device hands them back.
- An interrupt, raised when the device has finished with a buffer.

QEMU's `virt` machine exposes virtio over **virtio-mmio**: 32 device slots at `0x0a000000`, 0x200
bytes apart, with interrupts SPI 16..47 (INTID 48..79). We force modern virtio (version 2),
whose register interface gives separate physical addresses for the descriptor table and each ring.

## Who does what: the kernel enumerates, the driver operates

The kernel reads three standardized registers of each slot (magic, device-id, version) to find the
block device and route it to a driver. That is bus enumeration: the same thing firmware does
walking a PCI bus, and it is the smallest amount of virtio knowledge that lets the kernel say
"the block device is in slot 31, its interrupt is INTID 79." It does not set up a queue, negotiate
a feature, or move a byte. Everything else is the driver's, at EL0.

The kernel hands the driver, and nothing more:

- the device's registers, mapped as user device memory (`Flags::user_device`). A slot is not
  page-aligned, so the kernel maps the containing page and passes the in-page offset;
- a DMA page, mapped read/write, *and its physical address* (in a register), because
  descriptors speak physical addresses and a process only knows virtual ones;
- **an `Irq` capability** for INTID 79;
- an endpoint to report what it read.

## DMA: the one place a process needs a physical address

Everywhere else, a process deals only in virtual addresses; the kernel and the MMU keep it that
way on purpose. A device is the exception. The virtio device is *not* behind our MMU (there is no
IOMMU on QEMU `virt`), so when the driver puts a buffer address in a descriptor, it must be the
physical address the device will actually read and write. The driver cannot compute that from a
virtual address, so the kernel hands it the DMA region's physical base at spawn, and the driver
works out physical addresses as `direct_memory_access_phys + offset`.

On real hardware there is a second concern: cache coherence between the CPU and a DMA-capable
device. QEMU's DMA is coherent, so we get away with just compiler/CPU ordering (`dmb ish`) around
publishing descriptors and reading results. A real driver would add cache maintenance. The note is
here so the gap is on the record.

## The virtqueue, and a read

The driver lays a descriptor table and two rings out in its DMA page and tells the device their
physical addresses. To read a block it builds a **three-descriptor chain**:

```
  desc[0]  header   { type=READ, sector }   device READS it     -> next desc[1]
  desc[1]  data     512 bytes               device WRITES it    -> next desc[2]
  desc[2]  status   1 byte                   device WRITES it    (end)
```

then publishes the head index in the available ring, bumps the ring's index, and pokes
`QueueNotify`. The device does the read, writes the data and a status byte, puts the head on the
used ring, and raises its interrupt. The driver waits for that interrupt (below), then checks
the status byte is 0 (OK) and reads the 512 bytes the device left in the buffer.

## The completion arrives as a message

The driver does not poll. It `WAIT`s on its `Irq` capability (9a). When the device finishes and
raises INTID 79, the kernel masks the line, turns it into a notification, and wakes the driver. The
driver quiets the device (reads its `InterruptStatus`, writes `InterruptACK`), then `ACK`s its `Irq`
capability to re-enable the line. This is the whole reason 9a came first: a userspace driver's
completion path *is* interrupt-as-message.

## The bug this milestone flushed out: a scheduler with no idle

The first time the driver `WAIT`ed for its interrupt with nothing else to run, the kernel
**panicked**: "every thread is blocked on IPC: a deadlock." It was wrong. A thread blocked on a
device interrupt is not deadlocked; it is waiting for an event that will arrive. The scheduler had
no idle thread, so a moment where every thread was legitimately waiting for I/O looked
identical to a genuine deadlock.

The fix is the idle thread every real kernel has: a thread whose entire body is `wfi`, kept out of
the ready queue and scheduled only when nothing else can run. It parks the CPU until an interrupt
makes something runnable. See notes on the scheduler.

## Interrupts are restored only after the scheduler lock drops

`schedule()` never restores interrupts or returns from inside the locked block. Every path leaves
through one point: the lock drops, and *then* interrupts are restored, once. Restoring first leaves
a one-instruction window with interrupts on and the run-queue lock held. A timer firing in it
re-enters `schedule()` and takes the lock again, a lock-order violation that the test suite hits
only intermittently, under enough scheduler churn (more threads, the idle thread, a driver blocking
and waking on interrupts). It is the kind of "cheap to follow, expensive to retrofit" ordering rule
DECISIONS §9 (locking: IrqSafeMutex, plus a discipline) is about.

## The kernel's half

This is the design behind `kernel/src/virtio.rs`, the enumeration and the confined transport.
notes/dma.md has the confinement itself and notes/pcie.md the transport seam.

Which slots exist is the machine's answer. On riscv64 the virtio-mmio window is mapped only when
the device tree names a `virtio,mmio` node. On the T-Head TH1520 the QEMU window's address is DRAM,
and a probe there would read RAM and could mistake it for a device. A machine without the bus
reports zero slots.

Block devices have one ordering on every machine, and it is the roster's row order: every
virtio-mmio slot in slot order, then every modern virtio-pci function in bus:device.function order.
`disk_service::devices` lists the roster in the same order, so a listing and a wiring cannot
disagree about which disk is which. QEMU numbers mmio slots in device order. The aarch64 and riscv64
runners attach the nifefs disk first and the RedoxFS disk second, and both boards have five mmio
block devices, so every ordinal a wiring asks for resolves before the PCI half. `q35` has no
virtio-mmio bus (`arch::x86_64::mmu::VIRTIO_SLOTS` is 0). There ordinal 0 is the first
`virtio-blk-pci` function, and `helpers/qemu-runner-x86_64.sh` attaches the two images in the same
order. The FS server drives ordinal 1, the crash test ordinal 2, and the roster wirings 3 and 4.

The two halves are not symmetric. Resolving a PCI ordinal brings the function up: it sizes and
assigns BARs and enables decoding and bus mastering. The mmio half reads two identity registers. So
a presence probe such as `fs_service::is_crash_disk_present` means "this ordinal exists", with the
side effects of having asked.

The PCI accessors `pread` and `pwrite` are safe functions over a module invariant rather than
`unsafe fn`s. They are private, and every call site passes `common` plus a constant offset under
0x40, `isr`, `notify_addr[q]`, or `device_config` plus an offset the capability's length bounds.
All four are fields of `Transport::Pci`, resolved from a mapped BAR by `pci::PciVirtioDevice` and
copied in one place, `Transport::pci`. The compiler closes the set of callers. Converting them would
put twenty `unsafe` blocks in one file restating one sentence, and would make nothing checkable.
The invariant has one gap the code closes: `notify_addr[q]` is zero until `setup_queue` resolves
it, so `notify` refuses a queue whose doorbell is not ready rather than write through
`phys_to_virt(0)`. The mmio transport has one fixed notify register and no such gap.

`MAX_DEVICES` is an exception and a foot gun. Nothing unregisters, so it counts the devices a boot
has ever wired. The test suite spawns a driver per gate, and tests share a transport where they can
(the genuine and tampered package fetches share one `net_stack`). Memory is not the binding limit;
a net service hands its frames back (notes/frames.md). The fix is reuse on driver death, which needs
a generational name so a stale `Object::Virtio` capability cannot resolve to a different device. That
changes what a capability means, a design fork under §16 (object revocation), and it meets
`entropy_service::ensure`'s one-service-per-device rule. Milestone 647 proposes it.

Two test hooks go around the shadow ring to reach the hardware. `provoke_iommu_escape` points queue
0's available ring at a frame the IOMMU domain does not include and kicks; the device's first read
is an out-of-domain IOVA, so the IOMMU faults it. `provoke_write_to_shadow` points queue 0's used
ring into the shadow page, which the domain maps read-only, and fills its `flags` word with
`0xffff`. With nothing available, a virtio-blk device answers a kick by setting and then clearing
`VRING_USED_F_NO_NOTIFY` in that word. QEMU 11.1.1 does so whenever `VIRTIO_RING_F_EVENT_IDX` is not
negotiated (`virtio_blk_handle_vq` through `virtio_queue_set_notification`), so a landed write
leaves `0xfffe`. No fault record is promised. The device reads the word before writing it, and
QEMU's AMD-Vi model refuses the write from the read's cached translation without logging an event
(`amdvi_do_translate`'s IOTLB hit). So the test asks whether the write landed, which every unit
answers alike. Under TCG QEMU handles a kick synchronously in the vCPU thread, so either result is
in place when the kick returns.

## What a driver bug, and a driver's malice, cost

The virtio driver is a process. A wrong descriptor into its own memory, a mis-parsed superblock, a
bad pointer: each faults the driver alone (7a's `user_fault`), the kernel reports it, and the
machine keeps running. That is fault isolation, and it was true from milestone 9.

Malice needed a second fix, and it now has one. There is no IOMMU on QEMU `virt`: the device
is a second bus master doing DMA against raw physical addresses, with no MMU in front of it, so
page-table permissions do not apply to it. A hostile driver that could program the queue and ring
the device itself could point a descriptor at the kernel image or another process's frames and the
device would read or write it. So the kernel keeps the two DMA-critical powers: programming the
ring addresses and the "go" signal, and validates that every descriptor stays within the
driver's own DMA region before the device sees it. The driver operates the device through a
`Virtio` capability (status, features, submit) but cannot aim it outside its region. A malicious
descriptor pointing at kernel memory is refused, end to end, and the device is never told to go.
See [dma.md](dma.md).

## What it prints

```
      a driver at EL0 read the file 'motd' off a virtio disk,
      through a nifefs superblock it parsed itself,
      woken by the device's interrupt delivered as a message.
      the kernel issued no virtio command and touched no DMA.
```

## nifefs

The filesystem is [`crates/nifefs`](../crates/nifefs/src/lib.rs): read-only, flat, fixed
everything, host-tested, with one definition of the format shared by the disk-building tool and the
reader. Block 0 is a superblock (magic, a small directory of name → start-block/length); files
follow, block-aligned. The driver reads block 0, walks the directory to find `motd`, and reads its
data block. It is deliberately the least thing that is still a real filesystem, in the same spirit
as `crates/elf`: milestone 9 is about drivers and block I/O, not filesystem design.

The driver is the format's third reader (the kernel and `xtask` are the others), and the only one
that walks the directory without holding the whole image. It has one block buffered, so it can see
`nifefs::ENTRIES_IN_FIRST_BLOCK` entries and no more. It used to restate the offsets by hand and
was the one place that had to be found by hand when the entry layout widened on 2026-08-01. It now
depends on `nifefs` for them, which is CLAUDE.md's rule 7 (what two binaries must agree on is a
crate). See [nifefs.md](nifefs.md).
