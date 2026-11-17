# The ledger

An appendix to [the milestone](../1084-current-cpu-contract-outside-review.md).

| item | state |
|---|---|
| 1 | done: the manifest is the crate's own (license, lints, `rust-version` 1.88 built once, description, repository-free metadata, docs.rs targets, a `publish = false` comment a stranger can act on); the dependency on `address_space_layout` stays a path with a version until that crate leaves |
| 2 | done: README with the template's headings, both license files |
| 3 | done: the doc comments are in the crate's own terms, with no section, milestone, program or person named; the crate-level `Name:` block no longer quotes the decider |
| 4 | done, provisional (decision B, decided by the maintainer 2026-11-14): `PAGE_CONTENT_BYTES`, `PAGE_VIRTUAL_ADDRESS`, `thread_page_virtual_address`, `build_page_with`, `CPU_OFFSET_BYTES`, and the spelled-out parameters; no alias of the old names. The renamed items carry `Name: provisional.`, and the architect has not ratified any of them |
| 5 | done (decision A) |
| 6 | done: `allowance` reads an allowance the target cannot hold as unknown (`usize::try_from`) |
| 7 | done: `word` reaches its load through `base.add(off)`, which keeps provenance |
| 8 | done: `publish` asserts alignment, the sentinel and the bound in debug builds, and `new` asserts alignment; the tests that must hand the reader a bad word write it with a test helper instead of `publish` |
| 9 | done: `build_page_with(Option<NonZeroU64>, Option<NonZeroU64>)`, with `build_page()` the all-unknown page; the parameter types are the maintainer's, provisional |
| 10 | done: two Kani harnesses, each with a falsification patch that the sweep turns red |
| 11 | done: the `BUGS` list is the five facts a stranger can act on |
| 12 | the README and `BUGS` sentence are done; the farm check is not: the farm already inlines a crate's `pub mod` files, so "more than one source file" is not the failure, and the failure it would catch (a private `mod x;` or an `include!` in a copied file) needs a check that knows `cfg(test)` and `cfg(kani)`; see `BUGS` |
| 13 | done: `#![no_std]` |
| 14 | held: `CPU_OFFSET_BYTES` keeps its type `u64`, which is a published type and so a one-way door; recorded for the name review |
| 15 | done: the kernel's safety comment says the store stays inside the page's content |
| 16 | done: the crate's note links the kernel-tests note |
