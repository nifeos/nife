# P1: before the tag

An appendix to [the milestone](../1100-memory-region-heap-outside-review.md).

## P1: before the first tag

5. `initialize` is safe, takes no state into account, and a caller can call it again after
   allocations. DONE (a later call changes nothing, and `is_initialized` answers whether one would; the signature stays `()` because a `bool` or `Result` return breaks the question-shape naming rule or leaves every caller ignoring a `Result`; `a_second_initialize_changes_nothing`).
   Finding: a second call resets `committed` to zero and sets a new `base`, but leaves talc's
   `heap_end` as it was. The next `grow` maps pages at the new base and calls `talc::extend(end, run
   + bytes)`, which registers one free gap from the old heap's top to the new run's end, across the
   unmapped hole between them. A later allocation can be handed memory in that hole and fault. With
   the same base, the second call makes every later `MAP` land on an already-mapped address, which
   the kernel refuses with `BadPointer` after spending a page of the budget (the kernel's own
   comment; unverified by a run). No caller in the tree calls it twice. The API lets a stranger.
   Evidence: `src/lib.rs` lines 100 to 107 and 188 to 193. `vendor/talc/src/base/mod.rs`, `extend`.
   `kernel/src/syscall/memory_region.rs` lines 71 to 76.
   Change: make the second call inert and visible: return `bool` or `Result<(), AlreadyWired>` and
   refuse when `heap_end.is_some()` or `committed > 0`, and say "once" in the doc comment. The
   alternative, an `unsafe fn` with the contract in `# Safety`, moves the burden to every caller for
   nothing: the check costs one compare on a path taken once.
   Door: two-way until the tag (a signature change on a public method is one-way after it). Effort:
   XS.

