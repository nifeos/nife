# Verified firsthand, inferred, and not run

An appendix to [the milestone](../1050-designware-mobile-storage-outside-review.md).

Verified by the reviewer on a host:

- `cargo build`, `cargo test` (92 tests and 5 doctests pass), `cargo clippy --all-targets`
  (clean), `cargo doc` with warnings denied (clean).
- Release builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`.
- Three Kani harnesses (`the_clock_divider_is_total`, `a_register_field_never_exceeds_its_width`,
  the MBR parse) pass, and the clock-divider falsification patch turns its harness red.
  `falsify.py --check` holds all 23 records.
- The README example compiles under `rustdoc --test`. The source holds no `unsafe`, and the prose
  files hold no contractions.
- The reviewer read every source location cited above in the clone.
- The tags, releases, rulesets, team, workflow runs and job logs of the repository, through the
  GitHub API, and the crates.io figures.

Inferred, not verified: the cause of the runner shutdown in the weekly mutation shards, and the
behavior of Linux and U-Boot on a data error, from the reviewer's memory of those drivers. The busy
ceilings of the specification and the internals of `dwmmc-host` are inferences too.

Not run: the full Kani suite and the full falsification replay (CI shows both green on `main`),
cargo-mutants, Miri, the `standalone` and prose gates of the template locally, and anything on
hardware.
