# What was checked, and how

An appendix to [the milestone](../1082-capability-manifest-note-outside-review.md).

Verified firsthand on the clone, with every cargo run under a 12 GiB memory cap:

- `cargo test -p capability_manifest_note`: 29 unit tests, 2 integration tests (one ignored by
  design), 3 doctests pass (2 doctests are `ignore`, the `carry!` examples). `cargo clippy
  --all-targets` and `cargo doc --no-deps` with `-D warnings` are clean on the pinned nightly. The
  crate builds for `x86_64-unknown-none`.
- `cargo kani -p capability_manifest_note`: both harnesses (`a_decoded_manifest_encodes_back_to_its_bytes`,
  `decode_never_panics`) verify, in 243 and 94 seconds on this host. The run is slow for a 56-byte
  input because `zero` reaches `core::slice::memchr` through `position`, which Kani unwinds byte by
  byte.
- `script/crate-move --dry-run --into <empty> --template <local clone at v7>
  capability_manifest_note`: 8 findings, listed under items 1, 2, 5, 6 and 8. The dry run itself
  behaved; the open fix #3521 to `crate_move.py` (diff output no longer stripped) was not needed on
  this crate.
- The template's helpers at head, run over the crate by hand. `unsafe_scan.py` finds 5 lines of
  `unsafe` in shipped source (item 9). `spelled_out.py` finds 15 names over a ceiling of 0 (item 6). `unit_symbols.py` finds 0 of 454. `file_length.py`, `function_length.py` and `comment_blocks.py` are clean. `abbreviations.py` reports 46 components that are not words (report only, no ceiling; `descsz`,
  `subtree`, `unsorted` among them).
- Two probes written against the public API in a throwaway test file, run, then deleted (item 3).
- `helpers/lint_externalized_crates.py --count`: 37 hits for this crate, at its ceiling.
- The recorded mutation results: 3 survivors in the census of record, each classified equivalent
  (`Scope::word` returning 1, `<` to `<=` in `before` at a position where the bytes already differ,
  and `56 - 55` to `56 / 55`). The review ran no census.
- The crates.io API for the download figures in the prior-art box, and the FreeBSD `elf(5)` and
  `elfctl(1)` manual pages and the UAPI group's package-metadata specification for the prior art.

Inferred from reading, not run: that `tests/fuzz_seed.rs` fails in the staged repository (item 8;
the dry run's build step failed on the manifest before it reached the tests), and the claims
marked unverified in the prior-art box.

Not run: a mutation census, a fuzz campaign, Miri, a bare-metal build for the two other targets,
`cargo semver-checks`, and the template's `check.yml` on a real repository.
