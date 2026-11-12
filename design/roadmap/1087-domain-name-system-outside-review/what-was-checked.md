# What was checked, and how

An appendix to [the milestone](../1087-domain-name-system-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at `main` and a clone of
`nifeos/crate-workflows`, on one Linux host with stable Rust 1.99.1:

- `cargo test -p domain_name_system`: 40 integration tests and 4 documentation tests pass.
  `cargo clippy --all-targets -- -D warnings` and `cargo doc` with `RUSTDOCFLAGS=-D warnings` are
  clean. The library builds for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`
  and `x86_64-unknown-none`.
- Kani 0.68.0 verifies all three harnesses. Each of the three falsification patches applies and
  fails exactly its own harness: the two codec patches at the assertion their header names, the
  decoder patch at the unwinding assertion, as its header predicts.
- `script/crate-standalone --no-kani domain_name_system`: 10 failures, listed under items 1 to 4;
  the build step passes for the host and the three bare-metal targets.
- `script/crate-review-packet` and `python3 helpers/name_release.py --draft domain_name_system`:
  one ratified crate name, five ratified constants, 48 public names with no `Name:` block.
- The template's naming helpers over `src/` and `tests/`: `unit_symbols` and `british_spelling`
  clean. `spelled_out` 2 findings. `register_names` 15 findings, all on the word `address` (item 7).
  The `abbreviations` report judged one name here, which looks like a harness problem on this side and stays unverified.
- Type sizes, measured with a scratch program: `Name` 256 bytes, `Header` 12, `Question` 260,
  `Record` 288, `Reader` 40, `Query` 258, `Answer` 296, `Reject` 2, `Error` 1, `TcpReply` 24,
  `Result<Answer, Reject>` 300.
- Three behaviors probed with the same program: a reply whose A record carries TTL `0xffff_ffff`
  yields `ttl()` of 4,294,967,295 (item 8). `Name::from_dotted("-a.test")` succeeds
  (item 14). A pointer to a later offset is `Error::PointerForward`.
- The recorded mutation census (`notes/project-metrics/mutation-census.csv`, the latest row):
  228 mutants, 200 caught, 3 missed, 4 timed out, 21 unviable, 0 real holes. `mutation-triage.csv` triages the three survivors as equivalent, with reasons. No census ran here.
- Prior art: `smoltcp` 0.14.0's `wire/dns.rs` read from the local registry. The comparison rows
  for `hickory-proto`, `domain`, `simple-dns` and `dns-parser` are from the other-implementations
  appendix of the releasable-crates note (`notes/releasable-crates-*/other-implementations.md`) and
  from memory of those crates' APIs, not re-read here. Each carries an unverified mark where it bears on a recommendation.

Inferred from reading, not run: that the crate builds at Rust 1.88, which `Header::write` needs for
`<[T]>::as_chunks_mut`. Its release notes say it stabilized in 1.88, and no 1.88 toolchain existed here. The behavior of the template's `coverage` and `mutation` jobs on this crate before a
repository exists.

Not run: a mutation census (outside its window), a fuzz campaign, the template's workflows, a
release dry run, `cargo semver-checks` (no previous release). The review read no fuzz finding, seed or crash input beyond the three captured fixtures the tests already ship.

Hostname of the reviewing machine: stated in the review's final message, not here.
