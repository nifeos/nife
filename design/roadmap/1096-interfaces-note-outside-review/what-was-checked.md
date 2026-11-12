# What was checked, and how

An appendix to [the milestone](../1096-interfaces-note-outside-review.md).

Verified firsthand, in a clone outside every worktree:

- `cargo test -p interfaces_note`: 7 tests pass, the one doctest is `ignore`. `cargo clippy
  --all-targets -- -D warnings` and `cargo doc` with warnings denied are clean. The crate builds for
  `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none` on the
  workspace's pinned nightly.
- With the license inlined, `cargo +1.85 test` passes and `cargo +1.85 build --target
  x86_64-unknown-none` builds; `cargo +1.84` refuses the manifest (edition 2024). So the
  `rust-version` is 1.85.
- Kani 0.68.0 verifies both harnesses (`decode_interfaces_never_panics`,
  `a_decoded_interface_list_encodes_back_to_its_bytes`).
- `script/crate-move --dry-run` against the template at `v7`, once with `--no-build` (6 findings)
  and once with the build (7 findings). Both regenerated falsification patches apply with no offset
  and no fuzz, so the open `crate_move.py` fix for stripped diff output did not come up.
- The template's helpers from its `main`, which `v7` does not yet run, ran over the staged tree.
  The results: `standalone.py` (8 lines), `register_names.py` (1), `passive_voice.py` (19 passives in 2,408
  words against an allowance of 4), `prose_acronyms.py` (2), `abbreviations.py` (1, report only);
  `spelled_out.py`, `unit_symbols.py`, `question_shape.py`, `service_model.py`,
  `british_spelling.py` and `contract_names.py` refuse none of 94 names.
- `helpers/lint_externalized_crates.py --count`: 9 hits for this crate, equal to its waiver.
- The callers: `components` (fifteen programs, each one `carry_interfaces!` list),
  `crates/system_initializer/src/image_request.rs` (`serves_declared_abi`), and the
  `capability_manifest_note` test that holds the two note headers equal.
- Download figures for the prior-art crates, from the crates.io API.

Read, not run: the recorded mutation census (88 mutants: 78 caught, 0 missed, 3 timeouts triaged
as equivalent, 7 unviable). A census is outside this review's hours.

Not verified: the behavior of the prior-art loaders below, which this review states from their
public documentation without rereading it; whether the template's mutation verdict counts a
cargo-mutants timeout as missed (item 15).
