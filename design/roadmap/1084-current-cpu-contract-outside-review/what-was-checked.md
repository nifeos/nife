# What was checked, and how

An appendix to [the milestone](../1084-current-cpu-contract-outside-review.md).

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main` (merge of
#3514), with the tree's pinned nightly:

- `cargo test -p current_cpu_contract`: 13 unit tests and 3 documentation tests pass.
- `cargo clippy -p current_cpu_contract --all-targets -- -D warnings` and `cargo doc` with
  `RUSTDOCFLAGS=-D warnings`: clean.
- `cargo build --lib` for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and
  `x86_64-unknown-none`: clean.
- `cargo miri test -p current_cpu_contract`: every test and doctest passes.
- `script/crate-standalone --no-kani current_cpu_contract`: 13 failures (items 1 to 4).
- `helpers/name_release.py --draft current_cpu_contract`: 19 rows, 1 ratified (the crate), 4
  provisional, 14 with no `Name:` block, 2 pre-ruled renames.
- `helpers/lint_externalized_crates.py --count`: 3 consumer-name hits, at the crate's ceiling.
- `helpers/template_report_gates.py --list current_cpu_contract`: 6 abbreviation findings (every
  one the component `va`), 1 prose acronym and 1 README acronym (`TCB` unexpanded at `src/lib.rs:17`).
- Every caller: `kernel` (the writer, `publish` at two sites and `build_page_allowing` at two) and `user_mode_runtime` (the reader). `machine_statistics_contract` re-exports `CPU_ID_BOUND` and sizes its own page by it. In `components`, fourteen programs declare `INTERFACE_ID` and `VERSION`. The rest are `fixtures`, `system_tests` and `xtask`'s farm, which copies `src/lib.rs` textually
  into the standard library's platform layer.
- A three-test Miri experiment on the reader's shape (`&AtomicU64` formed over read-only bytes):
  Miri accepts it over a `static` and refuses it over an immutable local binding (Stacked Borrows
  retag for `SharedReadWrite` over a `SharedReadOnly` tag). That is what the crate's `BUGS`
  section says. Miri also warns on every integer-to-pointer cast in that shape (item 7).
- Linux's `rseq` user header, read from the kernel tree, for the prior-art box.

Inferred from reading, not run: the behavior on a target whose `usize` is narrower than 64 bits
(item 6), and the behavior of the template's `proofs / kani` job on a crate with no harness (item 10).

Not run: a mutation census (outside its window; the review found no recorded figures for this crate in the tree), Kani (the crate has no harness), and `cargo semver-checks` (no previous release). Also not run: the template's `standalone.py` on a copy (nife's `crate-standalone` covers the same checks), and a build at Rust 1.88, the family's `rust-version` (nothing confirms that this crate builds there; the sibling states it).
