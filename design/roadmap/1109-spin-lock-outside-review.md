---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 953, 1017
decision_dependencies: 39, 46
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1109. `spin_lock`: what an outside maintainer would fix before it leaves

Raised by a read-only outside review of `crates/spin_lock` as a stranger to this tree would meet
it. It changes nothing; it lists what the lanes that move the crate change, and the one question
only an architect can answer.

The settled baseline this review does not reopen is the rulings recorded in milestone 953 (the
kernel's lock proves what it takes from `spin`). They ratified the crate name, `SpinLock`,
`SpinLockGuard`, both `new`, `lock`, `try_lock` and `force_unlock`. They chose a test-and-set lock with
loom-ready atomics over `spin`. They placed `force_unlock` outside the models by construction. The baseline also holds
the mutation triage that classes the `pause` survivor as equivalent. No review package exists for
this crate in `notes/crate-reviews/`, so the baseline is milestone 953's built record and the
`Name:` blocks in the source. No later rule contradicts a ratified name here: the six public names
carry no unit, no abbreviation, and `lock` and `try_lock` are the standard library's spellings.

Reuse: the template `nifeos/crate-workflows` and its scaffold, loom 0.7, cargo-mutants,
cargo-llvm-cov, Miri, `cargo deny`, the template's name helpers. No new tool.

## Goal

A stranger who finds the crate in its own repository can tell in one screen what it is (a
test-and-set spinlock whose exclusion and happens-before edge loom checks) and what it is not (fair,
interrupt-safe, poisoning, reentrant). The stranger can build, test and run the models from the
README alone. Every gate in that repository is green for the crate's own reasons. In particular, the one
proof the crate has runs somewhere after the move.

## Finding status

- P0-1, the only proof runs nowhere after the move: Done, the template's `proofs / loom`
  ([nifeos/crate-workflows#65](https://github.com/nifeos/crate-workflows/pull/65)).
- P0-2, the crate does not stand alone: Open. [#3636](https://github.com/nifeos/nife/pull/3636)
  added the README, the license files and the manifest literals; the `Name:` blocks, the citations
  of paths in this tree and the manifest's `repository` remain, all move work.
- P0-3, the scaffold's lint table: Done, [#3636](https://github.com/nifeos/nife/pull/3636).
- P1-4, another crate holds the name on crates.io: Open, and an open decision below. The `Name:`
  block records the recommendation as provisional and the README states the clash.
- P1-5, test modules hidden from cargo-mutants' skip: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-6, the `pause` survivor's exemption line: Open. One `.github/mutants-exempts` line with the
  triage's reason, at the move.
- P1-7, the second falsification is a hand step: Open. The loom falsification patch here and a
  loom form of the template's falsify gate in nifeos/crate-workflows remain; the README calls the
  step manual.
- P1-8, no README, example or doc-test: Done, [#3636](https://github.com/nifeos/nife/pull/3636).
- P1-9, the standard library's accessors: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P1-10, documentation for the kernel's reader: Done,
  [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-11, what the models do not check: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-12, `T: ?Sized`: Done, [#3636](https://github.com/nifeos/nife/pull/3636).
- P2-13, `repr(C)` reads as a promise: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- P2-14, the `rust-version`: Done, [#3636](https://github.com/nifeos/nife/pull/3636) (1.88,
  tested on it).
- P2-15, loom's lockfile footprint: Done, [#3636](https://github.com/nifeos/nife/pull/3636) (the
  README says why).
- P2-16, `is_locked`, `try_lock_weak`, `lock_api` and a fair variant: Declined. `try_lock` answers
  `is_locked`, nothing calls `try_lock_weak`, and a `lock_api` impl is a third-party dependency
  that gives loom nothing.

## Open decisions

### The name the crate leaves under

The crate is a small lock for the kernel and its tests: one flag that a taker sets and spins on,
with five machine-checked models of its ordering. Its name in nife is `spin_lock`, ratified with its
six public names in milestone 953 (the kernel's lock proves what it takes from spin). crates.io
reads `_` and `-` as one character, and `spin-lock` 0.2.1 already exists there: a small crate with
15 downloads in the last 90 days, last published about two years before this review. So `spin_lock`
can never reach crates.io under its own name.

Today that costs nothing. Every consumer pins by git `rev`, and a standing ruling holds publishing
for every crate until the last outside review lands. A crate leaves for its own repository only if
it is publishable or used by more than one repository. The three callers all live in nife. The
crate's unique value is its loom-ready source and models, which `spin` (192 million recent
downloads) does not offer; no one outside has asked for it.

Recommendation: A.

- A. Keep `spin_lock` as the package and repository name, publish nothing, and choose a
  publication name only if a publication is ever wanted, before the first tag. Renaming a crate
  nobody outside consumes, to free a registry name nobody has asked for, is work with no reader.
- B. Rename to `test_and_set_lock` before the move, so the repository, the package and the
  registry agree from the first commit. Milestone 953's ruling weighed it and set it aside only
  for length; it says exactly what the thing is, and it was free on crates.io when this review
  looked.
- C. Rename to `checked_spin_lock` or `model_checked_spin_lock`. Both name the verification rather
  than the lock, the shape the tree refused for `script/interleaving-check`'s predecessor.

Door: one-way. The repository name is public from the day it exists, and the first tag fixes the
name the registry would see; a rename after that breaks every pin.

Prior art: `spin` took the family's short name on crates.io, and the flag-lock crates that followed
(`spinning_top`, `spin-lock`, `spinlock`) each took a qualified or compound spelling. Linux calls
the thing `spinlock_t` and the queued variant `qspinlock`; FreeBSD calls the type `mtx` and the
variant `MTX_SPIN`, naming the discipline rather than the mechanism. nife differs by naming the
mechanism and keeping the models beside it. Example: with A, nife's manifests keep
`spin_lock = { git = "https://github.com/nifeos/spin_lock", rev = "<sha>", version = "0.1.0" }` and
the kernel keeps `use spin_lock::SpinLock`. With B, the move commit writes `test_and_set_lock` in
three manifests, in `kernel/src/sync.rs`, in one kernel static, in six `system_tests` statics and in
`tools/host_reordering_litmus`.

Blocks: the move and the tag under B or C; under A, only a first publish.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1109-spin-lock-outside-review/what-was-checked.md).

## Soundness, read line by line

Four `unsafe` sites, all compiled and reached by every caller. Each holds:

- `unsafe impl<T: Send> Sync for SpinLock<T>` (`src/lib.rs` line 59): the same bound as
  `std::sync::Mutex`. `Send` is automatic from `AtomicBool` and `UnsafeCell<T>`.
- The guard's `Deref` and `DerefMut` (`src/lib.rs` line 211, `src/lib.rs` line 220): a guard exists only
  after a successful Acquire compare-exchange, nothing clears the flag before its drop, and the
  borrow ties to the guard. The guard's auto traits are right: `Send` when `T: Send` (as
  `spin`'s guard, unlike `std`'s, which is `!Send` for pthread reasons a spinlock does not have)
  and `Sync` when `T: Send + Sync`, through `PhantomData<&'a mut T>`.
- `unsafe fn force_unlock` (`src/lib.rs` line 175): a Release store under a stated contract; its one
  caller in the tree is the kernel's panic path.

The lock loop (`src/lib.rs` line 140) is the test-and-test-and-set shape `spin` uses: a weak Acquire
compare-exchange, a Relaxed read loop with the architectural pause while the flag holds, retried
until the exchange wins. A spurious failure on a free lock falls through the inner loop at once.
`try_lock` uses the strong exchange so a `None` is a refusal, and builds the guard only on the
success path. The guard's drop is the Release store. Nothing here is wrong.

What the loom models prove and do not prove, as read against loom's own cell tracking. Every access
to the data goes through `read` and `write` (`src/lib.rs` line 77, `src/lib.rs` line 87). They record a point
access and check that the last write happens-before each read. They also check that every read and
the last write happen-before each write. That is the exclusion-plus-ordering property, and the
should-panic model shows the check fires. The pointer leaves loom's closure (`cell.with(|p| p)`),
which loom's documentation names as the pattern it cannot track for duration; so the models check
the ordering of every access and not that two accesses overlap in time. Loom's scheduler runs one
thread at a time, so the duration check adds nothing to these models, but the documentation
should say so (item 11). The three-taker model's preemption bound of one is loom's limit on
spin-wait loops, recorded honestly. Loom models C11, not aarch64 or riscv64; the crate says so.

## Prior art

The comparison with nearby work is in [the appendix](1109-spin-lock-outside-review/prior-art.md).

## The items

Each item: what is wrong, where, why it matters, the fix, effort (S/M/L), and whether it is a
one-way door (a published name, wire format, identity, layout or dependency) or two-way.

### P0

1. The crate's only proof runs nowhere after the move. `crates/spin_lock/src/lib.rs` line 307 DONE (the template's `proofs / loom`, nifeos/crate-workflows#65).
   holds the five loom models, and `script/interleaving-check` runs them as nife's required
   `interleavings` job. The template has no loom job: `check.yml` runs `cargo test` without
   `--cfg loom`, so the models are not compiled, and `proofs.yml`'s `falsify.py` reports
   "no `#[kani::proof]` harness under src/; nothing to falsify" and passes (verified on the
   standalone copy). A repository whose proof gate is green while its proof never runs is the
   failure the template exists to prevent. Fix: add a `loom` job to the template (shared gates live
   once in the template, per the standing ruling), `RUSTFLAGS="--cfg loom -D warnings" cargo test
   --release --lib`. Give it a step that fails when a crate declares `cfg(loom)` in `check-cfg` and
   no model ran. The caller's `proofs.yml` then enables it. Six nife crates carry `cfg(loom)`
   today (`clock_contract`, `memory_corruption_canary_gate`, `memory_regions`, `spin_lock`,
   `thread_wake_handshake`, `work_steal_slot`), and `clock_contract` is in step 2 of milestone
   1017's order, so the job serves more than this crate. Effort M. Two-way.

2. The crate does not stand alone. `standalone.py` on the source as it is reports: no README, PARTIAL: the `Name:` blocks, the citations and `repository` stay LEFT: move work.
   no `LICENSE-MIT`, no `LICENSE-APACHE`, 6 lines that cite paths into nife (first
   `src/lib.rs` line 8; `notes/memory-ordering.md`, `notes/interleaving.md`,
   `notes/interleaving/what-was-modeled.md`, `script/interleaving-check`, `kernel/src/sync.rs`),
   and 8 `Name:` blocks (first `src/lib.rs` line 21). `Cargo.toml` lines 11 to 16 cites `notes/interleaving.md`
   and `script/interleaving-check` as well, and carries `publish = false`, `license.workspace` and
   `lints.workspace` with no `rust-version`, `description` or `repository`. Each is a finding
   `check / standalone` fails on. The fix is the routine move work: the move commit strips the
   `Name:` blocks (the standing ruling), rewrites the citations as plain prose, writes the manifest
   literals, and adds the two license files and the README (item 8). Effort S. Two-way.

3. The scaffold's lint table breaks this crate. `sample/Cargo.toml` in the template lists DONE (the manifest's own table).
   `check-cfg = ['cfg(kani)']` only. With that table the standalone copy fails
   `cargo clippy --all-targets -- -D warnings` with 14 `unexpected cfg condition name: loom`
   errors (verified). nife hides this in `[workspace.lints.rust]`, which does not move. Fix: the
   moved manifest's `[lints.rust]` carries
   `unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)', 'cfg(loom)'] }`, and the
   scaffold's sample and README say that a loom crate adds `cfg(loom)`. Effort S. Two-way.

### P1

4. Another crate already holds the name on crates.io. crates.io treats `_` and `-` as the same character OPEN: under "Open decisions" above; provisional in the `Name:` block.
   in a crate name, and `spin-lock` 0.2.1 exists (verified by the API: `spin_lock` resolves to it).
   The ratified name cannot go to the registry, ever, under the registry's rules. Nothing breaks today:
   nife and basalt pin by git `rev`, and a standing ruling holds publishing for every crate.
   But the move sets the repository name and the package name, and every later rename costs
   every pin and every `use spin_lock::` path in three manifests. This is the one one-way door the
   review found; see "Decisions for the architect". Effort S if ruled before the move, L after.
   One-way.

5. The test modules are invisible to cargo-mutants' test-module skip. `src/lib.rs` line 227 is DONE (both modules behind `#[cfg(test)]`).
   `#[cfg(all(test, not(loom)))]` and `src/lib.rs` line 307 is `#[cfg(all(test, loom))]`. cargo-mutants
   skips a module behind a plain `#[cfg(test)]` and does not read `all(...)`. So `cargo mutants
   --list` on the crate alone names 28 mutants. Eight of them are in `tests::` and `interleavings::`
   (`bounded`, `Reached::mark`, `Reached::assert`, `take`, `publish_through_the_lock` and the
   arithmetic in `take`). Inside nife, `.cargo/mutants.toml` excludes `tests::` and
   `interleavings::` by regex; that file does not move. In the moved repository the template's
   `mutation` verdict would report eight survivors that are test code, and the
   `mutants-exempts` list would have to carry eight lines or the crate would carry a
   `.cargo/mutants.toml` of its own. Fix: write the gates as two attributes, `#[cfg(test)]` then
   `#[cfg(not(loom))]` or `#[cfg(loom)]`; the listing then drops to 20 (verified). Effort S.
   Two-way.

6. The one equivalent survivor needs its exemption line. `pause` at `src/lib.rs` line 98: off LEFT: move work.
   loom its body is `core::hint::spin_loop()`, which nothing can observe, so "replace pause with
   ()" survives; the recorded triage calls it equivalent. The template refuses a survivor without a
   `.github/mutants-exempts` line that gives the reason. Fix: one line at the move, with the
   triage's reason. Effort S. Two-way.

7. The second falsification is a hand step, not a gate. `notes/interleaving/what-was-modeled.md` LEFT: the template's; the README calls the step manual.
   records that weakening the compare-exchange's success ordering to Relaxed fails the four
   positive models; this review replayed it (4 failures, the should-panic model passing). The
   in-crate should-panic model (`a_relaxed_release_publishes_nothing`, `src/lib.rs` line 472) keeps
   only the release-side falsification standing. The template's `falsify.py` replays Kani patches
   only. Fix: ship `falsifications/interleavings.two_takers_are_exclusive_and_ordered.patch` (the
   one-line Acquire-to-Relaxed change at `src/lib.rs` line 142) and teach the template's falsify gate a
   loom harness (run the loom job with the patch applied; the verdict is the test summary's
   failure count, not Kani's line). Until then the README says the step is manual. Effort M.
   Two-way.

8. No README, no example, no doc-test. `cargo test` runs 0 doc-tests. A stranger has only the DONE (the README and a doc-test).
   crate-level comment, which opens "The kernel's spinlock" and refers to `InterruptSafeMutex` and
   nife's notes. Fix: write the README the scaffold asks for. `## EXAMPLES` holds a static
   `SpinLock<u32>` incremented under `lock()` and read back, as a doc-test on `SpinLock`. `## What
   is proven` gives the five models in a user's words, and the C11 limit. `## Building and testing`
   gives the loom command, says that it needs `--release` only for speed, and says that it took 0.02
   s after the build here. `## BUGS` lists no fairness, no poisoning, no reentrancy, `force_unlock`
   outside the models, loom's C11 limit and the three-taker bound. The README also needs `## Minimum
   Rust version`. The scaffold's
   `README-crate.md` assumes Kani in "Running the proofs" and "Running one falsification patch";
   those sections need a loom form. Effort M. Two-way.

9. The standard library's accessors are missing. `std::sync::Mutex` and `spin::Mutex` both DONE (`the_standard_methods_read_and_move_the_data`, `debug_prints_the_data_when_free_and_does_not_spin_when_held`; `get_mut` and `into_inner` are absent under `--cfg loom`).
   have `get_mut(&mut self) -> &mut T`, `into_inner(self) -> T`, `Default`, `From<T>` and `Debug`.
   The rule that a method takes std's spelling and meaning where std has one settles the names,
   and adding them is compatible. `Debug` for `SpinLock` should print through `try_lock` and write
   `<locked>` on refusal, as std does, so a debug print never spins. The guard gets `Debug` and
   `Display` through `T`. Effort S each. Two-way.

10. The documentation speaks to the kernel's reader. `src/lib.rs` lines 1 to 22 and the `lock` doc at DONE (crate documentation).
  `src/lib.rs` line 131 say what the crate does and nothing a stranger needs about what it does not do. A
  second `lock()` on the thread that holds the lock spins forever (std says a reentrant lock may
  panic or deadlock; a spinlock always deadlocks). A panic while holding releases the lock and
  leaves the data as it was, with no poisoning. The guard is `Send`, unlike std's. Interrupts are
  the caller's business, which is the whole reason the kernel wraps it.
    Fix: a crate-level comment of one paragraph on what it is, one on what it is not, one on loom.
    Effort M. Two-way.

### P2

The P2 items are in [the appendix](1109-spin-lock-outside-review/later-items.md).

## Decisions for the architect

None ruled. The one decision this review raised, the name, is under "Open decisions" above.

## Done when

The checks are in [the appendix](1109-spin-lock-outside-review/done-when.md).

## BUGS

- The Linux and BSD rows of the prior-art table are from memory and marked unverified.
- Item 13's codegen claim was not measured here in either direction.
- The review did not run the template's workflows, because no repository exists; every template
  finding comes from running the template's helpers and reading its workflow files.
- This review found nothing security-relevant against this crate.

## Follow-on

- **Milestone 1017.** The crate moves in its order; the move lane takes the items marked move work.
- **Outstanding.** Item 7: the template's falsify gate cannot replay a loom patch, checked
  against nifeos/crate-workflows' `falsify.py`.
- **Decision.** Item 4, the name, under "Open decisions" above.
- **Refused.** Item 16: `try_lock` already answers `is_locked`, and a `lock_api` impl is a new
  dependency.

## Index row

An outside review of `spin_lock` before it leaves. The template's falsify gate cannot replay the
crate's loom falsification, and the move commit still owes the `Name:` blocks, the citations and the
`pause` exemption. The ratified name is already taken on crates.io. Three P0, seven P1 and six P2
items; one one-way door, the publication name.
