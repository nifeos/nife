# Done when

An appendix to [the milestone](../1098-line-editor-outside-review.md).

- `standalone.py --manifest-dir crates/line_editor` reports nothing. No file in the crate
  names a nife path, a milestone, a decision section or a person.
- `proto` and `component` live in the contract crate the architect names. Every caller points at
  it. `capability_plan` is gone from the manifest. `QUIESCED` has one definition.
- The engine's public names are the ones ruled in decisions 2 and 3. `spelled_out.py` and the
  template's `check` job are green at a ceiling of 0. `HIST`'s successor carries a `Name:` block.
- The README and the crate documentation carry the bindings table and every limit with its number.
  They also carry a `BUGS` list that states ASCII-only input, the one-row redraw, the history depth, the
  Meta prefix and the mouse-report payload (or the two get a fix and a test).
- `lib.rs` carries `#![forbid(unsafe_code)]`. Two fuzz targets build and have run once with no
  finding. Two Kani harnesses pass. A test pins the handoff byte layout.
- `cargo test`, `cargo clippy --all-targets -D warnings`, `cargo doc -D warnings` and the
  bare-metal build are green at the commit the tag names. The mutation run in the template's
  window reports none missed outside `.github/mutants-exempts`.
