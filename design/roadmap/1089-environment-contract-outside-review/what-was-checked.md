# What was checked, and how

An appendix to [the milestone](../1089-environment-contract-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main`, on the pinned
nightly toolchain:

- `cargo test -p environment_contract`: 9 unit tests and 2 documentation tests pass.
  `cargo clippy --all-targets -D warnings` and `cargo doc` with warnings denied are clean. The crate
  builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`.
- `cargo package --list`: the package would ship `Cargo.toml`, `Cargo.lock` and `src/lib.rs` only,
  with Cargo's warning that the manifest has no description, documentation, homepage or repository.
- `script/crate-standalone environment_contract`: 11 failures (names, three manifest rows, three
  files, three citation classes, proofs); the build row passes.
- The template's naming helpers over `src/lib.rs`: `spelled_out` refuses 3 (`cap`, `ConfigPage`,
  `va`), `register_names` refuses 3 (`OFF_TZ_LEN`, `OFF_LANG_LEN`, `OFF_TERM_LEN`), `abbreviations`
  reports 19 components that are not words, `unit_symbols`, `british_spelling` and
  `question_shape` refuse none of 86.
- A throwaway integration test, written in the review clone and deleted afterwards, that hands
  `ConfigPage` a built page with one length byte changed: `TERM` length 255 panics at
  `src/lib.rs` line 309 ("range end index 330 out of range for slice of length 99"). `TZ` length 41
  returns `Some("UTC")` followed by 38 NUL bytes. `TZ` length 90 returns a 90-byte string that
  contains the `LANG` and `TERM` fields. Item 1 and item 2 follow from this.
- Every member of `KNOWN_TIME_ZONES` exists in this host's `/usr/share/zoneinfo`; every member of
  `KNOWN_TERMINAL_TYPES` has a terminfo entry here. The longest canonical zone names in this host's
  tzdata are 30 bytes, and the longest backward link (`America/Argentina/ComodRivadavia`) is 32.
- The callers: `kernel/src/user/device_starters.rs` and `system_tests/src/user/std_service.rs`
  assemble a page; `components/src/printenv.rs`, `crates/swish/src/endowment.rs` and the standard
  library overlay's `sys/env/nife.rs` read one. `cargo xtask std-src` copies `src/lib.rs` verbatim
  into the overlay as `envproto.rs` (`xtask/src/farm/copied.rs`). The overlay's `seed()` calls
  `tz()`, `lang()` and `term()` before `main` runs in every `std` program that holds the page.
- Recorded, not rerun: `.cargo/mutants-baseline.txt` has the crate at 46 caught, 0 missed,
  0 timed out, 7 unviable. The census window had closed at review time.

Tried and not completed: a Kani 0.68.0 totality harness over a symbolic 99-byte page through
`ConfigPage::new` (item 5). With an unwind bound of 2 the run ends in 5 seconds at the unwinding
assertion for the eight-byte magic compare, with 198 checks undetermined. With a bound of 120,
enough for the UTF-8 validation loop over the longest field, the run was still solving when a
seven-minute cap stopped it, at 30 MB of memory. So the review has no verdict from Kani on item 1.
The integration test above is the evidence, and item 5 says how to keep the harness small.

Not run: the stranger test, `cargo semver-checks` (no release to compare against), the template's
workflows (they run in the repository), `script/crate-move --dry-run`.
