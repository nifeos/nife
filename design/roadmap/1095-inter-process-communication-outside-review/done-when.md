# Done when

An appendix to [the milestone](../1095-inter-process-communication-outside-review.md).

- A review package for the crate is in `notes/crate-reviews/` with a row in the index. The maintainer
  has decided D1 and D2, and the architect has ruled D3 and every name item 5 and item 12 list.
- `intrusive_queue` has its own repository. This crate's manifest pins it by a full revision
  on that repository's `main`.
- `standalone.py --manifest-dir .` passes in the new repository with no `--allow-unpublished`.
  Every name gate the template runs passes at ceiling zero. `typos` with the scaffold's
  configuration finds nothing.
- The manifest carries `rust-version = "1.85"`, a description, the repository and the license in
  full. It has no `lints.workspace`.
- `docs/design.md` and a README from the scaffold exist. The README states the measured Kani time
  and memory and one harness command. `docs/stranger-test.md` records one run with no open
  finding.
- Fifteen harnesses verify. Fifteen falsification patches replay red (the fifteenth from item 8,
  or the paragraph that explains its absence is rewritten to account for the overflow check).
- `cargo test`, `cargo clippy --all-targets`, `cargo doc` with warnings denied, and the three
  bare-metal builds are green in the new repository's `check`. One full `mutation` run is green
  with its survivors named.
- The kernel builds against the pinned revision with the renamed items. `script/verify` has
  no row for a crate that is no longer in the tree.
