# P2: later

An appendix to [the milestone](../1103-page-frames-outside-review.md).

## P2: can follow later

11. `#[must_use]` is absent everywhere. Dropping the `Option` from `alloc` or `alloc_contiguous`
    leaks a frame silently; `containing`, `from_addr`, `addr`, `bitmap_bytes`, `page_frames_in`,
    `stats`, `Stats::free`, `largest_free_run` and `is_used` are pure. Add the attribute to all
    eleven. Evidence: `src/lib.rs` lines 48 to 64, `130-137`, `172`, `211`, `229`, `265`, `297`. Door:
    two-way. Effort: XS.

12. There is no `free` for a run. `alloc_contiguous` hands out `count` frames and the two kernel
    callers that return them loop over `free` one frame at a time
    (`kernel/src/e1000e.rs` lines 338 to 343, `kernel/src/memory.rs` lines 1364 to 1369). A `free_contiguous(first,
    count)` asserts once that the allocator owned the whole run and had it in use, and gives `alloc_contiguous` the
    symmetric partner `buddy_system_allocator::dealloc(start, count)` has. Door: two-way now.
    Effort: S.

13. `alloc_contiguous` takes no alignment. A device that needs a run aligned beyond one frame
    cannot ask; `bitmap-allocator` and `buddy_system_allocator` both can. No caller in the tree
    needs it (the virtio ring fits one frame; the two network drivers ask for a run with no
    alignment). A `## BUGS` line in the README says so now; `alloc_contiguous_aligned(count,
    align_frames)` lands when a caller appears. Door: two-way. Effort: XS for the line.

14. `from_addr` panics on an unaligned address and offers no fallible form. `x86_64`'s
    `PhysFrame::from_start_address` returns `Result<_, AddressNotAligned>`. Add a `const fn`
    returning `Option<PageFrame>` beside the panicking one (which `const` assertions in callers
    need); name provisional. Evidence: `src/lib.rs` lines 48 to 54. Door: two-way (adding is compatible).
    Effort: XS.

15. The crate assumes a 64-bit `usize` and does not say so. `page_frames_in` (`src/lib.rs` line 136) and
    `index_of` (`306`) cast a `u64` quotient with `as usize`. On a 32-bit target a span of 2^44
    bytes or more truncates, and `index_of` could pass its range check with a wrapped index. No such
    board exists for this tree, but the crate states no target bound and the template builds it for
    three. Add `const _: () = assert!(usize::BITS >= 64, "...")` or `usize::try_from` with a
    documented refusal, and a `## BUGS` line. Door: two-way. Effort: XS.

16. The crate-level `Name:` paragraph garbles its own story: it says the kernel's capability type
    took the name `PageFrame` without saying from what, and attributes the collision to a type where
    it was the word "frame" in the compositor. The kernel's kind is `Object::PageFrame`
    (`kernel/src/capability_object.rs` line 44). The move strips the block. Fix the sentence in nife so
    the history reads right. Evidence: `src/lib.rs` lines 8 to 11. Door: two-way.
    Effort: XS.

17. The three mutation survivors need their exemptions written before the first weekly run. nife's
    census argues all three equivalent (`alloc`'s hint, `largest_free_run`'s boundary,
    `alloc_contiguous`'s early return) and classifies the crate at zero real holes. The dry run
    carries the row into `.github/mutants-exempts` as a comment only. Write the three entries with
    their arguments in the template's form, in the new repository. Evidence:
    `notes/mutation-testing/baseline-survivors-abi-to-ipc.md` lines 150 to 160.
    `notes/mutation-testing/survivor-classification.md` line 55. Door: two-way. Effort: XS.

18. `PageFrame` derives `Ord` and not `Hash`. Host tooling and tests that collect frames in a set
    (the tests use `Vec::contains`) would want it. Add `Hash` to the derive list. Evidence:
    `src/lib.rs` line 35. Door: two-way (adding a derive is compatible). Effort: XS.

19. `alloc` advances the scan hint, `free` lowers it, and `alloc_contiguous` and `mark_used` leave
    it alone. Correctness does not depend on it (the scan wraps) and `free` says so;
    `alloc_contiguous` does not. Verified by a throwaway test: after one `alloc` and one
    `alloc_contiguous(4)`, the next `alloc` scans over the run and still answers the right frame.
    Either advance the hint past the run or say in the comment why the hint stays. Evidence:
    `src/lib.rs` lines 123 to 125, `229-255`. Door: two-way. Effort: XS.

20. `notes/physical-memory.md` is stale on the nife side: it names `FrameAllocator::new` and an
    `init`, and gives a six-step order where the kernel's code has three loops (`mark_free` each
    RAM region, `mark_used` each forbidden region, `mark_used` the bitmap;
    `kernel/src/memory.rs` lines 387 to 399). Edit it to the truth when the README of item 2 takes the
    arguments over, and point it at the crate's README for them. Evidence:
    `notes/physical-memory.md` lines 48 to 66. Door: two-way. Effort: XS.
