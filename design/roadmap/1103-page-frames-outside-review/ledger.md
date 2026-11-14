# Ledger

An appendix to [the milestone](../1103-page-frames-outside-review.md).

An earlier lane built item 4 and the `mark_free` harness; this lane checked them against
the code.

| item | state | where |
|---|---|---|
| 1 manifest does not stand alone | LEFT | move work: licenses, inlined lints and `rust-version` belong with the repository |
| 2 README | LEFT | move work, and it waits on whether the crate leaves |
| 3 documentation cites this tree | PARTIAL | the `Name:` paragraph no longer garbles its story (item 16) and the docs no longer cite `notes/mmu.md`. LEFT: the README rewrite takes the remaining section, milestone and path citations |
| 4 `mark_free` rounds outward | DONE | `mark_free_of_an_unaligned_start_keeps_the_frame_it_starts_in`, `mark_free_of_an_unaligned_end_keeps_the_frame_it_ends_in`, `mark_free_of_a_region_straddling_one_boundary_frees_nothing`; Kani `mark_free_frees_only_whole_frames_inside_the_range` |
| 5 `forbid(unsafe_code)` | DONE | `#![no_std]` and `#![forbid(unsafe_code)]` at the top of `lib.rs` |
| 6 three more harnesses | DONE | Kani `mark_used_claims_exactly_the_frames_the_range_touches`, `the_used_count_is_the_number_of_set_bits`, `a_contiguous_run_was_free_and_is_now_used`, each with a falsification patch. the counter harness and the host tests cover `free(alloc())` restoring the bit, with no harness of its own |
| 7 acronyms | DONE | DMA, MMU and MMIO expand at first use |
| 8 names | LEFT | Decisions 1 and 2 are the architect's, and the rename lane follows them |
| 9 `page_frames_in` truncates | DONE | `a_span_that_ends_mid_frame_is_covered_but_not_whole`; `page_frames_covering` (provisional) is what `kernel/src/memory.rs` calls |
| 10 patches drift | PARTIAL | every patch in the crate applies with `patch -p1 -F0` at no offset. LEFT: the blank-context-line fault in the move's regeneration, which belongs to the move tooling |
| 11 `#[must_use]` | DONE | the eleven items, with a reason on `alloc` and `alloc_contiguous` |
| 12 `free` for a run | LEFT | no caller uses it yet; the two kernel loops stay until a lane takes the symmetric API |
| 13 alignment | DONE | a `# BUGS` entry on `alloc_contiguous` |
| 14 fallible `from_addr` | DONE | `an_unaligned_address_is_refused_without_a_panic`; `PageFrame::try_aligned` (provisional) |
| 15 64-bit `usize` | DONE | a `const` assertion at the crate root |
| 16 `Name:` paragraph | DONE | the paragraph says what the word collided with |
| 17 mutation exemptions | LEFT | written in the new repository |
| 18 `Hash` | DONE | `a_frame_can_key_a_set` |
| 19 scan hint | DONE | `a_contiguous_run_leaves_the_next_alloc_a_free_frame`; the reason the hint stays is in `alloc_contiguous`'s documentation |
| 20 stale note | DONE | `notes/physical-memory.md` already names `PageFrameAllocator::new` and the kernel's four steps |

Decisions 1 and 2 carry background, a recommendation, prior art and an example above; the
architect has ruled neither.
