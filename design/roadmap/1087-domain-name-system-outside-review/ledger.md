# The ledger

An appendix to [the milestone](../1087-domain-name-system-outside-review.md).

| item | state |
|---|---|
| 1 | done: every public name is ratified as built, and the ratification pull request's body is the `name_release.py --draft` table |
| 2, 3, 4 | done: a standalone manifest, the README and both license files, shipped text that cites nothing in this tree. The caller workflows and the other scaffold files come from `script/crate-move` at the move |
| 5 | partly done: `#![forbid(unsafe_code)]` is in `src/lib.rs`; the crate's own `fuzz/` directory is staged by `script/crate-move` at the move |
| 6, 7 | done, provisional: parts a to d. The `.github/accepted-words` entries come at the move |
| 8, 10 to 15 | done |
| 9 | at the move: the three lines name the stripped source's line numbers |
| 16 | done: the constant, the doc sentence and the test client, which now holds a reply of 65,535 bytes |
| decisions 1 to 3 | decided by the maintainer, provisional; the architect may revisit |
