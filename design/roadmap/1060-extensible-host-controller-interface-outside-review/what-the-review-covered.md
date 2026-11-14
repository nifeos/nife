# What the review of extensible_host_controller_interface covered

An appendix to [the milestone](../1060-extensible-host-controller-interface-outside-review.md).

The reviewer ran these firsthand at `bea0257`, in a fresh clone on an x86_64 Linux host:

| check | result |
|---|---|
| `cargo build`, `cargo test`, `cargo clippy --all-targets`, `cargo doc` with warnings denied | clean on stable 1.99.1 and on 1.88, the `rust-version`; 30 unit tests and 2 doctests |
| `cargo build --lib` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`, `x86_64-unknown-none` | all three build; `src/` holds no `unsafe` |
| `cargo kani` (0.68.0) | 7 of 7 harnesses verify with no warning |
| the falsification patches | each fails its own harness; four name the promised assertion in the summary, three fail at an `assert_eq` or `assert_ne` that Kani reports as a placeholder |
| `cargo mutants` (27.1.0) over `src/` without `src/verification.rs` | 367 mutants: 277 caught, 49 missed, 41 unviable, no timeout |
| the three `cargo-fuzz` targets | 30 seconds each on nightly, no finding |
| `cargo deny check`, `cargo package --list` | pass, with two warnings for unmatched license allowances; the package ships `src/`, the patches and the repository's `.github/` |
| every public name against [design/naming.md](../../naming.md) | no finding against a ratified name |
| each decoded field against the xHCI 1.2 positions the reviewer recalls | no field misplaced |

Rechecked for this block on `main`: every file and symbol each item cites, and the template pin, 33
commits behind the template's `main`. The weekly run on `bea0257` fails in `mutation` and in
`stranger cadence` and passes its other jobs. The builds, tests, proofs, mutation counts and fuzz
runs come from the review; this block did not rerun them.

Inferred, not verified: the specification bounds of P1-5 and P1-7 (BUGS).

Not run: anything on hardware, `cargo semver-checks` (no release exists), Miri (green weekly), a
long fuzz campaign, a release dry run and the template's name-check helpers.

Effort sizes: XS is under an hour for a lane, S half a day, M a day or two.
