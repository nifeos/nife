# P1: before the tag

An appendix to [the milestone](../1099-machine-statistics-contract-outside-review.md).

## P1: before the first tag

4. The accessor totals are not total functions, and a reader panics on a page it accepted.
   Finding: `Snapshot::sum` folds with `Iterator::sum`, which panics on overflow in a debug build
   and wraps in a release build. `from_words` admits any page with the magic and the version. A page whose counters are large (a writer other than this kernel, or a corrupted frame) therefore makes `busy_ticks`, `idle_ticks`, `context_switches`, `interrupts` and `runnable` panic in a reader that validated its input. `total_bytes` and `free_bytes` already saturate, so the crate is
   inconsistent with itself. Verified with a scratch test: every counter at `u64::MAX` under a
   valid header, `busy_ticks()` panics with "attempt to add with overflow". With eight lines the
   sum of eight words cannot overflow unless each exceeds 2^61, which no honest kernel reaches, so
   this is a hole in the claim, not a defect a user of this kernel can reach.
   Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 270 to 272 and 260 to 268.
   Change: fold with `u64::saturating_add`, one unit test over a page of `u64::MAX` counters that
   pins every accessor to `u64::MAX`, and the harness of item 7 that states every accessor is total.
   Door: two-way. Effort: S.

5. `counter` returns a `'static` reference under a contract that promises less, and does not say
   what a reader may do with it.
   Finding: the safety section says the buffer must be "live for the call", and the function
   returns `&'static AtomicU64`, so a caller who keeps the contract can still hold the reference
   after the mapping is gone. The int-to-pointer cast `page_va as *const AtomicU64` has no
   provenance, which Miri warns about and which a `*const u64` or `NonNull<u64>` parameter would
   give it. The function is public and generic, and nothing says that a reader holding a read-only mapping may only load. The standard library states that every atomic access to read-only memory is undefined behavior except a `Relaxed` load of at most eight bytes on the 64-bit targets this crate builds for. That is exactly the shape `Snapshot::read` uses without saying why. On a
   target with a pointer narrower than 64 bits the cast truncates the address silently; every
   target the crate names is 64-bit, so this is a hole in the claim.
   Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 134 to 145 and 242 to 256;
   `core::sync::atomic`, "Atomic accesses to read-only memory".
   Change: make the lifetime the caller's (`unsafe fn counter<'a>(...) -> &'a AtomicU64`, or `AtomicU64::from_ptr`, which is std's spelling of the same operation and carries the same contract). Take the address as a pointer, or keep `u64` with a sentence that it must be representable as a pointer plus a `const` assertion that the fixed address fits `usize`. Write two sentences at `read`. The loads are `Relaxed` because the mapping is read-only and std guarantees only that shape there. A reader must not store or read-modify-write through `counter`. The kernel's writer in this tree does not use `counter`; its only caller in the tree
   is `read`.
   Door: two-way (the signature changes before any release). Effort: S.

6. The layout's line count belongs to another crate, and a change there moves this page's layout
   without a version bump here.
   Finding: `WORDS = word::LINE * (1 + CPU_ID_BOUND)` with `CPU_ID_BOUND` re-exported from
   `current_cpu_contract`. A release of that crate raising the bound changes this crate's `WORDS`
   and `PAGE_CONTENT_BYTES` at the next build while `MAGIC`, `VERSION` and `INTERFACE_ID` stay.
   A reader built against eight lines then misses cores a sixteen-line kernel writes, and both
   agree on the identity. The kernel's `MAXIMUM_CPUS` is the same constant, so inside this tree the
   two cannot drift, but across two pinned repositories they can.
   Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 27 and 51;
   `kernel/src/cpu.rs` line 28.
   Change: this is Decision 1. Under the recommendation, the crate carries its own
   `CPU_LINES: usize = 8` (name provisional) and a `const` assertion that it equals
   `current_cpu_contract::CPU_ID_BOUND`, so a bound change fails this crate's build and forces a
   version decision here instead of moving the layout silently. The re-export stays for callers
   that size arrays by it. One sentence at `VERSION` says the line count is part of what the
   version names.
   Door: one-way (layout). Effort: S.

7. Nothing is machine-checked, and the README's "What is proven" section will have nothing to say.
   Finding: no `#[kani::proof]`, no `falsifications/` directory. `cargo kani` exits 0 with "No
   proof harnesses were found", so the template's `proofs / kani` and `proofs / falsify` jobs pass
   without proving anything; the review verified this on the copy, which the sibling reviews had
   left open. The reader's two decisions, the magic compare and the version compare, and the
   totality of every accessor are small enough for Kani to close over every page.
   Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 211 to 240 and 258 to 308;
   `crates/block_roster_contract/src/lib.rs` lines 484 to 560 (the pattern).
   Change: a `verification` module behind `cfg(kani)` with three harnesses, each with a
   falsification patch the template replays red. The first takes `from_words` over arbitrary words
   and states `None` exactly when the magic word or the version word differs, and otherwise that
   every field equals its word and `online` is the nonzero test. The second states that every
   accessor of a `Snapshot` built from arbitrary words returns without panicking (after item 4).
   The third states that `build_header(f, h)` followed by zeroes reads back as a page with
   `frame_bytes == f`, `tick_hertz == h`, no online cpu and every total zero. The README's "What is
   proven" section then names them and says that no proof covers the caller's mapping promise.
   Door: two-way. Effort: M.

