# Done when

An appendix to [the milestone](../1108-socket-contract-outside-review.md).

- Every `OPERATION_` constant's documentation names its argument word and its reply words. The
  layout note says who writes `LENGTH_OFFSET_BYTES` and when. A test or doc test in the crate exercises a send
  that passes its length as the argument. The README's "Replies" bullet agrees.
- The manifest's `publish` line carries a comment that stands alone. The caller passes
  `allow-unpublished` until the release. The `repository` field has a value.
- D1 has a ruling. If D2 goes as recommended, `peer_word` and `word_peer` exist with a test and a
  harness. The stack fills the second reply word on `ACCEPT`, and std's `peer_addr` reads it.
- The README carries the scaffold's sections with this crate's figures. A stranger run is on record
  with no open finding against it.
- `.github/mutants-exempts` carries the three equivalent mutants with reasons. `mutation / mutation`
  has one green full run on `main` at the pinned template.
- Three harnesses become five, each new one with a falsification patch that `proofs / falsify`
  replays red. The fallback in `Ports::next` has documentation or goes away.
- Offsets and lengths share one type. The frame size is one named constant. The layout note states
  the byte order once.
- `cargo test`, `cargo kani`, `cargo clippy --all-targets` and `cargo doc` with warnings denied are
  green at the commit the move pins. They pass on 1.88 and on stable, for the host and the three
  bare-metal targets.
