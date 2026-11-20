# Done when

An appendix to [the milestone](../1110-subtree-scope-outside-review.md).

- `script/crate-move --dry-run --into <dir> subtree_scope` exits 0 against a template release that
  runs `passive_voice.py` and `spelled_out.py`, with `filesystem_contract` written as a `git` and
  `rev` dependency and the staged `deny.toml` accepting it.
- `helpers/lint_externalized_crates.py` reports `subtree_scope` clean and its waiver entry is gone.
- `Refusal::IsADirectory` exists with errno 21, `check_landing(NodeKind::Directory, false)` returns it, and
  `redoxfs_server` has no `EISDIR` special case before `check_landing`.
- `Bindings::unbind` returns `Result<Option<u64>, Refusal>`, and the mutation census for the crate
  reports no survivor.
- `Bindings::index` cannot fold a badge wider than `usize` onto a live slot, with a test that spells
  such a badge where the target allows it and a `BUGS` line where it does not.
- The eight names of item 7 appear in the tree as built (provisional; the maintainer ruled them two-way), and the review package lists each with its status.
- `cargo test --doc` runs at least two doctests, and the README's `## EXAMPLES` compiles as one of
  them.
- A sixth harness states that `walk` asks for every component before the last, in order, and its
  falsification patch replays red.
- The README carries the scaffold's sections in order, the Kani install line on one line, the
  measured proof time and peak, and a stranger run is on record with no open finding against it.
