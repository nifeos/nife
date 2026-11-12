# What was checked, and how

An appendix to [the milestone](../1110-subtree-scope-outside-review.md).

Verified firsthand, on the clone, every heavy run under a 12 GiB memory cap:

- `cargo test -p subtree_scope`: 8 unit tests pass, 0 doctests exist. `cargo clippy --all-targets
  -D warnings` and `cargo doc --no-deps` are clean. `cargo build --target x86_64-unknown-none`
  builds, so `no_std` holds on one bare-metal target.
- `script/crate-move --dry-run --no-build --template-rev v7 subtree_scope`: 7 findings. The
  regenerated falsification patches apply with no offset and no fuzz, so the open `crate_move.py`
  fix (#3521, diff output no longer stripped) did not come into play here.
- `script/crate-review-packet subtree_scope`: 17 public items, one foreign type in a public
  signature (`filesystem_contract::dir::Rights` in `walk`), 0 `unsafe` sites, 5 harnesses each
  paired with a falsification patch, one dependent (`redoxfs_server`).
- `helpers/lint_externalized_crates.py`: 1 consumer-name hit, under its ceiling of 1.
- The template's head helpers over the staged tree: `spelled_out.py` refuses 2 names,
  `passive_voice.py` counts 20 passives in 3,366 words against a ceiling of 6, `unit_symbols.py`,
  `register_names.py`, `british_spelling.py`, `function_length.py` and `history_phrases.py` pass.
  The pinned template release (v7) does not run the first two; the next pin bump will.
- Download figures for the prior-art crates, from the crates.io API.
- Every caller in the tree (`redoxfs_server/src/{server,dispatch,names,memo}.rs`), which I read to
  see how it consumes the crate's decisions.

I read these records and did not run them.

- The mutation census (`notes/project-metrics/mutation-census.csv`, newest row): 62 mutants tested,
  57 caught, 1 missed, 0 timed out, 4 unviable, 0 real holes. The one survivor is the `unbind`
  guard `i != 0`, which `mutation-triage.csv` classes as equivalent.
- The Kani reach record (`notes/kani-reach-2026-10-04.csv`, rows 176 to 180).
- The falsification replay times (`notes/project-metrics/falsification-times.tsv`, rows 339 to
  343).
- The Kani peak (`helpers/kani-peaks.tsv`): 5,943 MiB at one job.

Not run: `cargo kani` and the falsification replays (the recorded times are 242 s and 104 s for
the two walk harnesses alone), a mutation census, the stranger test, `cargo semver-checks`, Miri.
