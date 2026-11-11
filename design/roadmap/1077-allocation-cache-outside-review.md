---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
## Goal

A stranger who finds `allocation_cache` in its own repository can tell in one screen what it is for. The crate is a bounded per-owner stash of freed small blocks in front of any locked heap, so most allocations
and frees never take the lock. The same stranger can build and test it from the README alone. One example shows how to drop it in front of a heap of their own. The repository's gates are green on the first pull request for the crate's own reasons. The public surface at the first tag is
the surface the crate means to keep.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1077-allocation-cache-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1077-allocation-cache-outside-review/prior-art.md).

## The items

Effort: S is under half a day for a lane, M a day or two, L longer. A door is one-way when somebody
outside the repository acts on it after the first tag (a public name, a published behavior, a layout, a dependency). Everything else is two-way, and the maintainer decides it. The crate has no
release, so a public rename is one-way only from the tag on; the items say "one-way at the tag"
for those.

### P0: blocks the move

1. The manifest and the files do not stand alone.
   Finding: `standalone.py` from the template lists ten findings, and `script/crate-standalone` agrees. The crate has no README, no `LICENSE-MIT`, no `LICENSE-APACHE` and no `rust-version`. It sets `license.workspace = true` and `lints.workspace = true`, which inherit from a workspace the repository will not have. A manifest comment cites a milestone (`Cargo.toml:14`). The crate documentation holds a path into this tree (`src/lib.rs:8`). The crate also carries three provenance blocks and one architect's name (`src/lib.rs:32-34, 84, 99`). `cargo package` adds that the manifest names no `repository`, `documentation` or `homepage`, and it has no `description`, `keywords` or `categories`. The
   `publish = false` comment points at `crates/publish-allowlist.txt`, a file of this tree.
   Change: the scaffold's README shape (`README-crate.md`) filled from this crate. It holds one line of purpose, `## EXAMPLES` with the doc example and a second one that puts the cache in front of a heap, `## Building and testing` with the three bare-metal targets, `## BUGS`, `## Minimum Rust version` and `## License`. Add the two license files. Set `license = "MIT OR Apache-2.0"`, `rust-version = "1.88"` (verified to build), `description`, `repository` and `categories = ["no-std", "memory-management"]`. Add an inline `[lints]` table carrying `unsafe_op_in_unsafe_fn`, `missing_docs` and `unexpected_cfgs` with `cfg(kani)`. The `publish = false` comment says what a stranger can act on. The move strips the `Name:` blocks (`helpers/crate_move.py`,
   `moved_source`); the rest is the lane's.
   Door: two-way. Effort: S.

2. The crate documentation addresses one consumer, and names it.
   Finding: the crate root says what `std` does with the cache and how `cargo xtask std-src` copies the file, and under which path (`src/lib.rs:3-4`). It sends the reader to `notes/std/allocation.md` for why one thread owns a cache (`src/lib.rs:8`). The `BUGS` entry says "A thread's cache is
   spilled at its end" (`src/lib.rs:29`), which is the consumer's behavior and not the crate's. The crate has `drain`, and nothing calls it. `heap_layout`'s doc explains the rule by talc's chunk grid (`src/lib.rs:95-97`), while the rule is general. A heap sees one layout at both ends of a block. The manifest's dev-dependency comment names the `std` copy and `user_mode_runtime`
   (`Cargo.toml:14-16`). A stranger reads the whole thing as a fragment of another project. The
   standing rule for crates that leave is that they do not talk about their consumers.
   Change: rewrite the crate documentation for a stranger. It says what a class is and states the three obligations of the owner. First, one owner at a time. Second, `refill` and `spill` run under whatever excludes other users of the heap. Third, every block a `spill` or `drain` hands back goes to the heap under `class_layout(class)`. It says why a cached block is fungible within its class. It also says why `heap_layout` must shape the uncached path too (any heap that rounds or tags its own way). The talc tag and
   chunk grid become the README's example. `BUGS` says the crate-level fact: a `Cache` dropped
   while it holds blocks leaks them, because it cannot free, and the owner must `drain` first. The
   reasons for a per-owner design belong in the README's design section, not in a link.
   Door: two-way. Effort: S.

