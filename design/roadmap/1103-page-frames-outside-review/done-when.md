# Done when

An appendix to [the milestone](../1103-page-frames-outside-review.md).

- `standalone.py` on the staged tree reports nothing but the `publish = false` line the soak
  allows, and `script/crate-move --dry-run --no-build page_frames` exits 0.
- The README takes a stranger from a clone to `cargo test`, the three bare-metal builds,
  `cargo kani`, one harness and one falsification, carries the three design arguments, and a
  stranger run is on record with no open finding against it.
- No sentence in `src/`, `tests/` or `falsifications/` cites a section, a milestone, a nife path
  or a nife program.
- `mark_free` of a region that begins 16 bytes into frame 2 and spans three frames frees frames 3
  and 4 only. A host test pins it. The documentation of `mark_free` and `mark_used` states the
  asymmetry as the code has it.
- `lib.rs` opens with `#![no_std]` and `#![forbid(unsafe_code)]`.
- `cargo kani` verifies eight harnesses, each with a falsification patch that applies with
  `patch -p1 --dry-run -F0` at no offset, and `falsify.py` passes on the staged tree.
- `prose_acronyms.py` over `src/` and `tests/` reports nothing.
- The architect has ruled Decisions 1 and 2, one rename lane has performed them and the two-way
  renames of item 8, and `cargo build` of the kernel for all three architectures is green.
- `page_frames_covering` (or the ratified name) exists and `kernel/src/memory.rs` calls it.
- Every pure function and both allocation functions carry `#[must_use]`.
- `check` and `proofs` are green on `main` in the new repository for the crate's own reasons.
