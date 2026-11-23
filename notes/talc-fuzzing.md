# Fuzzing talc, the heap under every program

*Name: provisional, minted 2026-11-14 (UTC) by lane talc-fuzz-and-miri.*

Every nife program that allocates runs on `talc` 5.1.1, carried in `vendor/talc` with one upstream
fix, through `memory_region_heap` (see [heap.md](heap.md)). talc is mostly `unsafe` pointer
arithmetic, and Miri has already found one real defect in it: the fit check in `full_search_bin`
and the in-place grow check formed a pointer past the end of the heap before comparing it.
SFBdragon/talc#57 fixes both, and `vendor/talc.divergence.patch` carries that fix. This note says
what checks talc now, what each check can and cannot see, and how to run them.

## What upstream's fuzz targets check

talc's repository has three libFuzzer targets in `fuzz/fuzz_targets/`, at the commit
`vendor/talc.pin` names. They build against `vendor/talc` unchanged, with talc's `counters` and
`error-scanning-std` features.

| Target | What it drives | What it asserts |
|---|---|---|
| `manual` | a `Talc` with a 25-bin test binning: allocate, free, reallocate, claim, extend, truncate, read the reserved range | talc's consistency scan before each operation; at the end, no live allocation and one heap at most |
| `global_alloc` | allocate, free, reallocate through the `GlobalAlloc` source over the system allocator | the consistency scan and the sanitizer |
| `allocator` | the same operations through the `Allocator` source | the same; its own header says nothing uses it |

talc's consistency scan checks the gap list, the sizes, the bins and the tags, and that no two
gaps overlap. None of the three reads a block back. They fill each block and never check the bytes, so a block
handed out twice shows only if the consistency scan or the sanitizer notices. A coverage run of
`manual` and `global_alloc` reaches `full_search_bin`, its fit check and the in-place grow check.

These targets cannot find the defect #57 fixed. That code added a size to a pointer and compared
the result with the heap end. On a 64-bit host the sum never wraps, so the comparison gives the
right answer and nothing touches memory. AddressSanitizer sees only accesses. Only Miri, which
checks every pointer computation against its allocation, reports it.

## What Miri finds in talc's own tests

`cargo miri test` and `cargo miri run --example` cover talc's eleven unit tests, its doctests and
its five examples, under the Stacked Borrows model the tree's own Miri job uses. They pass with the
fix and without it. talc's own tests never reach the failing case, so a check that the fix stays
fixed has to come from this tree.

## The in-tree target: `memory_region_heap_sequence`

`crates/memory_region_heap/fuzz/` is an unpublished fuzz workspace with one target. It drives talc
the way a nife program's heap does and checks the heap after every step.

Its `nifeos/nife:crates/memory_region_heap/fuzz/README.md` holds the input format, every invariant
it checks and its BUGS. It drives a copy of the crate's policy rather than `MemoryRegionHeap`,
because it reads talc's counters and heap end, which the crate keeps private.

## How to run it

Every command runs from the repository root, under the heavy-slot helper, with a memory cap. A run
writes any failing input to `$HELD`, a private directory outside the repository with mode 700.
Findings stay there, and a report names only their kind, signature and SHA-256 (see
[fuzzing.md](fuzzing.md)).

A few minutes, to check a change:

```sh
HELD=<private directory>
cargo fuzz build -O --fuzz-dir crates/memory_region_heap/fuzz
systemd-run --user --scope -q -p MemoryMax=12G python3 helpers/heavy_slot.py talc-smoke -- \
    cargo fuzz run -O --fuzz-dir crates/memory_region_heap/fuzz memory_region_heap_sequence \
    "$HELD/corpus" crates/memory_region_heap/fuzz/seeds/memory_region_heap_sequence -- \
    -max_total_time=240 -rss_limit_mb=4096 -artifact_prefix="$HELD/artifacts/"
```

A long run uses the same command with `-max_total_time=14400 -max_len=4096`, at a time when the
machine's heavy slots are otherwise idle.

The seeds replay as an ordinary test, and under Miri, which is the check that would catch #57's
defect coming back (it needs a nightly that ships Miri, and reads the seed directory):

```sh
cargo test --manifest-path crates/memory_region_heap/fuzz/Cargo.toml --test seeds
MIRIFLAGS="-Zmiri-permissive-provenance -Zmiri-disable-isolation" \
    cargo +<nightly with miri> miri test --manifest-path crates/memory_region_heap/fuzz/Cargo.toml --test seeds
```

`python3 crates/memory_region_heap/fuzz/seeds/make-seeds.py` rewrites the seeds. Each names one
shape a program's heap meets, such as a budget that runs out, a second initialize, or a block
asked to grow past the arena's end.

## BUGS

- The target's own BUGS, the copy of the policy among them, are in its README.
- Neither script/fuzz nor the CI fuzz job runs this target yet. script/lint type-checks it, and
  script/fmt formats it.
