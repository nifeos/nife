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

1. The crate's only proof runs nowhere after the move. `crates/spin_lock/src/lib.rs` line 307 LEFT: the template's.
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

2. The crate does not stand alone. `standalone.py` on the source as it is reports: no README, LEFT: move work.
   no `LICENSE-MIT`, no `LICENSE-APACHE`, 6 lines that cite paths into nife (first
   `src/lib.rs` line 8; `notes/memory-ordering.md`, `notes/interleaving.md`,
   `notes/interleaving/what-was-modeled.md`, `script/interleaving-check`, `kernel/src/sync.rs`),
   and 8 `Name:` blocks (first `src/lib.rs` line 21). `Cargo.toml` lines 11 to 16 cites `notes/interleaving.md`
   and `script/interleaving-check` as well, and carries `publish = false`, `license.workspace` and
   `lints.workspace` with no `rust-version`, `description` or `repository`. Each is a finding
   `check / standalone` fails on. The fix is the routine move work: the move commit strips the
   `Name:` blocks (the standing ruling), rewrites the citations as plain prose, writes the manifest
   literals, and adds the two license files and the README (item 8). Effort S. Two-way.

3. The scaffold's lint table breaks this crate. `sample/Cargo.toml` in the template lists LEFT: move work.
   `check-cfg = ['cfg(kani)']` only. With that table the standalone copy fails
   `cargo clippy --all-targets -- -D warnings` with 14 `unexpected cfg condition name: loom`
   errors (verified). nife hides this in `[workspace.lints.rust]`, which does not move. Fix: the
   moved manifest's `[lints.rust]` carries
   `unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)', 'cfg(loom)'] }`, and the
   scaffold's sample and README say that a loom crate adds `cfg(loom)`. Effort S. Two-way.

### P1

4. Another crate already holds the name on crates.io. crates.io treats `_` and `-` as the same character LEFT: the architect's (decision below).
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

7. The second falsification is a hand step, not a gate. `notes/interleaving/what-was-modeled.md` LEFT: the template's.
   records that weakening the compare-exchange's success ordering to Relaxed fails the four
   positive models; this review replayed it (4 failures, the should-panic model passing). The
   in-crate should-panic model (`a_relaxed_release_publishes_nothing`, `src/lib.rs` line 472) keeps
   only the release-side falsification standing. The template's `falsify.py` replays Kani patches
   only. Fix: ship `falsifications/interleavings.two_takers_are_exclusive_and_ordered.patch` (the
   one-line Acquire-to-Relaxed change at `src/lib.rs` line 142) and teach the template's falsify gate a
   loom harness (run the loom job with the patch applied; the verdict is the test summary's
   failure count, not Kani's line). Until then the README says the step is manual. Effort M.
   Two-way.

8. No README, no example, no doc-test. `cargo test` runs 0 doc-tests. A stranger has only the LEFT: move work.
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

9. The standard library's accessors are missing. `std::sync::Mutex` and `spin::Mutex` both DONE (`the_standard_accessors_read_and_move_the_data`, `debug_prints_the_data_when_free_and_does_not_spin_when_held`; `get_mut` and `into_inner` are absent under `--cfg loom`).
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

11. Say what the models do not check. `read` and `write` (`src/lib.rs` line 77, `src/lib.rs` line 87) DONE (crate documentation).
    return loom's pointer out of `with` and `with_mut`, which loom's documentation names as the
    pattern it does not track for duration. The causality check still fires on every access, which
    is the property the crate claims, and the review confirmed it. Fix: say so in the comment at
    `src/lib.rs` line 73 and in the README's "What is proven"; or hold loom's `ConstPtr` and `MutPtr`
    in the guard under `cfg(loom)` so the guard tracks the duration too. Effort S for the words, M for
    the pointers. Two-way.

12. `T: ?Sized`. std and `spin` both allow it; nothing here needs `Sized` except `new`. LEFT: loom's cell is sized.
    Relaxing the bound on the type and the guard is compatible. Effort S. Two-way.

13. `repr(C)` reads as a promise. `src/lib.rs` lines 46 to 50 says the flag sits at offset 0 so an DONE (the comment).
    inlined `lock` needs no offset arithmetic. The fields are private, so no caller can rely on the
    layout soundly, and the codegen claim is on nobody's record this review found (unverified).
    Fix: either cite a measurement in the comment or say the layout is an implementation choice
    and not part of the API. Effort S. Two-way.

14. Pick the `rust-version`. The crate builds and tests on 1.85, the edition's floor LEFT: move work.
    (verified); every review package so far writes 1.88. Either is defensible; the number is the
    maintainer's. Effort S. Two-way.

15. Loom's lockfile footprint. In a repository of its own the crate's `Cargo.lock` holds 31 LEFT: awareness only.
    packages, 30 of them loom's tree (`tracing-subscriber`, `regex-automata`, `generator`,
    `windows-sys` among them). `cargo deny` passes them (verified), and Dependabot will open bumps
    for a tree only the model run links. No action beyond awareness; the README says why the
    lockfile is large. Effort S. Two-way.