8. The two `unsafe` functions have no host test, so Miri never sees them.
   Finding: coverage places the only uncovered lines in `counter` and `Snapshot::read`. The
   host tests reach the reader libraries and the kernel through `from_words` and `build_header` on the host
   and reach `read` only under QEMU. A host test over an aligned frame is cheap, passes under
   Miri (the review ran one), and is the only place the `&AtomicU64` shape over a buffer gets
   checked outside a running kernel.
   Evidence: cargo-llvm-cov, lines 142 to 145 and 249 to 256 uncovered.
   Change: one test with a `#[repr(align(4096))]` frame of `WORDS` words that writes a header and one cpu line and reads it back through `read`. The test reaches the frame through `as_mut_ptr` so Miri accepts the atomic shape. A second test reads an unprepared frame as `None`.
   Door: two-way. Effort: S.

9. The `word` module mixes absolute header indices with per-line offsets under one namespace.
   Finding: `word::MAGIC` through `word::VERSION` index the page; `word::ONLINE` through
   `word::RUNNABLE` are offsets into a cpu line, added to `word::cpu(id)`. Both are plain
   `usize` constants in one module, and `word::ONLINE == word::MAGIC == 0`, so `words[word::ONLINE]`
   compiles and reads the magic. The doc comment on the module says the rule; the types do not.
   Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 79 to 120.
   Change: two submodules (`word::header` for the absolute indices and `word::line` for the
   offsets, names provisional) or an `_OFFSET` suffix on the per-cpu constants; a typed index is
   heavier than the crate needs. `LINE` (words per line) is a generic word under the naming rules
   and reads better as `LINE_WORDS` (provisional). The new names join item 1's table.
   Door: two-way before the move; the names are one-way after it, which is why they go in the
   ratification table. Effort: S.

10. One API inconsistency a stranger meets at once.
    Finding: `online_cpus` returns `usize` while every other accessor returns `u64`, so `top`
    writes `m.online_cpus() as u64`. The `Cpu` type and the `cpus` field use the scheduler's word, while the prose says "core" in the module documentation, on `Cpu`, on `online_cpus` and in every accessor's summary. The vocabulary rule keeps `cpu` for the unit the scheduler runs a thread on and `core` for the physical core. The page counts the former.
    Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 4, 147, 274 to 308;
    `crates/top/src/lib.rs` line 108.
    Change: keep `usize` (a count that indexes an array is std's `usize`) and say so in one
    sentence, or return `u64` like its neighbors; the maintainer decides. Say "cpu" where the
    prose means the scheduler's unit.
    Door: two-way (before the move). Effort: S.

11. Documentation a stranger reads, beyond the citations of item 3.
    Finding, six parts. (a) The BUGS line reads "`vmstat` prints `microseconds` and `sy` as one":
    a unit sweep rewrote the `us` column name (user time) as a unit symbol; the design note still
    says `us`. (b) Words 6 and 7 of the header line and of every cpu line are reserved and read zero. Nothing says so or says what a later writer may put there. `current_cpu_contract` states its growth rule and this crate states none. (c) `TICK_HERTZ` has no reader in the tree
    (`vmstat` divides by uptime instead), and nothing says what it is for. (d) `read` does not say
    why its loads are `Relaxed` (item 5). (e) `passive_voice` counts six passives in 2,153 words,
    two over the template head's allowance; `prose_acronyms` refuses `PR` at line 34. (f) The BUGS
    section describes one kernel: how its tick samples the run queue, that its switch count is a
    tick old, that its filesystem is a program. A contract's BUGS section says what the contract
    cannot express.
    Evidence: `crates/machine_statistics_contract/src/lib.rs` lines 11 to 17, 34, 80 to 120, 88
    to 89; `notes/machine-statistics.md` "Limits".
    Change: write `us`. Document the reserved words and the growth rule Decision 1 settles. Say `TICK_HERTZ` lets a reader turn tick counts into time without a clock. Add two sentences at `read`. Put two sentences into the active voice and expand "pull request". Rewrite BUGS as the contract's limits (no consistency across words, no user and system split, a sample's age
    bounded by the writer's tick and not by the page, a fixed line count).
    Door: two-way. Effort: S.
