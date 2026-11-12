# Done when

An appendix to [the milestone](../1097-intrusive-queue-outside-review.md).

- `Cargo.toml` names the crate's description, repository, `rust-version = "1.85"` and license
  files, carries the template's `[lints.rust]` block, and no comment in the repository points at a
  nife path, a decision section, a milestone or a person.
- The README follows the scaffold's section order, carries `EXAMPLES` and `BUGS`, the three rules,
  the provenance clause of item 5, the Kani figures and the falsification sequence, and a stranger
  run is on record with no open finding against it.
- `unsafe impl<T: Node + Send> Send for Queue<T>` is the impl, with its reason beside it.
- `Unqueued::new` states the provenance clause; every example and test helper mints from an owning
  pointer; a Miri test of the table-walk pattern is in `tests/` and the weekly `miri` job runs it.
- The architect rules the seven names of decision 2, and the blocks are gone from the crate.
- The compile-fail doctest names `E0382`.
- The harness asserts a popped node's link is clear, and a second falsification patch replays red
  under `proofs / falsify`.
- `cargo test`, `cargo kani`, `cargo clippy --all-targets`, the three bare-metal builds and the
  template's `standalone` check are green at the commit the release tags.
