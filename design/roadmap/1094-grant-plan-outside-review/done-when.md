# Done when

An appendix to [the milestone](../1094-grant-plan-outside-review.md).

- The crate that leaves names no program, package, path or person of this tree:
  `helpers/lint_externalized_crates.py` drops its waiver for `grant_plan`, and the template's
  `standalone` job is green on the new repository with no `allow-unpublished` beyond the ruled
  hold.
- `Endowment` carries a `Manifest` and no `Program`. The leaf crate for the manifest types and the
  slot numbers exists, and every program that read a slot from `grant_plan` reads it from there. The
  program table, `spawnproto`, `job_windows` and the region budgets live in nife, and nife's tests
  pin every shipped wire id.
- `cargo doc` with warnings denied, `cargo clippy --all-targets`, `cargo test`, and the builds for
  `x86_64-unknown-none` and `riscv64imac-unknown-none-elf` are green on the new repository.
- `tokenize` refuses a line it could not hold in full, with a test of seventeen tokens.
- At least the eight harnesses of item 5 exist, each with a falsification patch that the template's
  `falsify` job replays red.
- Every public limit in bytes ends `_BYTES`, no public name carries a word from the spelled-out
  list, `register_names.py` and `spelled_out.py` report zero refused over `src/`, and
  `british_spelling.py` finds nothing.
- The planning functions take their three large arguments by reference, and a `size_of` test pins
  `Endowment`, `Holdings`, `NameSet` and `Cwd`.
- A test checks each refusal sentence that names a number against the constant it spells.
- The README carries `## EXAMPLES` with a runnable end-to-end example and `## BUGS`, the crate root
  carries no `#![allow(missing_docs)]`, and a stranger run is on record.
- One full mutation run over the crate that leaves is green at the commit the first tag names.