6. `grow` reports success for a request of zero bytes after mapping nothing, so `alloc` can spin. DONE (the last line became `got > 0 && got >= need_pages`; `a_grow_of_nothing_at_the_maximum_is_a_refusal`, the crate's first host test).
   Finding: when `shortfall` returns 0 and the heap has hit the cap, `grow` computes `need_pages =
   0`, `want_pages = 0`, maps nothing and returns `got >= need_pages`, which is `0 >= 0`. `alloc`'s
   loop then calls `try_allocate` again with the same heap and the same answer, forever. Today the
   case is unreachable, because `shortfall` restates talc's `required_chunk_size` and talc's bin
   search finds any top gap at least that large (read from `try_allocate` and `size_to_bin_ceil`;
   unverified by a proof). The doc comment at lines 211 to 212 promises the opposite failure mode
   ("a short grow is followed by another, which doubles, until the cap refuses"). At the cap, the
   code does not refuse, it succeeds vacuously.
   Evidence: `src/lib.rs` lines 147 to 198 and 246 to 254.
   Change: `grow` returns `false` when `need == 0`, or the last line becomes `got > 0 && got >=
   need_pages`. A host test (item 8) that drives `alloc` at a reached cap with a fake page source
   and asserts a null return, not a hang.
   Door: two-way. Effort: XS.

7. The crate reads every nonzero kernel result as "budget spent", and some of them cost a page each
   time.
   Finding: `grow` breaks on any nonzero return with the comment "budget spent (OutOfMemory)". The
   kernel also returns `BadPointer` for a misaligned, non-user or already-mapped address and
   `NotPermitted` for a slot without `WRITE`, and an already-mapped address spends a page of the
   region before the refusal (the kernel's comment; unverified by a run). A program whose heap band
   collides with another mapping therefore loses one page per failed allocation, with no way to see
   why, and a program with a misaligned `base` fails every allocation with no diagnosis.
   Evidence: `src/lib.rs` lines 166 to 178. `kernel/src/syscall/memory_region.rs` lines 71 to 110.
   `crates/abi/src/lib.rs` line 327.
   Change: keep the kernel's last error in `Inner` and stop growing for good on anything but
   `OutOfMemory` (one page lost, not one per allocation). Expose it through one accessor the README
   names, so a stranger's first failed boot has a number to read. `initialize` checks
   `base.is_multiple_of(PAGE_BYTES)` and refuses otherwise (item 5's return type carries it).
   Door: two-way until the tag. Effort: S.

8. The growth policy has no host test, no proof, and exists three times.
   Finding: `[lib] test = false`. The only test is the QEMU exerciser, which reports one integer
   (committed bytes) and asserts it is positive, page-aligned and under the cap. There is no Kani
   harness. The std overlay restates the policy line for line
   (`patches/std-nife/overlay/std/src/sys/alloc/nife/heap.rs`), and the host tool
   `redoxfs_server/src/bin/second_mount.rs` restates `grow` and sizes its request as `size + align +
   2 * CHUNK_UNIT` "as `MemoryRegionHeap::alloc` sizes it", which is no longer how `shortfall` sizes
   it: the third copy has drifted. The mutation census of milestone 0955 (mutation testing reaches the kernel and its programs under QEMU, a diff and a slice at a time) groups this crate with two
   others at 346 mutants. No per-crate outcome exists on record (unverified).
   Evidence: `Cargo.toml` lines 11 to 14. `system_tests/src/user/heap_tests.rs`.
   `patches/std-nife/overlay/std/src/sys/alloc/nife/heap.rs` lines 67 to 123.
   `redoxfs_server/src/bin/second_mount.rs` lines 63 to 66 and 154 to 156.
   `design/roadmap/0955-the-kernel-and-its-programs-are-mutated-under-qemu.md` line 37.
   Change: a page-source seam, as `dlmalloc`'s `Allocator`, `rlsf`'s `FlexSource` and talc's own
   `Source` have one. It is `trait PageSource { fn map_page(&mut self, virtual_address: u64) ->
   Result<(), Error>; }` and `MemoryRegionHeap<S: PageSource = MemoryRegionPages>`, with the syscall
   in the default. Then add host tests (doubling from 8 pages, the cap, exactness at a power-of-two
   cap, `realloc` in place and by copy, zero-size layouts, item 5's second call, item 6's reached
   cap, item 7's refused address). Add Kani harnesses on the arithmetic that talc cannot be in
   (`committed` never exceeds `maximum` and stays page-aligned for any sequence of grows and any
   failing page; `grow` of zero is `false`; `shortfall` does not overflow for any valid `Layout`).
   Each harness gets a falsification patch. `cargo xtask std-src` generates the policy module into
   std as it already generates talc and `allocation_cache`. `second_mount` consumes the crate with a
   host page source instead of a copy. The template's `tests` and `coverage` jobs then measure
   something.
   Door: two-way until the tag. The generic parameter is public API after it. Effort: M.

9. The documentation does not survive a reader who has not read nife.
   Finding: the crate says "untyped" (lines 1, 42, 94 and the module docs) and "memory region" (the
   crate name, the slot's name, the `abi` module) for one thing, and defines neither. The only
   example carries `ignore` and never compiles. Nothing states the minimum useful cap. talc's first
   heap needs `min_first_heap_size::<DefaultBinning>()`, 1,575 bytes on a 64-bit target by hand from
   `BIN_COUNT = 191` (unverified by a test), plus the first chunk. `commit_all` under that cap maps
   `rest` pages, sees `claim` return `None`, and strands them with `committed` advanced.
   `initialize` rounds `maximum_bytes` down to a page silently, so a cap of 4,095 is a cap of zero.
   The doc of `committed_bytes` cites "the demo workload". `MemoryRegionHeap` is `Sync` and the doc
   never says the lock is a spinlock or what a preempted holder costs.
   Evidence: `src/lib.rs` lines 1 to 12, 24 to 29, 94 to 131, 185 to 187. `vendor/talc/src/lib.rs`
   lines 58 to 66. `vendor/talc/src/base/binning.rs` lines 115 to 119.
   Change: one vocabulary (the crate name's) and a one-sentence gloss of "untyped" with the term the
   kernel uses. Add a README with a snippet that compiles against the host page source of item 8.
   State the minimum cap and pin the figure with a test. Make `initialize` either round up or refuse
   a cap under one page and under the minimum. Name the spinlock in the type's doc.
   Door: two-way. Effort: S.
