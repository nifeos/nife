# P1: before the tag

An appendix to [the milestone](../1077-allocation-cache-outside-review.md).

## P1: before the first tag

4. The public arithmetic takes bare `usize` and has three edges no `Layout` reaches.
   Finding: `class(size, align)` rounds `size` with `next_multiple_of(GRID)` before it compares to
   `LARGEST` (`src/lib.rs` lines 66 to 74). In release, `class(usize::MAX, 8)` wraps to zero, passes the
   bound and returns `Some(18446744073709551615)` (verified); in debug it panics. `class_bytes(c)`
   computes `(c + 1) * GRID` (`src/lib.rs` line 79), which overflows the same way on a large index (inferred). `count`, `take` and `put` index `heads` and `counts` with the bare index, so a class
   at or past `CLASSES` panics on bounds (verified for `count`). None of this is reachable through
   a valid `Layout`, whose size is at most `isize::MAX` (verified: `class(isize::MAX as usize, 8)`
   is `None`), and the `std` consumer only passes layouts. The compiler catches a stranger who calls the
   `const fn` with a literal; it does not catch one who calls it with a runtime `usize`.
   Change: compare before rounding (`if size > LARGEST { return None }`), which is also one fewer
   operation on the allocator's fast path, or use `checked_next_multiple_of`. Add the edge tests
   the standalone copy ran. The index problem is a type problem, and the fix is the `Class`
   newtype that Decision 3 decides, provisionally: `class` returns `Option<Class>` and every method
   that takes a class takes `Class`.
   Door: two-way before the first tag. Effort: S.

5. The consumer relies on "zeroed is empty", and the crate neither states nor tests it.
   Finding: the thread block in the `std` overlay holds a `Cache` in zeroed memory. Its field comment says "Zeroed is empty, so a fresh block needs no setup"
   (`patches/std-nife/overlay/std/src/sys/thread_local/key/nife.rs` lines 50 to 51). `Cache::new` writes null heads and zero counts (`src/lib.rs` lines 139 to 144), so the claim is true today. Nothing in the crate says a later field may not break it. A test that reads a `core::mem::zeroed::<Cache>()`
   as empty passes (verified on the standalone copy).
   Change: one sentence on `Cache` ("an all-zero `Cache` is empty, so it can live in memory a
   runtime zeroes") and that test.
   Door: two-way. Effort: S.

6. `put`'s safety contract stops short of what `spill` and `drain` later assume.
   Finding: `put` asks for `class_bytes(class)` writable grid-aligned bytes used by nothing else
   (`src/lib.rs` lines 169 to 172). That is enough for the cache's own soundness. But every block the cache
   hands to `spill`'s or `drain`'s callback goes to a heap under `class_layout(class)` in both the
   host test and the consumer (`src/lib.rs` lines 411 and 423;
   `patches/std-nife/overlay/std/src/sys/alloc/nife/entry.rs` line 43). A block that merely satisfies
   `put` (a slice of a stack array, as the doc example uses) must never reach a `spill` into a heap.
   The crate does not say where that obligation lives.
   Change: `Cache`'s doc states the owner's rule. A block goes into a `Cache` the owner means to `spill` or `drain` into a heap only if that heap will accept it under `class_layout(class)`. `put`'s `# Safety` points to it. `spill` and `drain` say they hand back only what `put` and
   `refill` took.
   Door: two-way. Effort: S.

7. Five constant names carry no unit or name no thing, against the naming rules this tree applies
   at a release review.
   Finding: `GRID` is 16 bytes and `LARGEST` is 256 bytes (`src/lib.rs` lines 42 and 45); the units rule says a length or a limit carries its unit, and a limit opens `MAXIMUM_`. `DEPTH` and `BATCH` count blocks (`src/lib.rs` lines 51 and 56). The rule says a count of things names the things.
   `MOST_CACHED_BYTES` (`src/lib.rs` line 59) opens with `MOST_` where the rule writes `MAXIMUM_`. The
   template's `unit_symbols` gate passes all five because none carries a symbol; the rule is one
   that a reader applies. The methods `put` and `drain` do not take std's spelling and meaning
   where std has one: `put` is glibc's word for what std and `heapless` call `push`, with the same `Result<(), T>` shape on a full container. std's `drain` returns an iterator where this one takes a closure. `take` matches std's meaning.
   Change: as Decision 2 decides, provisionally: `GRID_BYTES`, `MAXIMUM_CLASS_BYTES`,
   `BLOCKS_PER_CLASS`, `BATCH_BLOCKS`, `MAXIMUM_CACHED_BYTES`, `push` for `put`, `pop` for `take`,
   and `give_back_all` for `drain` unless it comes to return an iterator. The lane renames in one
   pass before the move, and the consumer's three `use` sites follow.
   Door: two-way before the first tag. Effort: S.

8. The crate is `unsafe` at its core and has no proof.
   Finding: 22 `unsafe` sites, 7 of them in production code, each with a `SAFETY` comment a reader
   can check (`src/lib.rs` lines 87, 105, 161, 181 and 217; the two `unsafe fn` bodies). Kani harnesses:
   none; falsification patches: none. The scaffold's README template claims every property is a
   harness, which this crate cannot say. The pure functions are a cheap proof: for every `size` up to `LARGEST` and `align` up to `GRID`, `class_bytes(class(size, align))` is at least `size` and under `size + GRID`, and the index is under `CLASSES`. Also, `heap_layout(l)` keeps `l`'s alignment,
   is never empty and is never smaller than `l`. A bounded model of `put` and `take` over a fixed
   arena proves the count never passes `DEPTH`, `cached_bytes` is the sum, and `take` returns the
   last `put` (the stack discipline the `Free` link encodes).
   Change: those three harnesses with their falsification patches, in a `verification` module as the sibling crates lay it out. The README's proof section then separates what the harnesses
   prove from what the heap walk tests.
   Door: two-way. Effort: M.

9. The code relies on the `Free` link's alignment and does not assert it.
   Finding: `const _: () = assert!(size_of::<Free>() <= GRID)` (`src/lib.rs` line 119) checks the size.
   `put` casts the block to `*mut Free` under `#[allow(clippy::cast_ptr_alignment)]` with the
   comment that `GRID` alignment "is at least a `Free`'s" (`src/lib.rs` lines 177 to 179). True on every
   target the crate builds for, and a one-line `align_of::<Free>() <= GRID` beside the size
   assertion makes it a mechanism rather than a comment.
   Change: that assertion.
   Door: two-way. Effort: S.
