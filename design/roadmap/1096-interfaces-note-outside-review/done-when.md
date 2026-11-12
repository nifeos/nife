# Done when

An appendix to [the milestone](../1096-interfaces-note-outside-review.md).

- `script/crate-move --dry-run --into <dir> interfaces_note` exits 0 at the newest template
  release, with the build step run, and the staged `Cargo.toml` carries `rust-version = "1.85"`,
  the inline license and no inherited key.
- The README is in the shared section order and `readme_sections.py`, `readme_acronyms.py`,
  `passive_voice.py` and `prose_acronyms.py` report nothing against it or `src/lib.rs`.
- The README states the link-script requirement, the little-endian rule, where the id registry
  lives and that an absent entry passes.
- The architect rules decision 1, and the lint and the move check accept the ruled form; `lint_externalized_crates.py`
  drops this crate's waiver.
- The architect rules decisions 2 and 3, the renames have landed, and `script/names interfaces_note` reads
  `ratified` for every name.
- Four doctests run under `cargo test`, none `ignore`.
- `cargo kani` verifies three harnesses, each with a replayable falsification patch that
  `proofs / falsify` turns red.
- The dry run stages a fuzz target for the decoder and opts it in in the moved repository.
