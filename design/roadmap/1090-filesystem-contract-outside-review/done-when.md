# Done when

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md).

- `helpers/lint_externalized_crates.py` reports the crate clean before the move, and
  `standalone.py --manifest-dir .` passes in the new repository with no `publish = false`, no
  provenance block, no milestone or section citation and no architect's name.
- `check / clippy`, `check / coverage` and `check / standalone` are green on the moving commit.
  `proofs / falsify` replays all three patches red through `git apply -p1`.
- Decisions A, B and C have rulings; the rename lane has landed with `#[doc(alias)]` on every
  kept foreign spelling and one `register_names` exemption with its reason.
- `reply_err` and every packer assert their field's range, each with a `#[should_panic]` test.
  `reply_errno` is total with the three edge tests.
- The harness count rises from three to at least ten, each new one with a falsification patch
  that `proofs / falsify` replays red, and the README lists each property in one line.
- `.github/mutants-exempts` carries the classified equivalents with reasons. One full `mutation.yml`
  run on `main` is green before the tag.
- The README follows the scaffold's shape with a badge row, the `rustup target add` lines, the
  Kani cost, one `--harness` example, the falsification steps, and the MSRV check commands. The
  gates `passive_voice`, `prose_caps` and `prose_acronyms` pass it.
- `src/rm.rs` is gone from the crate or has a test. The contract states `CLIENT_WINDOWS` as a fact or the constant has moved.
- `cargo test`, `cargo kani` and `cargo +1.88 test` are green at the commit the tag names.
