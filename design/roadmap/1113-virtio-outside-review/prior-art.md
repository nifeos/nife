# Prior art

An appendix to [the milestone](../1113-virtio-outside-review.md).

How the crate sits against what a reader of other systems already knows. Recent downloads are the
last 90 days at the time of the review.

| system | shape | what a reader coming from it will miss here |
|---|---|---|
| Linux `drivers/virtio/` (`virtio.c`, `virtio_ring.c`, `virtio_mmio.c`, `virtio_pci_modern.c`) and `drivers/block/virtio_blk.c`, `drivers/net/virtio_net.c` | one transport and ring library, one device driver per class, the ring library owns the handshake and the descriptor free list | a ring library any device driver links; here each driver restates the handshake |
| OpenBSD `sys/dev/pv/virtio.c`, `virtio_mmio.c`, `virtio_pci.c`, `vioblk.c`, `vio.c` | the same split: `virtio.c` holds negotiation and the virtqueue, `vioblk` and `vio` are thin class drivers | the same |
| Redox `virtio-core` with `virtio-blkd`, `virtio-netd`, `virtio-gpud` | a userspace library crate plus one daemon per class, the shape nife's own reuse record cites | the library half; here the library-named crate holds the daemons' bodies and test roles |
| `virtio-drivers` 0.13 (97k recent, 433k all-time downloads) | `transport` (mmio, pci), `queue`, `device::{blk,net,gpu,input,...}` modules, `no_std`, a `Hal` trait for DMA allocation | a `Hal` seam, a queue type, typed errors; its ring placement is why nife's reuse record refused it |
| `virtio-queue` 0.18 (1.98M recent) and `virtio-bindings` 0.2 (2.16M recent), rust-vmm | the device side of the ring, and bindgen constants from the Linux headers | the constants: every register offset, status bit and feature bit this crate spells by hand is in `virtio-bindings` |
| `virtio-spec` 0.4 (17k recent) | typed definitions from the specification, `no_std`, no driver | typed register and feature definitions |

Where nife differs, and why the difference is the point: the kernel places every ring at a fixed,
Kani-proved layout (`direct_memory_access_validator::{LAYOUT_QSIZE, DESC_OFF, AVAIL_OFF, USED_OFF,
RING_BLOCK}`). It programs the queue addresses itself (`SETUP_QUEUE`). It validates every descriptor
before the device gets the signal to go (`NOTIFY`). A driver never chooses a ring address. That rules out
`virtio-drivers`' queue as recorded in
`design/roadmap/0784-reuse-what-the-tree-wrote/the-virtio-drivers-stay-ours.md`. It does not rule
out a library of nife's own holding the part every nife virtio driver shares. That part is the
register vocabulary, the status and feature bits, the handshake, the descriptor and ring accessors
at the fixed layout, the blk request chain and the net header. That part exists six times in the tree
today, and the crate named `virtio` is not where it lives. The concrete example is
`crates/virtio/src/lib.rs` lines 47 to 105 beside `components/src/virtio_net_transport.rs` lines 54 to 87: the same
constants, spelled twice, with `virtio_net_transport` recording the copy as a bug and this crate
recording nothing.

The crate name `virtio` appears unclaimed on crates.io (one API call returned no crate; unverified
beyond that).
