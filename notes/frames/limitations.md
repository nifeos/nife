# PageFrame capabilities: the BUGS list in full

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes, after
`notes/soak/limitations.md`.*

An appendix to [notes/frames.md](../frames.md), whose BUGS section lists each entry in one line.
This is each entry with its history and its measurements. The one entry that opened a design fork,
a `PageFrame` that names a run of pages, has its record in [run-fork.md](run-fork.md).

## The sweep that guarantees a run capability dies is not the one `REVOKE` uses

A capability naming a run outlives nothing it should not. Found 2026-08-27 by the adversarial
review of milestone 142 (a text display good enough that people use it instead of a GUI), and
fixed. `MemoryRegion::DESTROY` used to delete capabilities by *exact
object*, one mapped page at a time. So `PageFrame(base, 311)` matched nothing and survived the
reclamation of every page it named. A holder could then re-map pages the allocator had already
handed out as a page table or another process's stack. That is the use-after-free of §13 (capability revocation and untyped reclamation) with the
widening's name on it.

Reclamation now sweeps by overlap over the whole range
(`sched::delete_page_frame_caps_overlapping`). That closes an older hole the same way. A
capability to a page nobody had *mapped* was never a candidate before, because the sweep was
driven by the mapping log and the log had no record of it. `PageFrame::REVOKE` deliberately keeps
the exact-object sweep; see the next entry.

## A `REVOKE` on one run used to reach mappings it did not authorize

`revoke::unmap_everywhere` is space-blind. It pulls a physical page out of every address space
that maps it, regardless of which capability produced that mapping. That was equivalent to
"capability-scoped" while a `PageFrame` named exactly one page and could not overlap another. §102 (a Frame names a run of pages)
made overlap possible, and the tree's display wiring uses it. The gpu driver holds
`PageFrame(dma, 312)` while the painter and terminal hold `PageFrame(dma + 4096, 311)` over the
same memory. That is deliberate: the driver's capability must span the DMA window it registered
with the IOMMU. So revoking the terminal's run unmapped those pages out of the driver's space too,
under a capability nobody had revoked.

Fixed 2026-08-27 (design/decisions/0132-overlapping-page-frame-runs.md, option C, decided rather
than deferred to the first `GRANT`). Each mapping record now carries the base address of the object
it was made under. That names the capability *and every capability derived from it*, because
`derive` never changes the object. `PageFrame::REVOKE`'s unmap half matches on it
(`revoke::unmap_under_object`).

`revoke::LogEntry` grew from 16 bytes to 24 and `LOG_ENTRIES` fell from 255 to 170. So a space
pays about 1.5x the log pages it did. The existing `assert!(size_of::<LogPage>() == FRAME_SIZE)`
holds that arithmetic to account, and it was checked by breaking it. Reclamation and the device
take-back stay object-blind on purpose, because neither is asking a capability's question.

## An overlapping capability still survives a `REVOKE`

That is the decision rather than the gap. Revoking `PageFrame(dma + 4096, 311)` leaves the
driver's `PageFrame(dma, 312)` in place, naming pages the revoke has unmapped out of the revoked
capability's holders. §102 contemplates two capabilities coexisting over sub-ranges of one region.
The alternative is to sweep capabilities by overlap, as reclamation does. That would let a one-page
holder delete a 312-page capability by naming any page inside it. §132 (what `PageFrame::REVOKE` owes an overlapping run) refused it for that reason.

## A run and a longer run sharing a base are one object to the log

`PageFrame(p, 401)` and `PageFrame(p, 74)` are different objects to `Cap` and identical to
`revoke::LogEntry`. So revoking the shorter unmaps the longer's mappings of the pages they share.
Carrying the length would take `LOG_ENTRIES` from 170 to 127 to separate a pair nothing in the tree
mints. Bounded in the safe direction (the over-broad unmap cannot reach outside the revoked run) and
named in `LogEntry`'s own `BUGS`.

