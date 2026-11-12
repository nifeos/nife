# What was checked, and how

An appendix to [the milestone](../1103-page-frames-outside-review.md).

Verified firsthand on a fresh shallow clone of `nifeos/nife` at `75bb255a`, with the template
`nifeos/crate-workflows` at `b84ada0` for the helpers and at its `v7` release for the dry run:

- `cargo test -p page_frames`: 17 integration tests and 2 documentation tests, all green on the
  pinned nightly. Eight throwaway edge-case tests written for this review (never committed) also
  passed. They are the evidence cited in items 4, 9 and 15. The cases are `containing(u64::MAX)`, a
  saturating `mark_used` past the end of the address space, a zero-frame allocator, a range
  straddling `base` from below, an unaligned `mark_free`, the truncation of `page_frames_in`, and
  the hint after `alloc_contiguous`.
- `cargo clippy -p page_frames --all-targets -- -D warnings`: clean. `cargo doc` with
  `RUSTDOCFLAGS=-D warnings`: clean.
- `cargo kani -p page_frames` with Kani 0.68.0, the template's pinned version: five harnesses
  verified. The run took 8 seconds wall clock with 169 MB peak resident, measured with
  `/usr/bin/time -v`.
- Every falsification patch applied with `patch -p1 --dry-run -F0` against the in-tree source. All
  five apply, each at an offset of 25 to 52 lines (item 10).
- The template's helpers run by hand over the crate. `unsafe_scan` finds no `unsafe`. `spelled_out`,
  `unit_symbols` and `british_spelling` judged 155 names and refused none. `function_length`,
  `file_length` and `comment_blocks` report everything within limits. `prose_acronyms` has three
  findings (item 7). `passive_voice` counts eight passives in 2,222 words of `lib.rs`, over the 2
  per 1,000 ceiling. The template reads only the README, CHANGELOG and `docs/`, so this does not
  gate. `abbreviations` (report only) lists `addr` five times, `bm` and `fb`.
- `script/crate-move --dry-run --no-build` into a scratch directory: eight findings for the move
  lane (items 1, 2, 3, 10), and the tooling defect of item 10.
- Every caller in the tree. `kernel/src/memory.rs` is the only user of the allocator. The rest of
  the kernel and `system_tests` use `FRAME_SIZE` and `PageFrame::from_addr` only.
- The records. `notes/verification/core-harnesses.md`. The Kani reach record in `notes/` (5
  harnesses, 50 viable mutants in reach, 38 proof-killed, 48 test-caught, 1 proof-only, 1 killed by
  neither). `notes/mutation-testing/survivor-classification.md` (3 missed, 0 real holes). And
  `baseline-survivors-abi-to-ipc.md`, which argues the three survivors equivalent.

Not run: a mutation census (outside its window). Also not run: the bare-metal builds. The template's
`ci.yml` builds them, and the crate has no dependency and no `unsafe`, so the review expects no
finding. Also not run: a stable toolchain at the proposed `rust-version` (item 1 says how).
