# P1: before the tag

An appendix to [the milestone](../1084-current-cpu-contract-outside-review.md).

## P1: before the first tag

5. `cpu()` hands back an id the documented consumer cannot use. DONE (Decision A, ruled 2026-11-12 UTC).
   Finding: `cpu()` answers `Some(word as usize)` for every word other than `UNSCHEDULED`, with no
   comparison against `CPU_ID_BOUND`. Every documented consumer indexes a `[_; CPU_ID_BOUND]` by
   the answer (the doc example at lines 85 to 96, `user_mode_runtime::current_cpu`'s example). A
   kernel built against a copy of this crate with a larger bound shares the magic with a program
   built against this one, because the layout does not change, so the program's index is out of
   bounds and it panics. Inside nife the kernel's `MAXIMUM_CPUS` is this constant and the system
   test carries the bound back across the boundary, so no nife build can reach it today. Once the crate leaves and the two sides pin different versions, nothing checks it.
   Evidence: `src/lib.rs` lines 277 to 282, 75 to 98; `kernel/src/cpu.rs` line 28;
   `system_tests/src/user/current_cpu_tests.rs` lines 62 to 70.
   Change: `cpu()` answers `None` when the word is at or past `CPU_ID_BOUND as u64` (one more
   compare on a path that already compares twice), with the doc sentence "an id this crate's bound
   cannot index reads as unknown" and a test `an_id_past_the_bound_reads_as_unknown`. The doc of
   `CPU_ID_BOUND` states that a change to its value is a wire break that re-mints `MAGIC` and bumps
   `VERSION` (Decision A).
   Door: the compare is the restrictive direction and two-way; whether the bound is part of the
   identity is one-way (Decision A). Effort: XS.

6. `allowance()` truncates on a narrower target.
   Finding: `NonZeroUsize::new(self.word(OFF_ALLOWANCE)? as usize)`. On a target whose `usize` is
   narrower than 64 bits the cast drops the high half, so an allowance of `0x1_0000_0000` reads as
   `None` and `0x1_0000_0004` reads as 4. No target the crate names suffers from it. It is a hole in the claim, not a defect a current user can reach. `tick_hertz()` answers `NonZeroU64` and has no
   such hole, so the two readers also disagree in type.
   Evidence: `src/lib.rs` lines 255 to 257 and 269 to 271.
   Change: `usize::try_from(word).ok().and_then(NonZeroUsize::new)`, so an allowance the target
   cannot hold reads as unknown on every target; one sentence in the doc. The return type stays,
   since `available_parallelism` wants a `NonZeroUsize`.
   Door: two-way (no behavior changes on any supported target). Effort: XS.

7. The reader reaches its word through an integer, so Miri checks less than it could.
   Finding: `word()` computes `(self.base as u64 + off as u64) as *const AtomicU64`, and the comment says the author chose the shape so clippy need not trust a pointer cast. Miri reports the
   integer-to-pointer cast on every such load (verified in the experiment above) and falls back to
   wildcard provenance, so the weekly undefined-behavior job cannot tie the load to the buffer it
   came from. `publish` must start from an integer, since its API takes a `u64`, but `word()` holds
   a pointer and need not. The two sites also differ for no reason: `publish` uses `wrapping_add`
   with a paragraph on why, `word()` uses `+`.
   Evidence: `src/lib.rs` lines 185 and 244 to 248.
   Change: `self.base.add(off).cast::<AtomicU64>()` in `word()`, with the alignment argued in the
   `SAFETY` comment as it is now (the base is 8-aligned by `new`'s contract and every offset is a
   multiple of 8), and a `#[allow(clippy::cast_ptr_alignment)]` only if clippy still objects. Keep
   `publish`'s integer form and its reason.
   Door: two-way. Effort: XS.

8. The two `unsafe` entry points check nothing in debug builds.
   Finding: `publish(page_va, UNSCHEDULED)` silently un-publishes a thread. `publish` with a misaligned address and `new` with one have undefined behavior with no debug-time report. The kernel's two call sites pass `cpu::id()`, which is in range by construction, but the crate
   cannot know that.
   Evidence: `src/lib.rs` lines 179 to 187 and 228 to 232.
   Change: `debug_assert!(page_va % 8 == 0)`, `debug_assert!(cpu != UNSCHEDULED)` and
   `debug_assert!(cpu < CPU_ID_BOUND as u64)` in `publish`; `debug_assert!(va % 8 == 0)` in `new`
   (as a `const unsafe fn` it can carry a `debug_assert!` on a `u64`). Release builds keep the
   single store.
   Door: two-way. Effort: XS.

9. The builder and the readers disagree about what "unknown" is.
   Finding: `build_page_allowing(allowance: u64, tick_hertz: u64)` takes two plain words where zero
   means unknown; the readers answer `Option<NonZero…>`. `build_page()` is `build_page_allowing(0,
   0)`. The name says one of its two parameters and reads as if it permits something. Two kernel
   call sites and the tests call it.
   Evidence: `src/lib.rs` lines 143 to 166; `kernel/src/user/address_space.rs` line 159;
   `kernel/src/user/address_space_registry.rs` line 300.
   Change: `build_page_with(allowance: Option<NonZeroU64>, tick_hertz:
   Option<NonZeroU64>)`, with `build_page()` kept as the all-unknown page. Decision B decides the
   name, provisionally; the parameter types are the maintainer's.
   Door: two-way before the first tag. Effort: S, including the two kernel call sites.

10. Nothing is machine-checked, and the template expects a proof or a stated reason.
    Finding: no `#[kani::proof]` and no `falsifications/`. The three sibling contract crates reviewed so far (`socket_contract`, `block_roster_contract`, `entropy_contract`) each carry a
    harness and a patch; `counter_frequency_contract` carries none and its package left the question
    open. The template's `proofs / falsify` job reads Kani's summary line; nobody has verified what it does with zero harnesses.
    Evidence: `crates/current_cpu_contract/` (no `falsifications/`, no `kani` in `src/`); the
    template's `helpers/falsify.py` line 42.
    Change: two harnesses with their patches. First, for any 32 bytes, `cpu()` is `Some` only when
    the first eight bytes equal `MAGIC`, the word is not `UNSCHEDULED` and the word is under
    `CPU_ID_BOUND`. Likewise `allowance()` and `tick_hertz()` are `Some` only when the magic matches and the
    word is nonzero. Second, for any `allowance` and `tick_hertz`, the page `build_page_with` makes
    reads them back and reads `cpu()` as `None`. Each harness is small enough for stock Kani in
    seconds (unverified). Alternatively, the README's `## What is proven` states that nothing is,
    and why, as the sibling's does.
    Door: two-way. Effort: S.

11. The crate's `BUGS` section is stale and its claims have moved.
    Finding: "Nothing outside tests reads the word yet" is false: `user_mode_runtime::current_cpu`,
    the standard library's `available_parallelism` (through the copied source) and the kernel's
    tests read it. The timing figure names a machine and a pull request. The Miri entry describes a
    test-side workaround as a crate bug.
    Evidence: `src/lib.rs` lines 12 to 18.
    Change: a `BUGS` list a stranger can act on. An address space with no thread has no page and a reader faults. A process has at most `THREAD_PAGE_SLOTS` threads. `CPU_ID_BOUND` sizes and does not enumerate. The `&AtomicU64` over a read-only mapping is a shape the Rust memory model has
    not settled, and the fallback is a raw atomic load that forms no reference.
    Door: two-way. Effort: XS, inside item 3's rewrite.

12. The standard library copies this file by name, and a stranger cannot see that.
    Finding: `xtask/src/farm/copied.rs` copies `crates/current_cpu_contract/src/lib.rs` into the
    standard library's platform layer and rewrites `address_space_map::` to a sibling module. A
    second source file, a `mod`, or an `include!` in this crate would break that copy with no
    signal inside the crate's own repository. After the move the farm takes the copy from the pinned package's directory (`cargo metadata`), so the constraint survives the move.
    Evidence: `xtask/src/farm/copied.rs` lines 125 to 145; `xtask/src/farm/generate.rs` lines 61
    and 110.
    Change: one sentence in the README's `## BUGS` ("a consumer copies this crate's single source
    file into a standard library; keep the crate to one file with fully qualified paths"), and, in
    nife, a check in the farm that refuses a copied package with more than one source file. The
    second half is a nife lane.
    Door: two-way. Effort: XS.
