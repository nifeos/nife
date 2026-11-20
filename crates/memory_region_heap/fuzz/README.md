# The fuzz target

An unpublished fuzz workspace with one target, `memory_region_heap_sequence`. It drives talc the
way `memory_region_heap` does, from a byte string, and checks the heap after every step. talc is
built with its `counters` and `error-scanning-std` features, so its own consistency scan runs
before each operation.

The target drives a copy of the crate's policy (`src/policy.rs`), not `MemoryRegionHeap` itself,
because it reads talc's counters and heap end, which the crate keeps private. The copy lists every
place it differs. Its `PageSource` maps one page at a time from a host arena and refuses when its
page budget runs out.

## The input

A four-byte header (arena size in pages, page budget; two little-endian bytes each) and up to 512
operations. An operation initializes, commits all, allocates, frees, reallocates, sets the budget,
reads every block back, or allocates a block that ends at the top of the heap. Sizes come in
classes: empty, small, a page, a mebibyte, the cap and just past it, powers of two and their
neighbors, and sizes near `isize::MAX`. Alignments run from 1 to 64 KiB, with the 64-byte cache
line on its own bit.

## What it checks

After every operation:

- every live block has the alignment it asked for, lies inside the committed pages and overlaps no
  other;
- the first and last 16 bytes of every block hold its own pattern, and the whole block does when
  the target frees, moves or reads it;
- reallocation keeps the old contents up to the smaller size;
- committed bytes are whole pages, never past the cap, and equal to the pages mapped;
- talc's heap end is the top of the committed pages;
- talc's counters agree with the target: live allocations, live bytes, claimed bytes no more than
  committed, at most one heap;
- an initialize after the first effective one changes nothing;
- the heap refuses an allocation only when it has no region, too little room under the cap, or no
  budget left;
- `commit_all` reports success only when the heap holds its whole cap;
- AddressSanitizer sees no access to a page the heap has not mapped.

At the end the target frees every block and checks that talc counts none live.

## Running it

From the crate's root, with a nightly toolchain and `cargo-fuzz`, which finds this directory by
its name:

```sh
cargo fuzz run -O memory_region_heap_sequence fuzz/seeds/memory_region_heap_sequence -- -max_total_time=240
```

The seeds replay as an ordinary test, and under Miri, which checks every pointer talc computes
against its allocation:

```sh
cargo test --manifest-path fuzz/Cargo.toml --test seeds
MIRIFLAGS="-Zmiri-permissive-provenance -Zmiri-disable-isolation" \
    cargo +nightly miri test --manifest-path fuzz/Cargo.toml --test seeds
```

`python3 fuzz/seeds/make-seeds.py` rewrites the seeds. Each names one shape a heap meets, such as a
budget that runs out, a second initialize, or a block asked to grow past the arena's end.

## BUGS

- A change to the crate's policy that the copy misses goes unchecked.
- The copy reads every refusal as a spent budget. A refusal for another reason, which stops the
  crate's heap for good, is not modeled.
- The spinlock is out of scope: the target is single-threaded.
- A heap whose top is the top of the address space, where a pointer sum wraps, cannot exist on a
  64-bit host. The target puts blocks at the top of its arena instead; only Miri can tell that talc
  formed a pointer past the arena there.
- A run reaches about 120 inputs a second, because talc's consistency scan runs before every
  operation and the target fills and reads every block byte by byte.
