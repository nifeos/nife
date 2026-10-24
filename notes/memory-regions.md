# Memory regions: the kernel stops allocating

*Renamed 2026-08-24 (DECISIONS §113, milestone 158): the kernel object this file describes was
called `Untyped` (seL4's own word for it) until this rename. It is now `MemoryRegion`, chosen
because it names what the capability actually is (a run of physical memory) rather than seL4's
own word for what has not yet been done to it. This file is rewritten throughout to use the
current name; the historical number in "The one number that proves it" is untouched because it is
a measurement, not a name.*

Milestone 11, and DECISIONS §10's deliberately-deferred third axis. It is the strangest idea in
the whole project, and the one that makes seL4 verifiable: **a kernel that does not own a pool of
memory to hand out.**

## The idea

Normally a kernel has an allocator. Ask it for a page and it finds a free one and gives it to you.
Which means a process, by asking enough, can make the kernel allocate until it runs out, and a
kernel out of memory is a dead machine. Kernel-memory exhaustion is a whole class of attack:
fork bombs, fd floods, socket buffers, every one of them is "make the kernel allocate on my
behalf until it can't."

Memory regions remove the allocator from the hot path. A process holds a capability to a chunk of
raw physical memory (a **memory region**), and to get a page it retypes part of that memory
into the page. The kernel is a bookkeeper: it advances a watermark and hands back a physical
address. It does not choose a page from a pool it owns, because it owns no pool. Every page a
process spends comes from the region it was handed.

## The one number that proves it

```
  milestone 11: a process mapped 23 pages out of a memory region it was handed,
  and the kernel's used-frame count went 991 -> 991 (it did not move).
```

That flat frame count is the whole thing. The process allocated twenty-three pages of memory, and
the kernel's free memory did not change, because the pages came out of the process's own
region, carved once at the start. A process cannot make the kernel allocate, so it cannot exhaust
kernel memory. It runs out of *its own* budget, the retype returns `OutOfMemory`, and the kernel is
untouched. There is a test that asserts exactly this equality, and it fails loudly if the memory
comes from anywhere but the region. With the kernel allocator as the source, the same process maps
thirty thousand pages and the kernel loses thirty thousand frames.

## How it works here

- `Object::MemoryRegion(region)` is a capability to a region: a run of physical pages and a bump
  watermark. `kernel/src/memory_region.rs` is the whole allocator: `retype_page` advances the
  watermark and returns the next page, zeroed, or `None` when the region is spent.
- `invoke(region, MAP, va)` retypes one page and maps it, writable, at `va` in the caller's own
  address space. Both the page and any page tables it needs come from the region (the mapper's
  only source of memory is a closure that bumps the watermark), so the kernel allocates nothing.
- The region's backing is carved from the frame allocator once, when the region is created.
  That single allocation is the seL4 boundary, where all free RAM becomes a region handed to the
  first process. Everything after spends it.

## What a retype mints

Since milestone 824 (a retype mints no right its budget lacks), on calef's ruling (a) of 2026-10-07
(UTC) on PR #1798: a frame or kernel object retyped from a region carries the region capability's
rights, no more. `RETYPE` and every `RETYPE_OBJ` arm mint through `Cap::mint_child`, the inheriting
mint `SPLIT` already used and `split_never_widens_rights` proves. Before 824 each minted
`Rights::ALL`, so a holder of a budget without `GRANT` could carve a page and lend it. `GRANT` is
now delegated down the budget tree and never minted at a leaf.

seL4 gives a retyped object full rights over itself (recalled, not read). nife departs from that
because its leaf budgets withhold `GRANT` on purpose, and a full-rights mint made the withholding
cosmetic. Revoking afterwards was refused with the ruling: the window opens at the mint.

What a budget carries is now what everything made from it carries, so three shapes matter:

| Budget | Rights | Who holds it |
|---|---|---|
| The root (`cap::memory_region_root_cap`) | every right | the progenitor, which hands `BIND` and `GRANT` on |
| A lending budget (`abi::rights::ALL`, `cap::memory_region_lending_capability`) | every right | a builder, a shell, a program that lends a page it retyped: a networked program's socket pages, `jig`'s staged package |
| Spend-only (`abi::rights::SPEND_ONLY`, `cap::memory_region_cap`) | every right but `GRANT` | a leaf |

The spend-only budget was `WRITE` alone until 824. It gained `READ`, `ENUMERATE` and `BIND` so a
leaf can still wait on its own notification and join its own process. On a region those rights
grant nothing new: no method consults `READ` or `BIND`, and `ENUMERATE` answers `USAGE`, the
holder's own spending.

`BUGS`: a builder that lends what it carves now holds `GRANT` on its budget, which is also the right
to lend the budget. The sharpest case is a networked `std` program (`std_layout.rs`'s slot 3): it
needs `GRANT` to lend socket pages. So it may also hand its whole job region to a server it holds,
which could then spend or destroy the program's heap. It reaches nothing outside its own grant. A
separate socket-page region split off the job region would narrow this to the pages lent, at the
cost of a region per networked job and a reclaim that must destroy the child first.

## Where the boundary sits

Both the memory a process asks for and the memory it is made of come from regions.
`MemoryRegion::MAP` pages are retyped from the caller's region. `exec` carves one region per
process, from which the address space's root, its tables and its image pages are all retyped, so
teardown is `memory_region::destroy` and the whole budget returns in one call. The kernel's own
objects draw from the kernel's own budget (`kmem`, [kernel-budget.md](kernel-budget.md)), and an
object a process creates is retyped from its creator's region
([thread-control-block.md](thread-control-block.md)).

The table and every decision over it are `crates/memory_regions`: host-testable, Kani-proved for
the arithmetic, and searched by loom for the racing case (`script/interleaving-check`).
`kernel/src/memory_region.rs` keeps only what a crate a model checker runs must not have, the I/O:
the frame allocator, the direct map, the revoke and the lock. The division is not tidiness. A
double free in the frame allocator's caller was a protocol bug, and a protocol that lives in a
`no_std` kernel module is reachable by neither of this project's verification tools.

## How a region is reclaimed

Destroying a region returns its whole backing, and the name goes with it: the slot's generation is
bumped, so every `MemoryRegion` capability minted for it stops resolving, and the slot is reused by
the next `create` or `split` ([generational-names.md](generational-names.md)).

Returning retyped pages is safe because `destroy` first revokes every mapped page in the region
(`revoke.rs`, §13 (capability revocation and untyped reclamation)): each is unmapped from every
address space that held it, and every `PageFrame` capability to it is deleted. No live mapping
survives, so a peer cannot be left mapping memory the allocator hands out again
([capability-lifecycle.md](capability-lifecycle.md), [teardown.md](teardown.md)). A root region's
pages go back to the frame allocator; a child's go back to its parent's budget, never to the
allocator, so a page reaches the allocator only through the one root that owns it.

`RegionTable::claim_for_destroy` decides and removes the name in one borrow, so at most one caller
reaches the free loop for a region. Two callers can hold a name for one region (an owner's
`MemoryRegion::DESTROY` and a supervisor's `rendezvous::REAP`, both landing in
`sched::reclaim_region`). With a gap between the check and the removal, each would pass the
refusal check and free the same pages ([object-revocation.md](object-revocation.md)).

Two lock orders hold this together. `REGIONS` is released before the revoke, because revocation
takes `IPC_TABLES`, a higher rank. And `unpin` is separate from `destroy`: tearing down a region's
objects needs `IPC_TABLES`, while `destroy` must never take it, because it is reachable from
`AddressSpace::Drop`, which already runs under the reaper's `IPC_TABLES`. So reclaiming is the reap
(with `IPC_TABLES`), then `unpin` and `destroy` (without).

## How many regions can be live

`memory_region::MAX_REGIONS` (256) bounds regions live at once, not creations over the kernel's
lifetime, since slots are reused. Every suite run prints the peak on its `regions:` line, with the
test during which it was set. Measured 2026-09-26:

| architecture | peak | spare | live when the first test starts | live after the last |
|---|---|---|---|---|
| aarch64 | **225** | 31 | 1 | 222 |
| riscv64 | **224** | 32 | 1 | 221 |
| `x86_64` | **125** | 131 | 1 | 122 |

The table is nearly empty at boot. Almost every region live at the peak is one an earlier test left
behind (on aarch64, `ntp_tests` 39 and `login_tests` 33 lead, then the compositor, file service and
date stacks), and the peak is a few above that residue. A full table refuses quietly (a `--mem`
split hangs). `design/roadmap/0676-the-ntp-and-login-tests-give-their-regions-back.md` is the work
to account for the residue. There is no gate: as with `sched::MAX_THREADS`, a climbing peak is
usually a boot doing more, and a gate needs a held list like the frame ledger's
([frames.md](frames.md)), which regions do not have yet.

## The `memory_regions` crate

The kernel's allocator (`kernel/src/memory_region.rs`) is a bump watermark per region, with a
parent and child tree from `SPLIT` and a LIFO return of pages on `DESTROY`. The property that
matters is no double free: a physical page reaches the frame allocator at most once across any
sequence of create, split and destroy. `crates/memory_regions` holds the decisions that property
rests on, extracted so Kani can prove them and the kernel calls the proved code rather than
keeping a parallel copy (the discipline in notes/verification.md).

- `split_new_watermark` carves a child off a parent's budget by advancing the parent's watermark.
  A successful carve stays within budget, does not overflow, and strictly progresses, so
  consecutive carves are disjoint runs within the parent.
- `destroy_outcome` decides a reclaim. A region with a live object (`pinned`) or a live child
  refuses, and a root frees its run to the allocator. A child never frees to the allocator: its
  pages go back to the parent (a LIFO un-bump, or a hole). That last rule is the crux. A page
  reaches the allocator only when the root that owns it is destroyed, and each root owns one
  contiguous run from one allocation.

### The claim

The gap "How a region is reclaimed" describes is not hypothetical: riscv64 showed it on two cores as
`double free of frame 0x82a3e000`. `claim_for_destroy` takes `&mut self`, so the gap cannot be
written. That is rung one of the ladder in `AGENTS.md`: the caller holds either the table or a
claim, never a decision about a table it no longer holds. Removing the slot before the caller's I/O
also protects everything else that resolves the name, since a retype on a region whose pages are on
their way back to the allocator fails.

`DestroyClaim` is the exclusive right to reclaim one region's pages. Only `claim_for_destroy` mints
one, it has no public constructor, and it is neither `Copy` nor `Clone`, so the kernel's free loop
runs against a value nothing else can hold. Its fields are what the caller needs for the I/O the
table cannot do: revoke the mappings, then free the run to the frame allocator (a root) or give it
back to the parent (a child, through `return_to_parent`). Loom checks the claim
(`script/interleaving-check`). Kani does not model two callers.

### Returning a child

`return_to_parent` runs the LIFO test against the parent's current watermark in the same borrow
as the un-bump, so nothing can carve from the parent in between. A child at the top of the
parent's run gives its pages back and the run can be split again.

A child returned out of order leaves a hole only until nothing above it is live. Without that, a
bump-only pool (the progenitor's job pool) leaks a job's pages every time two jobs end in the order
they started (notes/swish-check-flake.md). So after the un-bump the watermark drops further, to the
highest page anything still holds: a live child's end, or the region's own retypes
(`retyped_to`). `coalesced_watermark` is that rule, proved.

The drop happens only when no sibling is in flight. A sibling that has been claimed but not yet
returned has left the table while its pages are still being revoked, so the scan cannot see where it
sits. Dropping the watermark under it would let the parent carve pages a peer may still map, the
use-after-free of §13 (capability revocation and untyped reclamation). The parent's child count
tells, because it drops at the return and not at the claim. A count above the live children the scan
found means a sibling in flight (or the phantom child below), and then only the LIFO un-bump
happens. The next return with nothing in flight reclaims the holes, because the scan starts from the
live set each time. The count dropping at the return is also what keeps the parent from being
reclaimed while this child's pages are in flight. Decrementing at the claim would reopen the claim's
window one level up the tree.

`spent` counts `Frames` and each `Object` kind over the whole subtree. A job budget's own pages are
almost all carved into job regions, and the objects live in those, so a count of the budget alone
would say its threads cost nothing. `Children` is the region's alone, since it is exactly the pages
the subtree counts are drawn from.

## BUGS

- **`Error::OutOfMemory` collapses three causes into one code.** `SPLIT` returns it when the
  caller's own budget is exhausted, when the caller's capability table is full, or when
  `MAX_REGIONS` is exhausted; `RETYPE_OBJ` collapses a different pair the same way (each method's
  doc comment in `crates/abi`). The first two are facts about the caller; the third is a fact about
  every other live region on the machine, which the caller did not cause and cannot fix. DECISIONS
  §119 (splitting `OutOfMemory`'s three causes is declined for want of a customer) declines it and
  records non-binding guidance for whoever has a customer: cause-specific `Error` variants, the
  shape of `crates/timetable`'s `Unbacked`/`Refusal` split and POSIX's `EMFILE`/`ENFILE`.
- **A split refused for a full table still bumps the parent**, which can then never be reclaimed.
  The phantom child also stops `return_to_parent` coalescing, so the parent keeps only the LIFO
  rule. `RegionTable::split`'s `# BUGS` has the reproduction, and
  [region-split-on-a-full-table.md](region-split-on-a-full-table.md) the proposal.
- A hole under a live child is not reused while that child lives, because `split` only bumps. A
  parent whose children always overlap (each carved before the one below it returns) never drains,
  so its holes never close. With every reap held back 150 ms, the riscv64 swish-check pool fills. A
  carve that searched the holes first would close it, at the price of the same scan on every split.
  It is not built, because no real workload has shown the overlap.
