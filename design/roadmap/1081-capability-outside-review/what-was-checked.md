# What was checked, and how

An appendix to [the milestone](../1081-capability-outside-review.md).

Verified firsthand on a fresh shallow clone of `main` at the review's head commit, and on a copy of
the crate outside the workspace with a standalone manifest:

- `cargo test -p capability`: 20 unit tests and 2 documentation tests pass. `cargo clippy -p
  capability --all-targets -D warnings` and `cargo doc` with warnings denied are clean. The crate
  builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`.
- Outside the workspace, the same tests pass on stable, on 1.85 and on 1.88. Outside the workspace
  the build warns twice about `unexpected cfg condition name: kani`, because the manifest leans on
  the workspace's `[lints]` for the `check-cfg` line (item 1).
- `cargo clippy` with `clippy::pedantic` reports 30 findings, among them five `u64` to `usize`
  casts that truncate on 32-bit targets (item 4), eight `Result`-returning functions without an
  `# Errors` section, and twelve missing `#[must_use]`.
- Kani 0.68.0, the version the template's `.kani-version` names: all 14 harnesses verify in 11
  seconds of solver time. All 14 falsification patches apply with `git apply --check`. The review replayed four of the fourteen (`derive_never_widens_rights`, `the_free_mask_is_the_empty_slots`, `the_count_is_the_slots`, `a_stranger_reveals_nothing_about_its_liveness`); each turns exactly its own harness red. It did not replay the other ten.
- `script/crate-move --dry-run --no-build` with the template at its newest release: 8 findings,
  listed under items 1, 2 and 6. The template's `standalone.py` and `falsify.py --check` ran on the staged tree directly. The template's `spelled_out.py` and `unit_symbols.py` judge 186 names
  of the crate and refuse none; its `abbreviations.py` reports 11 components that are not words
  (item 9).
- The kernel's call sites, by reading: which methods of the crate the kernel calls, and which
  function performs delegation (item 3).
- Sizes, measured: `Capability<u64>` 16 bytes, `Option<Capability<u64>>` 24, `CapabilityTable<u64,
  64>` 1,552; with the kernel's 24-byte object, `CapabilityTable<Object, 64>` is 2,064 bytes, as
  `notes/capabilities/table.md` states.
- Mutation results, read from the records and not rerun: `.cargo/mutants-baseline.txt` carries
  caught 98, missed 3, timeout 3, unviable 26; `notes/project-metrics/mutation-triage.csv` names
  the three survivors and argues each equivalent (item 13).
- crates.io, through its API: whether anyone holds the names `capability`, `capability_table` and six alternatives, and the download figures in the prior-art table.

Inferred from reading, not run: the behavior on a 32-bit target (item 4; the host has no 32-bit target and the review added none). The same holds for the cause of the move tool's patch defect (item 6; read in `helpers/crate_move.py`, not fixed and retested).

Not run: a mutation census, Miri, a release dry run, `script/crate-move` with a build, the template at head. The newest release predates its spelled-out, unit, passive-voice and function-length checks, so the staged tree met fewer gates than a new repository will.
