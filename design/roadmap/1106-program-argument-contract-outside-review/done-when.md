# Done when

An appendix to [the milestone](../1106-program-argument-contract-outside-review.md).

- `Cargo.toml` carries literal `license` and `[lints]` tables, `rust-version`, `description`,
  `repository` and `readme`. It has no comment that names a file outside the repository.
- The scaffold's files are present. The README follows the scaffold's section order with `EXAMPLES`
  and `BUGS`. A stranger run is on record with no open finding against the README.
- `standalone.py --manifest-dir .` reports that the crate stands alone. `spelled_out.py` reports 0
  names over its ceiling.
- `src/lib.rs` carries no `Name:` block. It has no path, section or milestone citation, and no
  person's name.
- The three ratified names are in place. `grant_plan` and the `std` overlay follow them. Nife's farm
  generation is green at the pinned commit.
- Three Kani harnesses verify, each with a falsification patch that `proofs / falsify` replays red.
  The README's "What is proven" names them with their run time.
- A host test reads a page through `from_virtual_address`. `cargo mutants` reports no missed mutant
  outside `.github/mutants-exempts`. The per-file coverage is at or above the floor.
- The README's `BUGS` states the two-field corruption case, the 32-bit address limit and the
  one-file constraint.
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo doc` with the template's
  rustdoc flags, and the three bare-metal builds are green at the commit the move check tags.