3. No workspace member depends on the crate, so the `std` farm loses it on the day it moves.
   Finding: `grep -rn allocation_cache --include=Cargo.toml` finds only the workspace member list
   (`Cargo.toml:106`) and the crate's own manifest. The review packet's dependents table is empty.
   The `std` farm finds each copied package through `cargo metadata` over the workspace
   (`xtask/src/farm/copied.rs`, `copied_package_directories`), and the move runbook states the consequence. With no member depending on the moved crate, `cargo xtask std-src` stops and names
   the missing package. Every other copied crate has a dependent in the tree; this one is the case
   the runbook describes and no crate has yet hit.
   Change: the move's nife-side pull request gives one workspace member a git dependency on the
   crate at the tag's commit, so `cargo metadata` lists it. The natural owner is `xtask`, the package that copies the file. A dependency there costs one host build of a 431-line crate and states the relationship in the one manifest that already names the file. The alternative, a
   `std_farm_sources` manifest that depends on every copied crate, is a general fix for the other
   thirteen when they leave, and is a separate milestone if wanted. The runbook's expected checks
   (`cargo tree --locked -i allocation_cache` prints the git source, `cargo xtask std-src` exits 0)
   already cover it.
   Door: two-way. Effort: S.

### P2: later

The P2 items are in [the appendix](1077-allocation-cache-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1077-allocation-cache-outside-review/before-the-tag.md).

## Decisions for the architect

Only one-way doors. Each is plain background first, the recommendation, then prior art and one
example.

### Decision 1: Whether the crate gets a repository of its own

Background: the standing rule narrows which crates leave. A crate gets its own repository only if it would earn a crates.io publication on its merits or more than one repository consumes it. Otherwise it stays in its package's repository, and promoting it later is a split, the easy direction. This crate has
one consumer, the `std` overlay in the `runtime` package, reached by a generated copy rather than a
dependency. On merits: the survey behind it found no pure-Rust `no_std` per-thread cache on
crates.io, so the crate fills a gap, and it is 150 lines with no dependency. Against: its parameters follow one consumer's choices (item 13). Its documentation addresses that consumer (item 2). It has no proof (item 8). No allocator in the prior-art table publishes its thread cache as a library.

Recommendation: the crate moves with the `runtime` package's repository when that package leaves,
not on its own now. Items 1 to 9 still land, because strangers read the crate wherever it lives,
and a later split then costs one afternoon. The alternative, a repository now, is defensible if the architect judges the crates.io gap worth filling ahead of a second consumer. Then the rest of this block is the move's worklist, and item 3 becomes the first thing the move pull request does.

Example: `nifeos/runtime` holds `allocation_cache/` beside `user_mode_runtime/` in one workspace;
nife's `xtask` depends on it by `git` and `rev` on that repository, and `cargo xtask std-src` reads
the copied file from Cargo's checkout.

### Decision 2: The crate name and the five constant names

Background: the crate name is provisional and unruled. It names what the thing holds (cached
allocations) rather than its owner, so it does not say "thread", which is right for an algorithm
that never reads an owner id. The word `cache` is a generic one in an operating system (a page
cache, a CPU cache, a disk block cache), which the naming rules warn against; here `allocation_`
qualifies it. The constants are in item 7.

Recommendation: keep `allocation_cache`. The nearest alternatives are worse: `free_block_cache`
reads as a disk block cache, `thread_cache` names the owner, `allocation_stash` trades a word
every allocator paper uses for one none does. For the constants: `GRID_BYTES`,
`MAXIMUM_CLASS_BYTES` (for `LARGEST`), `BLOCKS_PER_CLASS` (for `DEPTH`), `BATCH_BLOCKS`,
`MAXIMUM_CACHED_BYTES`; `CLASSES` stays. For the methods: `push` and `pop` in place of `put` and `take`. Std and `heapless` spell a bounded stack that way, and `put`'s `Result<(), T>` on a full class is `heapless::Vec::push`'s shape. `drain` either returns an iterator of `(Class,
NonNull<u8>)` or takes the name `give_back_all` so it does not claim std's meaning. The glibc
spellings `tcache_put` and `tcache_get` go in the documentation as the terms a reader searches for.

