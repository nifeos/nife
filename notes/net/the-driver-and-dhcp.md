# The network stack: the confined NIC, the driver, and DHCP

*An appendix to [`notes/net.md`](../net.md), which is the page to read. This file holds pieces 1 and
2 of milestone 30, the smoltcp pin, and phase A of piece 3. It exists to verify or challenge the
main page. A reader who only needs to use the socket contract should not have to open it. The
directory `notes/net/` and this file's stem are provisional names; naming is calef's.*

*Records cited below: milestone 30 (the network stack as a confined component), milestone 27 (Rust
`std` on the native ABI), milestone 32 (a real filesystem) and milestone 291 (thirty-one programs
wearing one name). Also §18 (the PCIe transport) and §23 (multi-queue DMA confinement).*

## Piece 1: multi-queue confinement (built, both ISAs)

The disk uses one virtqueue; a NIC uses two (receive on queue 0, transmit on queue 1). The §18
transport seam and the shadow-ring validator were queue-0-only. Piece 1 grew them to N queues
(N = 2 today, fixed and asserted) under the same confinement discipline, so the driver work sits on
proved ground rather than a NIC forcing a retrofit.

The mechanics are in notes/dma/directions.md ("Multiple queues, and the receive direction") and
DECISIONS §23.
The short version:

- `setup_queue(id, num, queue)` and `notify(id, queue)` take a queue number; the `Virtio`
  capability's methods grew an argument rather than gaining new methods, so the surface stays narrow
  and the disk's ABI is byte-identical (it passes queue 0).
- Queue `q`'s rings live at `q * RING_BLOCK` (0x200) in both the driver's DMA region and one
  kernel-private shadow frame. Per-queue last-validated index; per-queue PCI doorbell.
- The validator did not change. It bounds descriptor addresses, not directions. Receive is the
  direction where the device *writes into* driver memory, and the same in-region check that stops a
  read descriptor aimed at the kernel stops a receive descriptor aimed there. This is the property
  milestone 32's block write already relied on, now proved for the device-as-writer direction.

Tests (both ISAs): `the_validator_refuses_an_rx_descriptor_that_escapes_the_region` and
`a_second_queue_validates_on_its_own_block`, beside the existing confinement suite.

## Piece 2: the virtio-net driver (built, both ISAs)

A NIC driven from EL0, behind Piece 1's confinement, the same shape as the disk driver. The kernel
enumerates the device (`find_net_device` beside `find_block_device`, on the mmio bus in
kernel/src/virtio.rs and the PCI bus in kernel/src/pci.rs. The enumeration structs were generalized
from block-specific to transport-neutral, since a register base and an interrupt are all the kernel
hands a driver either way). It owns the registers and the two DMA-critical powers, and hands the driver
a confined `Virtio` capability, a DMA page, and an interrupt. On PCIe the NIC sits behind the IOMMU
(`iommu_platform=on`), the disk's pattern exactly, so it is confined in hardware too.

