# Done when

An appendix to [the milestone](../1084-current-cpu-contract-outside-review.md).

- `script/crate-standalone --no-kani current_cpu_contract` reports no failure in `manifest`,
  `files`, `citations` or `build`, and `names` once the lane applies Decision B.
- `helpers/lint_externalized_crates.py` reports the crate clean and its ceiling entry deleted.
- `helpers/template_report_gates.py --list current_cpu_contract` prints nothing.
- The README carries the template's required headings in order and states the page, its three
  words and their unknown states, the staleness caveat, the `!Send`/`!Sync` reason, the append-only
  layout rule and the single-file constraint. No doc comment names a nife path, section, milestone,
  program or person.
- `cpu()` answers `None` for a word at or past `CPU_ID_BOUND`, with a test; `allowance()` answers
  `None` for a word the target cannot hold; the doc of `CPU_ID_BOUND` states the identity rule
  Decision A settles.
- `word()` offsets a pointer rather than an integer, and `cargo miri test` prints no
  integer-to-pointer warning for the reader.
- `publish` and `new` carry the debug assertions of item 8, and the release build of `publish` is
  still one store (checked by reading the assembly once, or by the icount tripwire).
- Either two Kani harnesses with falsification patches pass `proofs / kani` and `proofs / falsify`
  in the crate's repository, or `## What is proven` states that nothing is and why.
- `cargo test`, `cargo clippy --all-targets -D warnings`, `cargo doc -D warnings` and the three
  bare-metal `cargo build --lib` runs are green on the pinned nightly and on the stated
  `rust-version`.
- The 14 programs, the kernel, the runtime, the fixtures, the system tests and the farm build
  against the renamed items, and the system test still carries the bound across the boundary.