Prior art: glibc's `tcache_count` and `tcache_max_bytes`, jemalloc's `ncached_max` and
`tcache_max`, TCMalloc's `max_length` and `num_objects_to_move`: each names the thing counted or
the unit. None of the three names its grid, because none has a fixed one.

Example: `let class = class(48, 8).unwrap(); cache.push(class, block)?; let again =
cache.pop(class);` and `const _: () = assert!(size_of::<Block>() > MAXIMUM_CLASS_BYTES);` in the
consumer's thread block.

### Decision 3: A `Class` newtype in the public signatures

Background: item 4. Every method takes the class as a bare `usize` that the caller got from
`class`, and the crate trusts it: an index past `CLASSES` panics on bounds, and `class_bytes` on
one overflows. The highest rung the tree's own ladder offers is to make the wrong state
unrepresentable: `class` returns `Option<Class>`, where `Class` is a `#[repr(transparent)]` newtype over `u8` with a private field. Every method, `class_bytes` and `class_layout` take it. The
consumer's `cache_for` already carries the pair `(&mut Cache, usize)` and changes one type.

Recommendation: the newtype, before the tag. It removes two panic paths from the allocator's hot code and costs no instruction (the index is still a byte). It also reads as the API means: a class is something `class` mints, not a number a caller makes up. The alternative, bare `usize` with a
documented contract, is a one-way door the other way, because adding the newtype after the tag
breaks every caller.

Prior art: `slotmap` and `thunderdome` mint their keys and refuse a raw integer. jemalloc's `szind_t` is a typedef that the code never derives from user arithmetic. `heapless` indexes by
`usize` because its collections are general, which this one is not.

Example: `pub struct Class(u8); pub const fn class(size: usize, align: usize) -> Option<Class>;
pub fn pop(&mut self, class: Class) -> Option<NonNull<u8>>; pub const fn class_bytes(class: Class)
-> usize;`

### Not brought

Items 5, 6, 8, 9 and 12 change no public name and carry a recommendation; the maintainer decides
them. Item 3's choice of which manifest carries the dependency is two-way. Item 13 waits for a second
consumer.

## Done when

The checks are in [the appendix](1077-allocation-cache-outside-review/done-when.md).

## BUGS

- Decision 1 may close this block early: if the crate stays with its package, items 1, 2 and 4 to 9
  still apply where it lives, and item 3 does not.
- The prior-art table's bin counts and depths come from memory, unverified. The shapes (a cache per owner, a batch under the lock, a flush at the owner's end) are the point, and one sitting
  with each source settles the figures.
- Item 10 did not reproduce the undefined behavior `vendor/README.md` records on the unpatched talc.
  The review cannot say whether the record is stale or Miri's current build misses it; the weekly
  job will.
- The review did not run Kani, a mutation census, the stranger test or the `std` farm after a
  simulated move. Item 3's failure mode comes from reading `copied.rs` and the runbook, not from a run.
- `notes/project-metrics/mutation-rowless-survivors.csv` still lists `class, replace > with >=`
  for this crate, a mutant the two boundary tests now kill according to the last two censuses; a
  nife housekeeping row, not the crate's.
- The tests assume `u128` is 16-byte aligned, which holds on current toolchains for the host; the
  tests do not run on the bare-metal targets.

## Index row

An outside review of `allocation_cache` before any move. Three items would turn the new
repository's first pull request red or stop the `std` farm, six land before the first tag, and six
follow later. The six cover three arithmetic edges on bare `usize` indices and a zeroed-is-empty contract the consumer relies on. They also cover the owner's obligations `put` leaves unstated, five constant names without units or things, the absence of any proof, and an alignment the code relies on without asserting. Three decisions wait for the architect: whether the crate leaves on its own at all, the
crate and constant names, and a `Class` newtype in the public signatures.
