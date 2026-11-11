# Done when

An appendix to [the milestone](../1071-entropy-contract-outside-review.md).

- `## What is proven` names the four properties, their bounds and the falsification patches, and
  the code contradicts no sentence of the README or the crate documentation.
- `requested_bytes(request(GET, 256))` is `MAXIMUM_BYTES`, the harness asserts the clamp, and the
  README's example runs unchanged over a 256-byte buffer.
- `passive_voice.py`, `readme_badges.py` and `standalone.py --allow-unpublished` from the pinned
  template report nothing on the staged tree, and `script/crate-move --dry-run` exits 0.
- The README takes a stranger from a clone to `cargo test`, the three bare-metal builds, `cargo
  kani`, one harness and one falsification, and a stranger run is on record with no open finding
  against it.
- `fill_with` returns `Result<(), Refusal>` (or the ratified name), the fill harness tells the two
  refusals apart, and `user_mode_runtime::fill` compiles against it.
- The contract documentation states the unknown-opcode answer, the reserved bits and the zeroed
  tail, and `readiness` carries the architect's ruling on a repeated draw with a harness and a
  patch.
- `Cargo.toml` names no path outside the repository, carries `no-std::no-alloc` and not
  `cryptography`, and the five pure functions carry `#[must_use]`.
- `check` and `proofs` are green on `main` in the new repository for the crate's own reasons.