## Revocation still says nothing to a device

§132 deliberately did not answer this half. A driver's IOMMU/DMA window is registered as a byte
range (`virtio::register(.., dma, DMA_PAGE_FRAMES * FRAME_SIZE, ..)`) and is not derived from any
capability. So revoking a `PageFrame` narrows what the *CPU* may reach. It leaves device-initiated
DMA into the same physical pages exactly as it was. Even a capability-perfect revocation of a
surface leaves the device able to write those pages until the driver's virtio registration is
itself torn down. Whether `REVOKE` should narrow a DMA window couples two objects that are
independent today. It is the last open section of the decision above and remains an architect's
call.

## A failed multi-page `MAP` gives the page tables back to nobody

Both mapping paths (`page_frame::MAP` and `address_space::MAP_INTO`) are all-or-nothing across a run
since 2026-08-27. A failure partway through unmaps the prefix it had already mapped and tombstones
its revocation records. So a caller that gets an error never has to wonder how much of its run
landed. The budget is not rolled back with it. Whatever L3s (and parents) the prefix needed were
retyped out of the caller's region, and a region is spend-only, so those pages stay spent. The
mapping is undone; the memory is not returned. A caller that retries a large run after an
`OutOfMemory` should expect to have less budget than it started with, not the same.

## A run capability names more than its object, by up to one page

`SURFACE_PAGE_FRAMES` is `SURFACE_BYTES.div_ceil(4096)`. At 1120x624 the surface is 2,795,520
bytes in 683 frames, so the capability names 2,048 bytes past the end of the surface. Those bytes are
inside the same contiguous allocation (the driver's DMA region), not another object's memory. And
nothing addresses them: every consumer bounds-checks against `SURFACE_BYTES` rather than the frame
count (`user_mode_runtime::mapped_window::MappedWindow`).

It is recorded because a capability that names more than the thing it stands for is exactly the
kind of fact this convention exists to write down. It is also recorded because the
`SURFACE_BYTES % 4096 == 0` assertion that used to hide the question by construction was deleted
this milestone. `crates/graphics_contract/src/lib.rs` has the search that justified deleting it.

## Not everything migrated

The console is deliberately last. A bootstrap that needs a capability service to print cannot
report its own failure, so it is its own decision with its own argument. The compositor path is not
in this milestone at all. `display_terminal` therefore maps its own page frames in `MODE_DISPLAY`
and still receives spawn-time mappings in `MODE_WINDOW`.

`date` keeps its `Spawn::maps` clock page in the kernel's test wiring. It is the one place in the
tree where both mechanisms appear in a single spawn literal: a `page_frame_cap` whose only job is to
be probed for presence, beside a `Mapping` that does the actual work. Migrating it means touching
the shell's spawn path and the grant manifest of §67 (a program's second stream is a declaration), which is a bigger change than the wart. Note that
the *shell* spawns `date` through `AddressSpace::MAP_INTO`, which is recorded. So the revocability
gap there is in the test wiring rather than in the real path.

## Each migrated program costs one more untyped region

`untyped::create` takes a contiguous run from the page frame allocator and holds it for the
process's life, and the region table has a finite number of slots. Eight pages apiece is negligible
next to what these programs already reserve (`mkfs` takes 384). A small contiguous request is easy
where the 128-page one of milestone 107 (the socket contract learns to accept) was not. But it is a reservation added to a machine whose page
frame pool that milestone found at the edge.

## The mapping records cost the process, not the kernel

Every `PageFrame::MAP` writes a record into a log page retyped from the *address space's* own
backing region. The untyped named in the call pays only for page tables. That is 255 records per
page and `AS_OVERHEAD` is sixteen pages of slack, so nothing here comes close; a program that maps
thousands of page frames would notice. (Corrected 2026-10-11 UTC: the 255 was written before the
2026-08-27 fix above, which took `LOG_ENTRIES` to 170 records per page. The conclusion stands.)
