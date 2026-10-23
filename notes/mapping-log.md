# The mapping log: how revocation finds every mapping

`kernel/src/revoke.rs` keeps a record of every mapping of an untyped-derived page, and the
capability each mapping was made under. Reclamation and `PageFrame::REVOKE` both read it. This note
carries the design the file's doc comments link to. The rulings are §13 (capability revocation and
untyped reclamation), §102 (a Frame names a run of pages) and §132 (what `PageFrame::REVOKE` owes
an overlapping run). [How authority moves, narrows, and ends](capability-lifecycle.md) is the
introduction to revocation, and [object revocation](object-revocation.md) covers the objects a
process builds.

## Two scopes, because two questions are asked

Reclamation asks whether a page is safe to hand out again, so it is object-blind.
`memory_region::destroy` unmaps a page from every address space that holds it and deletes every
capability whose run overlaps the range. Anything less is the use-after-free §13 exists to prevent,
because the allocator is about to give the memory to somebody else.

`PageFrame::REVOKE` asks whose authority is being taken back, so it is scoped to a capability. It
reclaims nothing, since a region is spend-only. A mapping made under a different capability over the
same physical memory is somebody else's authority, and it survives.

The two answers diverge because of §102. Once one capability can name a run, two capabilities can
overlap. The display wiring has three that do: a driver holds `PageFrame(dma, 312)` and two clients
hold `PageFrame(dma + 4096, 311)` over the same memory. A client's revoke must not reach into the
driver's space.

seL4 keeps a capability-derivation tree and revokes a subtree. This kernel keeps no tree. The
object address in each record stands in for one, because `Cap::derive` narrows rights and never
changes the object. One word therefore matches a capability and all of its derivatives.

## Who pays for the records

Each address space's records live in log pages retyped from that space's own region. A fixed
registry of live spaces (root, region, TLB tag, log head) finds them. A process that maps a
thousand shared pages spends its own budget recording them. A process that cannot afford a record
cannot make the mapping, and the mapper unmaps what it just mapped.

Teardown needs nothing extra. The log pages are region pages, so `memory_region::destroy` reclaims
them with the process, and forgetting a space is one registry slot going empty.

The cost reaches callers that size a region. `log_pages_for(n)` is how many log pages `n` mappings
need. The loaders that map a whole initial ramdisk, thousands of pages, have to budget for those
records as well as for the page tables reaching them.

## The record, and the one case it cannot separate

A record is three words: the physical page, the virtual address, and the object. The object is the
base address of the run the mapping was made under. A mapping made with no capability records the
page itself as its object. `MemoryRegion::MAP` retypes and maps in one step, and the kernel's own
loaders map pages into a space they are building. In both cases there is no derivation family for a
revoke to be scoped to, so the page really is the whole of what was mapped. `PageMapSource` makes
each call site say which case it is; most of the kernel's mapping sites map a page no capability
names.

Two capabilities that share a base but not a length are one object here. `PageFrame(p, 401)` and
`PageFrame(p, 74)` are distinct to `Cap`, and a revoke of the shorter one unmaps the longer one's
mappings of the pages they share. Carrying the length would cost a fourth word. A `LogEntry` would
round up to 32 bytes and a log page would hold 127 entries instead of 170. That is twice the log
pages a space pays for today rather than 1.5 times, to separate a pair nothing in the tree mints.
The failure is bounded in the safe direction: the over-broad unmap reaches only pages inside the
revoked run, and it is narrower than a space-blind unmap.

A record whose object is the marker `TABLE` is a page table, not a mapping. `PageFrame::MAP` and
`MemoryRegion::MAP` build a space's intermediate tables out of a region the caller names, which need
not be the space's own. Without a record, destroying that region would hand back a table the space
still links, and the page's next owner would be writing that space's translations. The marker is
the value 1, because a real object is page aligned and at least `0x4000_0000`. A fourth word for one
bit would cost 43 entries a log page. Every reader of a record must ask `is_table`.

## What the log costs, measured

Recording a mapping scans the head page's used slots for a tombstone before it appends. The
average scan is half of `LOG_ENTRIES`, and the cost of a record is linear in it. Taking
`LOG_ENTRIES` from 255 to 170 moved `map_el0` by 16.9% on aarch64 (464,182 to 385,919 ticks) and
16.4% on riscv64 (73,990 to 61,868), measured on 2026-08-27 (UTC) by changing that one number. A
change to the constant moves both benchmarks, which then need re-recording.

Walks loop over a chain's pages and then over each page's slice. A `flat_map` across the chain cost
`map_el0` 39% and `spawn_el0` 19% on aarch64 icount against the floor (2026-10-08 UTC).

The registry carries one past its highest live slot, so a walk costs the occupied prefix rather than
`MAX_SPACES`. Without that bound, doubling `sched::MAX_THREADS` from 128 to 256 cost `spawn_el0`
138,584 ticks through this file, with `forget_root` the worst site. See
[the benchmarks note](benchmarks.md).

## One critical section: read, map, record

Every sweep runs two passes in one order: capabilities first, under `IPC_TABLES` and each table's
lock, then the unmap pass, which takes the mapping registry and unmaps what the log holds when it
scans. A `MAP` that read its capability, then mapped and recorded in separate critical sections,
would let a sweep run wholly in between. That is the use-after-free again.

`MappingHold` is the registry held across the whole operation, which is seL4's answer: a
mapping-database lock. `MappingHold::current_cap` reads the running thread's capability under the
hold, taking its table lock beneath the registry (`CAPABILITY_TABLE`, 57, under `MAPPINGS`, 59). A
sweep deletes from the table before it takes the registry. So the sweep is either wholly before
the read, and the `MAP` answers `NoSuchSlot` as if it had started after the revoke, or its unmap
pass starts after the record and finds it. Unlike record-then-recheck, there is no instant where a
reclaimed page is mapped and about to be taken back. `system_tests::user::map_revocation_window_tests`
drives it.

