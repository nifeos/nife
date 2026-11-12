# What was checked, and how

An appendix to [the milestone](../1099-machine-statistics-contract-outside-review.md).

Verified firsthand, in a fresh shallow clone of `nifeos/nife` at the merge of pull request 3467
and a clone of `nifeos/crate-workflows` at its head. The crate stood as a copy in a scratch
workspace with its two path dependencies beside it and the workspace's license and lints inlined. The checks:

- `cargo test`: six unit tests and one documentation test pass on the current stable and on 1.88.
  `cargo clippy --all-targets -- -D warnings` is green on stable for this crate. On 1.88 the same
  command is red, but in `address_space_map` (a dead constant, `LOW_HALF_SPLIT_SHIFTS`), not in
  this crate (BUGS). `cargo doc` with warnings denied is clean.
- The crate builds and passes clippy with warnings denied for `x86_64-unknown-none`,
  `aarch64-unknown-none-softfloat` and `riscv64imac-unknown-none-elf` on the nightly this tree
  pins.
- Miri on the pinned nightly: all seven tests pass. A scratch test the review added, reading a
  host-aligned frame through `Snapshot::read`, also passes under Miri, with an integer-to-pointer
  provenance warning at the cast in `counter` (item 6).
- cargo-llvm-cov: 92% of lines. The uncovered lines are exactly the two `unsafe` functions,
  `counter` (lines 142 to 145) and `Snapshot::read` (lines 249 to 256). No host test reaches
  either (item 8).
- `cargo kani` on the copy: "No proof harnesses were found to verify", exit 0. The template's
  `proofs / kani` job therefore passes vacuously on a crate with no harness; the sibling reviews
  left this unverified, and it is now verified (item 7).
- A scratch test over a page that carries the magic and the version with every counter at
  `u64::MAX`: `busy_ticks()` panics with "attempt to add with overflow" in a debug build (item 5).
- The template's helpers over the copy: `unit_symbols`, `register_names`, `british_spelling`,
  `question_shape`, `service_model`, `prose_figures`, `history_phrases`, `function_length`,
  `file_length` and `comment_blocks` report nothing. `spelled_out` refuses three names (`PAGE_VA`,
  and the parameters `page_va` and `va`). `prose_acronyms` refuses `PR` at line 34.
  `passive_voice` counts six passives in 2,153 words, over the template head's allowance of four.
  `unsafe_scan` refuses a fuzz opt-in, correctly, for the three `unsafe` sites. `abbreviations`
  (report only) does not know `runnable` and `offline`, both real words.
- `script/crate-standalone --no-kani`: 14 failures, listed under items 1 to 3.
- `script/crate-move --dry-run` into an empty directory, template at `v7`: eight findings for the
  move lane. The build step fails at `cargo generate-lockfile` on the inherited `license` before it
  can reach the two path dependencies, so the dry run never reports those (BUGS). The staged
  source keeps the citations of items 3 and 4 because the rewrite strips only `Name:` blocks. The
  mutation baseline row it carries reads 33 mutants, none missed, five unviable.
- `helpers/lint_externalized_crates.py --count`: one hit for this crate, at its ceiling of one
  (the sentence "nife does not page out", `src/lib.rs` line 16).
- Every caller: `kernel/src/machine_statistics.rs` (the writer). `kernel/src/sched/preemption.rs`, `kernel/src/sched/interrupt_route.rs`, `kernel/src/sched/placement.rs` and `kernel/src/memory.rs` (the call sites). `crates/free`, `crates/vmstat` and `crates/top` (the reader libraries and their host tests). `components/src/free.rs`, `components/src/vmstat.rs` and `components/src/top.rs` (the `INTERFACES` declarations and the `Snapshot::read` calls). `crates/system_initializer/src/spawn_loop.rs` (the mapping at the fixed address). `crates/grant_plan/src/manifest.rs` (the slot). Last, `system_tests/src/user/machine_statistics_tests.rs`. Neither `xtask/` nor `patches/` copies this
  crate's source into the `std` platform layer, so the path-copy hazard the sibling reviews found
  does not apply here.
- The interface id table in milestone 956's block assigns 4 to this crate, and
  `notes/interfaces-note.md` repeats it.
- Download figures for the prior-art crates, from the crates.io API.

Inferred from reading, not run: the behavior of `counter` on a target whose pointer is narrower
than 64 bits (item 6), and whether `#![no_std]` can be unconditional (item 14).

Not run: a mutation census (outside its window; the carried row says none missed). Also not run: a fuzz campaign (the crate has `unsafe` in shipped source and cannot opt in to the public one). Also not run: `cargo semver-checks` (no release to compare against), the template's full workflow set against a real repository, and nife's QEMU system tests.
