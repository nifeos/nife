# P2: later

An appendix to [the milestone](../1081-capability-outside-review.md).

## P2: before the release, or when a lane is near

8. `Rights::ALL` is a hand-written literal.
   Finding: `ALL` is `0b1_1111` (line 84) and the doc warns that a right added without widening it
   is silently dropped at every delegation; the test `rights_bits_are_the_wire_format` is the only
   guard. `abi::rights::ALL` spells out the union.
   Change: `READ.union(WRITE).union(GRANT).union(ENUMERATE).union(BIND)` in a `const`, so the
   compiler keeps the invariant and the test keeps the wire value.
   Door: two-way. Effort: XS.

9. Abbreviated and stale parameter names.
   Finding: `reap_decision` and `survey_includes` take `fault_ep` and `invoked_ep`: `ep` abbreviates a word people say in full, and the object it names is now a rendezvous, which the same doc comments say. `derive` binds `src`; the tests name `Obj` and `orig`. The template's
   abbreviations report lists all eleven components. Parameters are names under the rule.
   Evidence: `src/lib.rs` lines 173, 193, 459, 724; `src/tests.rs` lines 4, 153.
   Change: `supervising_rendezvous`, `invoked_rendezvous`, `source`, `Object`, `original`
   (provisional); the kernel's two call sites pass positionally and do not change.
   Door: two-way. Effort: S.

10. `Error` is thin for a stranger.
    Finding: no `Display` or `core::error::Error` implementation, so a user cannot `?` it into a
    boxed error or print it without `Debug`; `insert_at` answers `NoFreeSlot` for an occupied slot,
    which also means "the table is full"; `get_with` answers `CannotWiden` for "you do not hold
    the right", a different fact. Capsicum separates these (`ENOTCAPABLE` versus `EBADF`).
    Evidence: `src/lib.rs` lines 197 to 207, 397 to 403, 435 to 444.
    Change: implement `Display` (and `core::error::Error`, which is stable in `core`); consider
    `SlotOccupied` and `InsufficientRights` variants (provisional), with the kernel mapping them
    onto its `abi::Error` as it does today.
    Door: two-way before the release; the variant set is a published name after it. Effort: S.

11. The process-wide peak is unconditional.
    Finding: `PEAK` is a `static AtomicU32` (line 506) raised on every first fill. On a target
    without 32-bit atomics (`thumbv6m`, `riscv32i` without the A extension) the crate does not
    compile; and a library that keeps a process-wide static for one consumer's idle loop surprises a
    reader who wanted a table.
    Change: a cargo feature (`peak`, provisional) that the kernel turns on and that gates `PEAK`,
    `note_peak` and `highest_seen`; or at least `#[cfg(target_has_atomic = "32")]`. The per-table
    `peak()` stays unconditional.
    Door: two-way now; a feature is a published name after the release. Effort: S.

12. No read-only walk, and `delete_matching` takes `Fn`.
    Finding: the crate offers no iterator over occupied slots, so the kernel walks `0..len()` with `get` in three places (`kernel/src/sched/capabilities.rs` lines 88, 149 and 328). One of them is a revocation sweep the crate already gave `delete_matching` to avoid. `delete_matching` takes `impl Fn`, where std's `retain` takes `FnMut`.
    Change: `iter(&self) -> impl Iterator<Item = (u64, Capability<O>)>` over the occupied mask;
    `FnMut`; then the three kernel sites become `delete_matching` or `iter`. The kernel half is a
    follow-up outside this crate.
    Door: two-way. Effort: S.

13. The mutation record travels as "unknown" when the triage knows it.
    Finding: the dry run writes into `.github/mutants-exempts` that it has no record of the three missed mutants. `notes/project-metrics/mutation-triage.csv` lines 270, 567 and 568 record them:
    `fill`'s `>` to `>=` (the global already holds the pair), `note_peak`'s `|` to `^` (the halves
    share no bit), and `1 << 0` to `1 >> 0`. The triage argues each equivalent, and each still survives on rerun.
    Change: write the three lines with their reasons into the staged `mutants-exempts` in the move
    pull request, so the first weekly run has nothing new to name.
    Door: two-way. Effort: XS.

14. One test pins the global exactly.
    Finding: `the_high_water_word_reports_the_occupancy_and_the_capacity_that_set_it` asserts
    `highest_seen() == (64, 64)` and holds only while no other test in the binary exceeds 64 of
    64; the comment says so. A future test with a wider ceiling than 64 is impossible, but one that
    fills 64 of 64 first merely ties, and the dependency on test ordering is the kind of thing a
    stranger's first added test breaks.
    Change: assert the peak half is 64 and the ceiling half is 64 only after this table reached
    64, with the floor test keeping its `>=`; or move the exact assertion into an integration test
    binary of its own.
    Door: two-way. Effort: XS.

15. Notes beside the crate have drifted.
    Finding: `notes/capabilities/table.md` says `Cap::mint_child` (the type is `Capability`) and
    that `kernel/src/sched.rs` scans for the owning thread (the scheduler split into `sched/`);
    `notes/capability-lifecycle.md` line 14 quotes `derive` returning `NoRights` (it is
    `CannotWiden`) and line 36 quotes a `Cap` type.
    Change: edit the notes to the truth; they stay in nife after the move.
    Door: two-way. Effort: XS.

16. `Option<Capability<O>>` has no niche.
    Finding: `Rights(u32)` uses every bit pattern, so `Option<Capability<u64>>` is 24 bytes for a
    16-byte payload and the kernel's 64-slot table is 2,064 bytes where 1,552 would hold it. Every
    thread's TCB carries one.
    Change: optional. A `Rights` representation with a reserved invalid pattern (a `NonZeroU32`
    holding `bits | 1 << 31`, or an enum) gives `Option` its niche; `from_bits` already masks, so the
    wire format does not change. Measure before and after. The free word and counts do not change.
    Door: two-way now; a layout anyone measures after the release. Effort: M.