The cost is page-table construction under the hold. Tables come from a region (`MEMORY_REGION`,
58, beneath this), and a run maps every page under one hold, so every other mapping and unmap pass
waits. `MAP` is spawn-time and setup work, never a step of the IPC round trip. The cost under
contention is not measured. The read is protected by convention, not by type: a caller can still
read a capability with `sched::current_cap` outside a hold and map with it. The `MAP` handlers
receive only the slot, which is the same defense `sched::Delegation` uses.

A page table is recorded before the mapper links it, under the same hold. `revoke_region`'s cut
pass takes the same registry, so it runs either before the retype, when the region is claimed and
the retype fails, or after the link, when the record finds the table.

## Listing what a space maps

`abi::address_space::LIST` answers from the log rather than from the page tables, the way `ps`
reads `/proc`. The kernel already keeps the record, so the answer costs nothing more and agrees with
what a revoke would find. The log records no permissions, so the syscall layer reads each address's
flags with `arch::mmu::translate_at`.

A cursor is a log page's physical address OR'd with an index into it. Pages are only ever prepended
to a chain and never freed until the space dies. So a page prepended after a walk starts is never
reached by it, and a page already visited is never revisited, because the chain runs toward older
pages and a cursor only advances. Since `page_phys` is never 0, `page_phys | index` is a valid
resume position even past a page's last entry.

A resumed cursor is the caller's word, so it is checked for membership before anything follows it.
The check walks from the head and stops at the wanted page, and the listing resumes from there. One
registry hold covers finding the space, the check and the walk, so a space cannot die and have its
page come back as another space's log in between. Without the check a caller-chosen cursor would
name any kernel-mapped page. A slot a later record reused is not detected, because a log entry
carries no generation; `crates/pmap` records that limit.

## Revoking a run

`revoke_page_frame_run` deletes capabilities by one exact-object match against `(phys, count)`.
That is the one capability, and its narrowed derivatives, this invocation can be revoking. The
unmap stays per page, because the log has one record per mapped virtual page. Both passes are
scoped to the invoked capability's family: a client revoking `PageFrame(dma + 4096, 311)` leaves the
gpu driver's `PageFrame(dma, 312)` mappings alone.

An overlapping capability keeps its authority and its own mappings over pages this call unmapped
from the revoked run's holders. That is deliberate. §102 lets two capabilities coexist over
sub-ranges of one region, and a one-page holder able to delete a 312-page capability by naming a
page inside it would hold more authority than one page grants; §132 refused that as option B.

Revocation owes a device nothing. `PageFrame::REVOKE` is a CPU-side operation. The gpu driver
registers its DMA window with `virtio::register`, that window is derived from no capability, and
revoking a capability does not narrow it. So the device can write those pages until the driver's
virtio registration is torn down. Coupling `PageFrame` and `Virtio` is an architect's call, left
open by §132.

`revoke_page_frame` is the one-page case. It deletes only an exact `PageFrame(phys, 1)`, so a run
that merely contains `phys` survives it, which is why reclamation does not use it.

## Taking a device back

`revoke_device_from_others` is §41 (the endpoint is the broker, and a device is revoked by taking it
back) for milestone 23 (a capability-routed component OS with live replacement). It deletes every
`DeviceFrame` capability naming the page except the invoker's, and unmaps the page from every space
but the invoker's.

Revoking a frame exists to make reclamation safe, so the revoker loses its own capability too. A
device page is never reclaimed. Revoking it makes ownership exclusive, which live replacement needs
between tearing one driver down and endowing the next. Deleting the invoker's capability would leave
the registers unreachable forever, since the kernel mints a `DeviceFrame` once, at boot.

This is one level of a derivation tree: the invoker is the root by construction, and every other
holder is treated as a derivative. Revoking one named holder while sparing another would still need
the real tree. The take-back scopes by holder, not by capability, and §132 left it so: scoping the
unmap by object would spare a second capability to the same MMIO page, and the device would have two
owners. The swap suite (`user::live_swap_tests`, `LOG_REVOKE_ENFORCED`) is the end-to-end evidence.

## Destroying a region

`revoke_region` runs in two passes at two granularities. The capability pass covers the whole range
once and matches by overlap, so a run capability over the region's pages goes, and so does a
capability to a page that was never mapped and has no record.

The unmap pass takes every recorded page in range in one scan per space, under the registry, and
invalidates each space once by its tag. Per-page invalidation would be a cross-core shootdown each
on riscv64 and x86_64, 683 of them for the scanout. A `MAP` cannot add a record in range meanwhile:
it would need a capability, and the sweep has finished, or a retype from a claimed region, which
refuses.

Then the tables. Each table the region paid for is cut out of the walk that reaches it, the space's
tag is flushed, and the records of everything beneath are tombstoned. The space keeps running and
loses that span, as it already loses a leaf the region paid for. One table is taken per scan, so the
scans strictly shrink, and a table below one already cut is simply dropped. A second, table-only
scan cost `spawn_el0` 3,072 ticks a spawn on aarch64 icount (+21.5%), so one scan finds both kinds.

A space's root is a page table too, and nothing here cuts it. `CONFIGURE` binds a space to a thread
whose TCB can come from another region, and `sched::reclaim_region` reaps a bound space only through
its thread. Destroying the space's region while the thread lives frees the root it runs on. calef
ruled on 2026-10-05 (UTC) that the thread dies as a resident, recorded in §16 (object revocation);
milestone 765 (a destroyed region cannot free the root a running thread walks) builds it.
