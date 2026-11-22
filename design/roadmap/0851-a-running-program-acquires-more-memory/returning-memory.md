# Returning memory while running: the measurements behind forks 3 and 7

An appendix of [a running program acquires more memory](../0851-a-running-program-acquires-more-memory.md).
It holds the arithmetic, so the main text can state the decisions. Written 2026-10-06 (UTC).

## The region table's peak, per architecture

Milestone 601 (the region table prints its peak) made every suite run print a `regions:` line: the
most regions live at once, of `MAX_REGIONS`, and the test during which the peak was set. The 225 in
`kernel/src/memory_region.rs`'s ledger came from that line on 2026-09-26, on `main` at `484f3ebe`.

Read again on 2026-10-06 (UTC) from the `test` job of three merge-queue CI runs (37512763962,
37512623317 and 37512004040). All three agree:

| suite | peak | spare | set during |
|---|---|---|---|
| aarch64 | 242 | 14 | `timetable_tests::a_calendar_entry_keeps_time_by_its_granted_clock_and_a_step_moves_it_by_s3` |
| riscv64 | 241 | 15 | the same test |
| x86_64 | 136 | 120 | the same test |
| x86_64, a second machine configuration | 112 | 144 | the same test |

Each architecture also runs a kernel-only pass that peaks at 8. The suite's peak is mostly residue:
regions earlier tests left behind, which the ledger itemizes. The peak rose 17 on aarch64 in ten
days with nothing in this proposal built.

An interactive boot's own count was not measured. It is far lower than the suite's, so the suite
is the binding case for `script/test` and the ceiling a stranger meets first.

## Why a program's regions are bounded by its memory

Band increments start at 256 pages and large allocations at the 1 MiB threshold, so no region a
program gets is under 1 MiB. A program under an account ceiling of C MiB holds at most C regions.
That is the per-program limit fork 7 recommends: memory runs out first. Under QEMU's 256 MiB that
is at most a few hundred regions machine-wide for every program together.

The capability table is the limit one level down. It has 64 slots a process
(`kernel/src/cap.rs`, `CAPABILITY_TABLE_SLOTS`), and a `std` program's 9 fixed slots come first.
A program that kept one capability per region would meet 64 long before memory. It need not keep
them. It maps a grant's pages, deletes its copy of the capability, and later releases the grant by
the number the broker gave it. The broker's copy is the one that `DESTROY`s. The cost is one slot
for the length of a mapping loop. Milestone 778 (capability tables sized per process) stays the
general answer for programs that hold many capabilities for other reasons.

## The threshold

glibc's 128 KiB default (from memory, with its dynamic rise toward 32 MiB on 64-bit) is priced for
`mmap`, whose cost is a per-process VMA. Here a large allocation costs:

- a broker round trip: a `CALL`, then the grant over the result endpoint, since a reply carries no
  capability (`moho_abi::reply`);
- a `SPLIT` in the broker and a `DESTROY` at free;
- one region slot machine-wide, and one capability slot while it maps the pages;
- one `MAP` per page, the same as in the band.

At 1 MiB, the 256 `MAP` calls dominate, and the fixed part is a few percent at most. That is an
estimate; the first slice's measured criterion counts it. At 128 KiB the fixed part is a third of
the cost or more, and slot use is eight times higher for the same bytes.

## What the region table costs, measured

Measured 2026-10-06 (UTC) on calef's Mac (Apple silicon, host `release` build) with a throwaway
program over `memory_regions::RegionTable<N>`, not committed. Kernel code under QEMU runs slower,
so treat these as lower bounds.

| `MAX_REGIONS` | table size | flat `USAGE`, root | flat `USAGE`, leaf | a destroy at a full table |
|---|---|---|---|---|
| 256 | 29 KiB | 0.4 µs | 0.5 µs | 0.3 µs |
| 1,024 | 116 KiB | 1.4 µs | 2.4 µs | 0.8 µs |
| 4,096 | 464 KiB | 5.7 µs | 7.4 µs | 4.4 µs |
| 16,384 | 1.8 MiB | 23 µs | 49 µs | 45 µs |
| 65,536 | 7.3 MiB | 103 µs | 127 µs | 73 µs |

The size is exactly 116 bytes a slot. "Flat" is one root with every other slot a child of it, the
shape a broker with many accounts makes. There each walk is linear in the table.

The worst case is a chain, each region split from the last, because `spent` runs `descends_from`
for every live region and each walks its parent chain:

| chain length, table full | `USAGE` at the root | `USAGE` at the leaf |
|---|---|---|
| 256 | 62 µs | 83 µs |
| 1,024 | 748 µs | 685 µs |
| 4,096 | 39 ms | 30 ms |

That is quadratic and worse at size, since the walk leaves the cache. The 2026-10-03 audit measured
the 256 row at 42 and 62 µs, the same order. They are linear only over a flat table;
over a chain they are not. Each runs under the region lock with interrupts masked.

