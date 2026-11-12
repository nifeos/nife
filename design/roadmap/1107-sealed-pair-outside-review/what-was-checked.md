# What was checked, and how

An appendix to [the milestone](../1107-sealed-pair-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at `137639a3a` (the `main` head at the
time of the review). Every run used the tree's pinned nightly and a 12 GiB cap:

- `cargo test -p sealed_pair`: 11 unit tests and 2 documentation tests pass. `cargo clippy -p
  sealed_pair --all-targets -- -D warnings` and `cargo doc -p sealed_pair --no-deps` with
  `RUSTDOCFLAGS=-D warnings` are clean.
- `script/crate-review-packet --no-rustdoc sealed_pair`: 645 lines in one file, 20 public items, no
  `unsafe` site, no Kani harness, no falsification patch and no identity value. It has two path
  dependencies into this tree (`nifefs`, `measured_boot`). It has two dependents (`xtask` as a
  dependency, `uefi_loader` as a build dependency). No tag.
- `script/names sealed_pair`: one `Name:` block, provisional, with the two refusals above.
- `helpers/name_checks.py crates/sealed_pair`: no unratified name fails a current rule.
- The template's helpers over `src/lib.rs`. `spelled_out.py` and `unit_symbols.py` judge 140 names
  and refuse none. `british_spelling.py` refuses one (item 9). `function_length.py` finds the
  longest function at 71 code lines (`Seal::explain`), under the 100 limit. `standalone.py` lists
  ten findings (item 4).
- The kernel's side of the question, by reading. `kernel/build.rs` (`generate_trust_root`) still
  compiles each digest's 32 raw bytes into `TRUST_ROOT`, so the byte search in `contains` is sound
  against a current kernel. `kernel/src/trust.rs` (`require`, `require_from_loader`,
  `require_program_measurements`) halts on a trust-root mismatch. It also halts on the two
  command-line words the loader hands over, when either is missing or disagrees.
- The progenitor's side, by reading. `crates/system_initializer/src/boot_steps.rs` reads the table
  as text (bytes that are not UTF-8 become the empty table). `measurement.rs` judges each program
  through `measured_boot::verdict`, which reaches `expected_in_manifest`. That function skips a line
  it cannot parse and refuses only the program that line named.
- The mutation census series (`notes/project-metrics/mutation-census.csv`, latest row): 69 mutants
  tested, 63 caught, 0 missed, 0 timed out, 6 unviable, 0 real holes. Not re-run; the window rule
  forbids a census outside 08:00 to 13:00 UTC.
- `nifeos/basalt` at its `main` head names neither the crate nor `card-check`.

Inferred from reading, not run: the behavior of `card_check` on a card whose boot environment
carries stale digest words (item 2). Also inferred: the time `contains` takes over a release kernel
(item 13).

Not run: `script/crate-standalone` (it builds a copy, and the packet already names what it would
flag). Also not run: Kani (there is no harness), a fuzz target (there is none), `cargo
semver-checks` (no baseline) and a stranger test (there is no README to test).
