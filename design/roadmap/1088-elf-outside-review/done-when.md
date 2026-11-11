# Done when

An appendix to [the milestone](../1088-elf-outside-review.md).

- `script/crate-standalone elf` reports no path, section, milestone or name citation in shipped
  text, no file read from outside the crate, and a manifest with no inherited key (items 1 to 3).
- The crate builds, tests and verifies in its own repository on the template's `check` and `proofs`
  workflows, with the three bare-metal targets passed as `targets`. `fuzz.yml` opts in with the
  moved target and seeds (item 2).
- No public identifier names the project, and the machine surface is what D1 ruled (item 4).
- `page_range` and the overlap check take the page size as a value that cannot be zero or a
  non-power-of-two. A harness quantifies over every power of two (items 5 and 6).
- No `as usize` narrows a file field. `usize::try_from` does, and the four sites are tests of
  `SegmentOutOfBounds` or `BadProgramHeaders` on a forged header with a bit above 32 set (item 7).
- `Error` and `NoteError` implement `Display` and `core::error::Error`. `Elf` is `Debug`, `Clone`
  and `Copy`. The UEFI loader's wording table and the kernel's `LoadError` display use them (item
  8).
- The names D3 ruled are in place across the crate and its dependents with no alias for the old
  spelling, and `script/names` passes (item 9).
- The README has the three sections named in item 10. An `examples/` program runs under CI.
  `docs/stranger-test.md` records one run with no open finding (item 10).
- Eight harnesses become nine with the program-header index proved. Each new one with a
  falsification patch that `proofs / falsify` replays red (item 11).
- `cargo test`, `cargo kani`, `cargo clippy --all-targets` and `cargo doc` with warnings denied are
  green at the commit the baseline tag names. So is the weekly `mutation` verdict, with the four
  timeouts triaged. Placing the tag is the architect's go at that time, and nothing publishes.
