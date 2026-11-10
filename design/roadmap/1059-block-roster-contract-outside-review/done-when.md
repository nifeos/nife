# Done when, on a host and in CI

An appendix to [the milestone](../1059-block-roster-contract-outside-review.md).

- P0-1 and P0-2 have closed. `docs/stranger-test.md` carries a date, and the weekly `mutation` and
  `stranger cadence` jobs pass on `main` with `.cargo/mutants.toml` and one exemption in place.
- The callers pin a template release that carries `mutation.yml`, the caller exists, and the
  ruleset requires `check / mutation-diff`, which reports (P1-1).
- P1-2 to P1-8 have closed in the repository, each by a pull request that cites this block by
  number. `Roster::read` returns `Result<Roster, ReadError>`, `Roster::get` takes `index`, the three
  transport constants carry their aliases, `Device` derives `Hash` and `Ord` with
  `transport` declared first and a host test pinning the order, and `devices()` is an
  `ExactSizeIterator`. The two call sites in this organization build against the new signature.
- `cargo test`, `cargo kani` and the falsification replay stay green on every change, and a new
  harness arrives with its patch.
- Each P2 item has either closed or become an entry in the README's BUGS or an issue in the
  repository, with its door and size.
- Nothing goes to crates.io, and nobody places a tag without a separate go of the architect.

## What needs hardware or QEMU

Nothing. The crate is pure logic with host tests and proofs. The end-to-end reading of a roster the
kernel wrote stays with the consumer's own system test.
