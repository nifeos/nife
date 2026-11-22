# Open decisions

An appendix to [the milestone](../1050-designware-mobile-storage-outside-review.md), which lists
each one and its state. One decision per section, ordered by what blocks the first publish.

## A shared DMA region trait: provisional answer

Option A, provisional: `designware_mobile_storage` keeps its own `DirectMemoryAccessRegion` for its
first release. Nothing has published, and the answer creates no repository and adds no dependency, so it
is a two-way door that the maintainers take; the release review of the first of the three
drivers to publish rechecks it. The brief below is the reasoning.

## A shared DMA region trait

Background. A DMA region is the memory a device reads and writes on its own: here, the page of
IDMAC descriptors and the data pages behind it. The driver never touches memory itself, so it asks
the caller for the region through a trait, `DirectMemoryAccessRegion` in
`src/internal_direct_memory_access.rs`. That trait has `u32` offsets, reports its bus address and
length, reads and writes 32-bit words and byte runs, and carries `barrier`, the ordering point the
driver calls before it starts the controller and after the controller finishes. Its `# Contract`
section states the coherent and the non-coherent case, and the driver does no cache maintenance of
its own.

Why now. The review recommended one trait per crate "until a third consumer exists". That trigger
has passed: three organization crates now define a trait of the same name. `nifeos/e1000e` and
`nifeos/designware_ethernet` (both `src/data_plane.rs`, read on `main`) define the same four
methods with `u64` offsets and 64-bit words, no bus address or length, and put `barrier` on a
separate `Tails` trait beside the two tail-pointer writes. They also refuse a device whose DMA is
not coherent, where this crate allows it. Once published, a shared trait is a public contract that
every driver and every outside implementation depends on. So the shape has to settle before the
first publish of any of the three. No crate here has published, so today the change is a rename and a
dependency.

The constraint. The two shapes differ for reasons, not by accident. The driver's own register
writes inside `HostController` start the IDMAC, so the region is the one place that knows when
ordering is due. A network data plane hands the start to a tail write the caller owns, so its
barrier lives there. The IDMAC builds 32-bit descriptors, so its region must sit below 4 GiB and
`u32` offsets state that in the type. A shared trait would either carry the union (bus address,
length, both word widths, a barrier the network crates do not call) or the intersection (byte runs
only), and each driver would wrap it. It is also one more crate with its own repository, review and
release, ahead of all three drivers.

Costs. Keeping three traits costs each implementer one small adapter per driver: the kernel
already writes one for each (`kernel/src/designware_mobile_storage.rs`,
`components/src/designware_ethernet_transport.rs`, `components/src/e1000e_transport.rs`), about
forty lines apiece. A shared crate costs a new repository, a review, a held first publish for all
three drivers until it lands, and a contract that is harder to change once three crates pin it.
Because every crate is before 1.0, a later 0.2 of each driver can still adopt a shared trait as an
ordinary breaking release.

The question: does `designware_mobile_storage` keep its own DMA region trait for its first release?

- **A (recommended).** Yes. Each driver keeps its own trait for its first release. The milestones of the two network crates decide whether they share one trait, not this one.
  A second storage driver that needs one reopens the question, and a shared crate arrives, if at all,
  as a 0.2 of each driver. This is the stricter choice: no new public crate, and a later release can still add a shared trait.
- **B.** A shared crate now (provisional name `direct_memory_access_region`) holding the union
  shape, adopted by all three drivers before any publishes. Costs the repository and review above
  and holds all three first publishes behind it.
- **C.** A shared crate for the two network drivers only, whose shapes already match, with this
  crate keeping its own. Decided in 1074 rather than here; listed so the constraint is visible.

One-way or two-way: two-way until the first publish of any of the three; one-way after it (no one takes back a published trait or crate name).

Prior art, read for this decision: Linux has one DMA API for every subsystem
(`<linux/dma-mapping.h>`, docs.kernel.org/core-api/dma-api.html): coherent memory from
`dma_alloc_coherent()`, streaming mappings with `dma_map_single()`, and the cache maintenance of a
non-coherent device in `dma_sync_single_for_device()` and `dma_sync_single_for_cpu()`. Drivers share
the allocator and the sync calls, not a per-driver view of descriptor memory. In Rust,
`embedded-dma` 0.2.0 shares only buffer traits (`ReadBuffer`, `WriteBuffer`) that promise a stable
address for a transfer; it says nothing about caches or descriptor layouts, and each HAL builds its
own DMA channel API on it. From memory, not re-read: U-Boot's `dw_mmc` and Linux's `dw_mmc` and
`stmmac` each lay out their own descriptors over that common allocator.

What the tree already does: each driver crate defines the trait it needs, and the kernel adapts its
own mapping to each, in the three files named above. No shared DMA trait exists in nife.

What the answer touches: the first publish of this crate, at whose release review it is rechecked.
Nothing else in the crate waits on it.
