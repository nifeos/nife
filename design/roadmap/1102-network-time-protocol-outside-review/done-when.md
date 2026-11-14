# Done when

An appendix to [the milestone](../1102-network-time-protocol-outside-review.md).

- `Cargo.toml` inherits nothing, carries `rust-version`, `description`, `readme`, `keywords`,
  `categories` and `license = "MIT OR Apache-2.0"`, and no comment points outside the repository.
- The repository holds a README with `EXAMPLES`, "What is proven", "Building and testing", "Running
  the proofs", `BUGS`, the minimum Rust version and the license. Both license files. A changelog.
  And `docs/stranger-test.md` with a run recorded before the first tag.
- The crate root's documentation opens with `(network_time_protocol, RFC 5905)` and expands every
  acronym at its first use; `contract_names`, `british_spelling`, `register_names` and
  `prose_acronyms` report zero findings.
- The pull request applies the names Decision 2 decides, with the three callers in nife
  bumped in the same pull request that moves them, and no `#[doc(alias)]` of the earlier spelling.
- The crate has no dependency.
- No shipped text names a nife note, script, milestone, pull request, service or person.
- `#![forbid(unsafe_code)]` is present; the fuzz caller opts in with one target over `Query::accept`.
- Eight Kani harnesses pass. Each has a falsification patch that `proofs / falsify` replays red. The
  eighth states that `accept` takes a consistent exchange, with the RFC's offset and delay.
- `Interval::nanoseconds` and `microseconds` are total over the public constructor, by a narrowed
  parameter or saturation, with a harness.
- `Query::accept` refuses a root distance of 16 s and a reference timestamp after the transmit
  timestamp, and its documentation states the caller's replay duty.
- `cargo test`, `cargo kani`, the full `mutation` run (none missed outside the exemptions file) and
  the fuzz target are green at the commit the first tag names.