The driver is `crates/virtio`'s `run_net`, dispatched by `components/src/block_driver.rs` on every
architecture (the aarch64 tests ran it as a role of `hello` until milestone 291, which found that
role to be this same binary's code reached through a second dispatch table). It brings up both
virtqueues (receive = 0, transmit = 1) through the
one capability, passing the queue number to `SETUP_QUEUE` and `NOTIFY`. The whole net-specific DMA
layout (two ring blocks at the kernel's 0x200 stride, a receive buffer, a transmit buffer) fits in
the single 4 KiB DMA page the spawn service already hands every driver.

The proof is a DHCP round trip, no TCP/IP stack in the loop. Post a receive buffer, hand-build a
DHCP DISCOVER (Ethernet + IPv4 + UDP + BOOTP, broadcast flag set), transmit it, and receive the OFFER
that QEMU user-mode networking (slirp) sends back. The driver parses the OFFER and reports the offered
address (`yiaddr`), which the test asserts lands in slirp's 10.0.2.0/24. A valid OFFER for our
transaction id is the only path to that report, so a match proves the DISCOVER left (TX) and the OFFER
returned (RX), across both queues and both directions of the confinement. Tests (both ISAs, both
transports): `a_userspace_driver_completes_a_dhcp_round_trip_over_virtio_net` and its `_pci` twin.

The runners attach two NICs (mmio + PCI-behind-IOMMU) on slirp when `NIFE_NET` is set, which xtask
sets for both test legs. slirp needs no host file, so unlike the disk there is nothing for the runner
to fail loud on; the manufactured-fact hazard (a NIC asked for but not enumerated) is caught by the
test asserting the exchange rather than skipping.

## smoltcp: the pin, and a corrected assumption

Pin: smoltcp 0.14.0, bumped from 0.13.1 (current on crates.io at 2026-07-28) on 2026-08-24 in `b41b5b4b8`, `default-features = false`. Features
to enable: `proto-ipv4`, `proto-dhcpv4`, `socket-tcp`, `socket-udp`, `medium-ethernet`. Divergence
policy is the vendored-engine discipline (DECISIONS §18 point 3, and the RedoxFS pin): pin the
version, carry any patch as a recorded diff, note the reason. No patch is known to be needed yet;
smoltcp is no_std-clean and used across embedded Rust.

Corrected assumption. smoltcp bills itself as "for bare-metal, real-time systems without a
heap." It can run with fixed socket buffers and a static `SocketSet`, so the net server does not
strictly need the untyped-backed `GlobalAlloc` that RedoxFS (milestone 32) and the `std` PAL
(milestone 27) require. In the build we shipped, net_stack does use `alloc` (over user_mode_runtime's `UntypedHeap`,
milestone 27) because it is available and makes the socket set and per-frame buffers simpler. The
`alloc` feature is a convenience, not a precondition, so a fixed-capacity server remains possible if
that heap were ever unavailable.

## Piece 3 phase A: smoltcp doing DHCP over the confined NIC (built, both ISAs)

The net server, `net_stack` (components/src/net_stack.rs), is the networking form of the userspace-reuse thesis: a
real, reused TCP/IP stack (smoltcp 0.13.1, not hand-built) running entirely at EL0 over a NIC the
kernel confines by DMA. The kernel knows nothing about DHCP.

- `components/src/virtio_net_transport.rs` presents smoltcp's `phy::Device` over the receive/transmit virtqueues. It brings
  the NIC up through the `Virtio` capability, posts receive buffers, and copies received frames out (RX
  tokens own their bytes so they never borrow the device). It transmits via the DMA ring (TX tokens
  carry a raw pointer to the device, sound because net_stack is single-threaded and the device outlives
  any token within a poll).
- `net_stack` links `alloc` over user_mode_runtime's `UntypedHeap`, builds a smoltcp `Interface` and a DHCP socket,
  and runs the poll loop, blocking on the NIC interrupt between polls. It reports the acquired
  address, which the test asserts lands in slirp's 10.0.2.0/24 (`the_net_server_acquires_a_dhcp_lease_over_smoltcp`
  and its `_pci` twin, both ISAs). Only a real DHCP handshake driven by smoltcp over the confined NIC
  produces that.
- The spawn service (`virtio_service::start_net_server{,_pci}`) grants net_stack the confined transport,
  the interrupt, a DMA page, a report endpoint, and an untyped budget for the heap, plus extra
  stack pages for smoltcp's packet building.
- Caveat (recorded): the DMA region is one 4 KiB page, so the buffers are small and the MTU is
  small (`virtio_net_transport::MTU`, 576). DHCP, DNS, and small TCP segments fit; a full 1514-byte frame does not. A
  larger MTU needs a multi-page contiguous DMA region, which the spawn path does not build yet. This
  is a demonstrator limit, not a protocol one.

DHCP is itself UDP, so smoltcp's UDP path over our NIC is exercised end to end by this test. What is
not yet built is the client-facing socket contract that lets *other* processes use the stack.

## What `virtio_net_transport` takes from the device on trust, and what it copies

A receive completion carries two 32-bit values the device wrote into the used ring: the descriptor
`id` and the byte count `total`. Neither is trusted (notes/shared-page-audit.md, finding 6). The
IOMMU and `crates/direct_memory_access_validator` confine where the device may touch. They say
nothing about what it may say, and the used ring sits inside the driver's own DMA page, which the
device is entitled to write.

An unchecked `id` walks `rx_buf(id) = 0x400 + id * 0x2C0` out of the one-page DMA region. At
`id = 4` it is already past the page. Near 1.5 million it lands on the process's heap, so a lying
device could have the driver copy its own memory into a frame and hand it to smoltcp, which may put
it on the wire. An unchecked `total` reads past the buffer and asks the allocator for up to 4 GiB.
Neither needs a race; the device only has to lie once.

So `rx_take` fails closed. A completion naming a buffer never posted is consumed and dropped, and a
count larger than a buffer is truncated to the buffer, as `entropy.rs` clamps its own length. The
dropped completion's buffer is not re-posted, because a bogus `id` does not say which buffer it
was. Each lie costs one receive buffer. That is the right trade: a device that lies has already
stopped being a network card.

The DMA-page layout (`RX_DESC` to `TX_USED`, at a 0x200 stride) restates the validator's
`DESC_OFF`, `AVAIL_OFF`, `USED_OFF` and `RING_BLOCK` as literals. The kernel aliases those
constants, while `entropy.rs` and `crates/virtio`'s own net driver spell the same numbers again.
Nothing checks the copies against the crate. Consuming the constants takes a `components` to
`direct_memory_access_validator` dependency edge, which is a §46 (thin primitives or whole
subsystems) decision. Milestone 410 (six copies of the shared-frame accessors) priced that edge on
2026-10-16 (UTC) and did not take it. The same pricing left the driver's `r8`, `w8` and sibling
aliases in place: each is a one-line forward over `MappedWindow` with no arithmetic of its own.