Inserting is linear too. `generational_table::insert_with` takes the first free slot by scanning
(`slots.iter().position`).

## Making the walks stop depending on the table

Three changes, all inside `crates/memory_regions` and `crates/generational_table`:

- First-child and next-sibling links in each region record. `spent` then walks the named region's
  subtree and nothing else, so its cost is what the caller holds. The chain's root walk becomes
  linear in the chain and the leaf walk constant.
- `return_to_parent` finds the highest live child by walking the parent's children rather than the
  table.
- A free-slot list in `generational_table`, so an insert is one pop.

What that touches in the proofs. The four Kani proofs in `crates/memory_regions` are over scalar
watermark functions and survive unchanged. `generational_table`'s four harnesses run on
`Table<u8, 2>` and rerun against the free list. The loom test in `script/interleaving-check` runs
on `RegionTable<2>` and reruns the claim and return. The links are new invariants, so new Kani
harnesses on a small table should prove them consistent after split, claim and return.

## Removing the table, seL4-style, priced roughly

seL4 keeps an untyped's state in its capability and revokes through a derivation tree linked
through capability slots (from memory). The table's only limit there is memory.

Here the table is also the revocation mechanism. A region's name carries a generation, and
reclaiming bumps it, so every capability to it stops resolving with no tree to walk
(`notes/generational-names.md`). Moving the record into memory the holder owns would lose that:
a reused page could carry a matching generation by chance. So option B means a derivation tree
and a redesign of §16 (object revocation). The code it touches is `kernel/src/cap.rs` (808 lines),
`kernel/src/revoke.rs` (1,374), `kernel/src/memory_region.rs` (382), `crates/memory_regions`
(1,761) and `crates/generational_table` (573), about 4,900 lines. The tree's invariants need new
proofs. That is months, a rough estimate. Sizing the table from RAM at boot would postpone the need
for it indefinitely, at the cost of a slice-backed table and Kani harnesses that take one.

## How the allocator sees a free increment

The allocator keeps one record per band increment: its region's capability slot, its start and its
length, in the order granted. `user_mode_heap` gains two pure calls, host-tested like the rest of
it: the highest free block, and withdrawing a range from the free list.

After a `dealloc` that coalesces into the highest free block, the allocator compares that block's
start with the newest increment's start. If the block reaches down past it, the increment is wholly
free. If the increment below is wholly free as well, the newest is withdrawn from the free list and
released to the broker, which `DESTROY`s it and uncharges the account. The cursor drops by its
length. The broker does the `DESTROY` so the account's books and the pages move in one place.

The check is cheap when the free list knows its tail, which it can keep beside its head. Without
that it is a walk of the free list on every `dealloc` that touches the top.

The job's own build region is never trimmed. It is the baseline the account returns to.

## Reserve and commit: what a reservation costs

A reservation is the runtime's own bookkeeping. The kernel keeps no record of it, so it costs no
kernel memory and needs no kernel bound. The address space is the bound. Above the map's 2 GiB,
the low half is 256 TiB on aarch64 (`T0SZ` 16), 128 TiB on x86_64 (split at bit 47) and 256 GiB
on riscv64 (Sv39), less the timebase page near the top of each. So 32 GiB fits on all three.

Page tables are the cost, and they are paid at commit, not at reserve. `MemoryRegion::MAP` takes a
page's tables from the same region as the page. `PageFrame::MAP` takes them from a region the
caller names (`crates/moho_abi`). That difference matters, because a region's `DESTROY` cuts every
table it paid for out of the address space, with everything mapped beneath it
(`kernel/src/revoke.rs`, the page-tables-outlive-destroy pass). If a committed region paid for a
table that also reaches a neighbor's pages, decommitting it would unmap the neighbor. So commit
retypes its pages as `PageFrame` runs and maps them with tables from one table region per program,
which lives until the program exits. Today's band, which maps with `MemoryRegion::MAP`, has the
same hazard once it trims, so it moves to this rule too.

What the tables cost, with 4 KiB pages on all three: one last-level table page per 2 MiB span ever
committed, and one more per 1 GiB span touched, plus a top-level page on the four-level layouts.
That is about 0.2% of what was committed. The 32 GiB exit criterion's 16 scattered commits cost at
most 33 table pages, 132 KiB.

The bound. Tables stay with the table region until exit, so a program that sweeps commits across
a huge reservation keeps paying, up to 0.2% of the span it touched: 64 MiB for all 32 GiB. That is
charged to its account, so its ceiling stops it, and no other program pays. The table region grows
from the account in 1 MiB steps, like any other region, so fork 7's bound on regions still holds.
Giving tables back early would need one table region per reservation, a region per reservation
whatever its size. Not proposed; recorded in BUGS.
