# What the review of non_volatile_memory_express covered

An appendix to [the milestone](../1058-non-volatile-memory-express-outside-review.md).

The reviewer verified these firsthand, in a fresh clone of `main` on one Linux host:

- `cargo build`, `cargo test` (29 unit tests and 7 doctests), `cargo clippy`, `cargo doc` and
  `cargo fmt --check`, all with warnings denied: clean on 1.99.1 and on 1.88, the `rust-version`. The same
  tests pass on 1.85 with `--ignore-rust-version`.
- The crate builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`.
- Kani 0.68.0 verifies all ten harnesses in about 14 seconds in total. Each of the ten
  falsification patches applies, fails exactly its own harness at the assertion its header names,
  and reverses to a clean tree.
- `cargo mutants` over the whole crate: 247 mutants, 161 caught, 76 missed, 10 unviable, no
  timeout. The 76 misses sit on exactly the 28 source lines that `.github/mutants-exempts` lists:
  six production lines whose OR-to-XOR and shift swaps act on disjoint bits, and 22 lines inside
  `#[cfg(kani)]` harness bodies that the ordinary tests never compile.
- All three `cargo-fuzz` targets ran 20 seconds each with no finding. `typos` and
  `cargo deny check` pass, and `cargo package --list` ships 55 files.
- No `unsafe` in `src/`, and no mention of this tree beyond the App secret names in `release.yml`.
- Through the GitHub API: the last weekly run green in every job, nine required checks, no tag.

Inferred, not verified: the ordering hazard of P1-6, read from the API shape and two consumers.
The page-size mismatch of P1-9 is also inferred, since nobody built a kernel with 16 KiB pages
against the crate.

Not run: `cargo semver-checks` (no baseline exists), Miri (green in the weekly job), a long fuzz
campaign, the release dry run, the name checks of the template at its head, and anything that needs
a controller. The reviewer changed nothing in the repository.

Effort sizes: XS is under an hour, S half a day, M a day or two.