16. Considered and not recommended. `is_locked` (not std; `try_lock` answers it) and `try_lock_weak` LEFT: not recommended.
  (no caller). Also not recommended: a `lock_api::RawMutex` impl (a third-party dependency, which
  needs its own ruling and would give loom nothing), and a fair or ticket variant (the kernel's
  sections are short and the layer above this crate masks interrupts).

## Decisions for the architect

One one-way door.

### Under what name does the crate leave

Background. The crate is `spin_lock` in nife, ratified with its six public names in milestone 953.
crates.io reads `_` and `-` as one character, and `spin-lock` 0.2.1 already exists there: a small
crate with 15 downloads in the last 90 days, last published about two years before this review.
So `spin_lock` can never reach crates.io under its ratified name. Today that costs
nothing: every consumer pins by git `rev`, and a standing ruling holds publishing for every crate
until several crates have left and a combine review finishes. Another standing ruling says a crate
leaves for its own repository only if it is publishable or used by more than one repository. The
three callers all live in nife today. Whether they still share one repository after the kernel
leaves is not yet settled. The names below were free on crates.io when this review looked: `test_and_set_lock`, `checked_spin_lock`, `model_checked_spin_lock`, `ttas_lock`,
`flag_lock`.

Recommendation first: keep `spin_lock` as the package and repository name, publish nothing, and
record in the crate's README and in the review record that another crate holds the crates.io name. Decide a
publication name only if a publication is ever wanted, and then before the first tag. The reason is
the standing publication hold and the "useful or shared" rule. The crate's unique value is the
loom-ready source and its models, which `spin` (192 million recent downloads) does not offer. That
value is a property of how nife verifies, not a demand anyone outside has shown. Renaming a
crate nobody outside consumes, to free a registry name nobody has asked for, is work with no
reader.

If you want a publication, the alternative is to rename before the move, so the repository, the
package and the registry agree from the first commit. `test_and_set_lock` is the name milestone
953's ruling already weighed and set aside only for length; it says exactly what the thing is,
which is the headline naming rule, and it is free. `checked_spin_lock` and
`model_checked_spin_lock` name the verification rather than the lock, the shape the tree refused
for `script/interleaving-check`'s predecessor.

Prior art: `spin` took the family's short name on crates.io and the flag-lock crates that followed
(`spinning_top`, `spin-lock`, `spinlock`) each took a qualified or compound spelling. Linux calls
the thing `spinlock_t` and the queued variant `qspinlock`; FreeBSD calls the type `mtx` and the
variant `MTX_SPIN`, naming the discipline rather than the mechanism.

Concrete example: with the recommendation, nife's manifests keep
`spin_lock = { git = "https://github.com/nifeos/spin_lock", rev = "<sha>", version = "0.1.0" }` and
the kernel keeps `use spin_lock::SpinLock`. With the alternative, the move commit writes
`test_and_set_lock` in three manifests, in `kernel/src/sync.rs`, in one kernel static, in six
`system_tests` statics and in `tools/host_reordering_litmus`, and the tag `v0.1.0` is the first
name the registry would ever see.

## Done when

The checks are in [the appendix](1109-spin-lock-outside-review/done-when.md).

## BUGS

- Item 1's fix is the template's, and `clock_contract` needs it before this crate does if
  milestone 1017 (the contract crates leave in this order)'s order holds; the two should land once.
- The Linux and BSD rows of the prior-art table are from memory and marked unverified.
- Item 13's codegen claim was not measured here in either direction.
- The review did not run the template's workflows, because no repository exists; every template
  finding comes from running the template's helpers and reading its workflow files.
- This review found nothing security-relevant against this crate.

## Follow-on

- **Milestone 1017.** The crate moves in its order; the move lane takes the items marked move work.
- **Outstanding.** Items 1, 7 and 12: the template has no loom job; loom's cell is sized.
- **Refused.** Item 16: `try_lock` already answers `is_locked`, and a `lock_api` impl is a new dependency.

## Index row

An outside review of `spin_lock` before it leaves. The moved repository would run none of the
crate's five loom models (the template has no loom job). The scaffold's lint table rejects
`cfg(loom)`. The test modules dodge cargo-mutants' skip. The ratified name is already taken on
crates.io. Three P0, seven P1 and six P2 items; one one-way door, the publication name.
