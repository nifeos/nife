---
status: NOT-STARTED
raised: 2026-10-09
promoted_from: narrowing-casts-and-unchecked-arithmetic-on-addresses-are-counted-and-fall
milestone_dependencies: none
decision_dependencies: 61, 268
machine_requirements: none
specific_machine: none
needs_person: no
---
# 895. Narrowing casts and unchecked arithmetic on syscall arguments and addresses are counted, and the count falls

*(Promoted from the proposal pile on 2026-10-12 (UTC) by lane
`maintainer/promote-proposals`; number provisional until the merge queue lands it. Title
and slug are drafts.)*

Raised by a maintainer session on 2026-10-09 (UTC), one of five lint proposals asked for together.
Its numbers are in [notes/lint-census-2026-10-09.md](../../notes/lint-census-2026-10-09.md).
It reuses the lint ratchet specified in
[the panic proposal](0896-panics-in-the-kernel-and-the-parsers-are-counted-and-fall.md); whichever of the
two is promoted first builds it.

Reuse: clippy's `cast_possible_truncation`, `cast_sign_loss`, `cast_possible_wrap` and
`arithmetic_side_effects`. Nothing is written but the ratchet's filter described below.

## The rulings, 2026-10-09 (UTC)

calef ruled on three of this proposal's questions on 2026-10-09 (UTC).

- Open question 1: the `u64`/`i64` to `usize` filter gets a short decision of its own rather than
  stretching §61. It is
  [§268 (the cast ratchet does not count a 64-bit integer cast to `usize`)](../decisions/0268-the-cast-ratchet-does-not-count-a-64-bit-integer-cast-to-usize.md),
  ratified by calef the same day, with a provisional number.
- Open question 2: the scope is the 16 measured files. A file is added only when a concrete path
  from a syscall argument into it is shown.
- Open question 4, and the thresholds: cast counts are frozen per file at today's value and may only
  go down. That is the mechanism. Zero stays the end goal for the three cast lints, with no date
  attached, so done criteria 3 and 4 below state it. `arithmetic_side_effects` is the exception:
  its ceiling only falls, and zero is not its goal (calef, 2026-10-09 (UTC)).

## The problem

An `as` cast that narrows does not fail. It keeps the low bits, so a physical address cut to 32
bits is a plausible wrong address rather than an error. In a kernel, a wrong address is how a
confinement break gets built. Arithmetic is the other half. Release builds keep overflow checks
(calef, 2026-10-04, `notes/overflow-checks.md`), so an overflow halts rather than wraps. That makes
unchecked arithmetic on a caller's value a reachable panic, the panic proposal's class.

Measured over non-test code, the note's scope:

- `as <integer>` appears 3,262 times, 945 of them in the kernel. The request's grep said about
  5,000; the gap is test code.
- Clippy flags 844 `cast_possible_truncation` hits (271 kernel), 155 `cast_possible_wrap` (30),
  185 `cast_sign_loss` (3) and 3,218 `arithmetic_side_effects` (648).
- `kernel/src/syscall.rs` has 7 truncation, 21 wrap, 2 sign-loss and 8 arithmetic hits.
- The memory and page-table code has 36 truncation, 4 wrap and 167 arithmetic hits. That is
  `memory.rs`, `memory_region.rs`, `kmem.rs`, `revoke.rs`, `user.rs`, the three `arch/*/mmu.rs`,
  and the crates `paging`, `page_frames`, `memory_regions` and `address_space_map`.

## What this tree already decided, and why this proposal reopens it

§61 (a lint is adopted on evidence from this tree) dropped `cast_possible_truncation` on
2026-08-03, and `Cargo.toml` records why. Of 497 hits then, 199 were `u64` or `i64` to `usize`.
Those warn about 32-bit pointers, and §19 (architectural parity is a tenet) names three 64-bit
targets only. A gate more than half inapplicable trains a reader to skim.

The ratio holds today: 340 of 844 hits are that class, and 143 of the kernel's 271. Clippy still
has no setting for pointer width (read from `cargo clippy --explain` on this toolchain).

What changed is the instrument. §61's objection was to `-D warnings` over every hit. The lint
ratchet reads clippy's JSON, and each truncation message names both types. So the ratchet can
drop the `u64`/`i64` to `usize` class before it counts. That leaves 504 genuine narrowings in
scope, 128 in the kernel. A reader sees no inapplicable hit. This is a new mechanism beside §61's
ruling, not a reversal of it, and an architect should say so if it is not.

## The mechanism, and its rung

Rung one exists and was considered. Address newtypes (a physical and a virtual address type in
`paging`) would make most address truncations unrepresentable. The tree has none today; `paging`
speaks `u64`. Introducing them touches every mapping call on three architectures, and it is a
design fork of its own. It is open question 3, not this milestone.

So rung two: the lint ratchet, with the filter above, over the scope below. A site leaves the
count by becoming `u32::try_from(x)?`, a checked or explicitly `wrapping_` operation, or a
`const` assertion where the bound is a compile-time fact.

## Scope

Ruled 2026-10-09 (UTC): `kernel/src/syscall.rs`, the memory and page-table files listed above, and
the four crates. That is 16 files with 213 hits after the filter drops 32: 167 arithmetic, 11 truncation,
25 wrap and 2 sign-loss. The rest of the tree is not counted.
Widening it is a reviewed edit to the scope list, and it needs a concrete path from a syscall
argument into the new file. The parsers' arithmetic belongs to the panic
proposal, which shares the overflow boundary.

## Done when

1. The four lints run in the lint ratchet over the scope, with §268's `usize` filter and a
   selftest fixture that proves the filter drops exactly that class.
2. The baseline freezes each of the 16 files at its count on 2026-10-09 (UTC), and a row may only go
   down.
3. `syscall.rs` carries no truncation, wrap or sign-loss hit. Each of its 29 is fixed or carries an
   `#[expect(..., reason = "...")]` saying why the value is in range. Zero is the goal, with no date
   attached.
4. The memory and page-table scope's truncation rows are at zero. Zero is the goal, with no date
   attached.
5. A file joins the scope only with a shown path from a syscall argument into it, in the same
   change that adds its row.
6. The `Cargo.toml` paragraph that records `cast_possible_truncation` as deliberately absent is
   amended to say where the lint now runs and why.

## Open questions for an architect

1. Ruled 2026-10-09 (UTC): a section of its own, §268 (the cast ratchet does not count a 64-bit
   integer cast to `usize`). The question was whether a filtered ratchet is acceptable against
   §61's rule.
2. Ruled 2026-10-09 (UTC): the 16 measured files, and a file is added only when a concrete path
   from a syscall argument is shown. So `sched.rs` and `cap.rs` stay out until one is.
3. Address newtypes in `paging`: worth a milestone of their own? It is the rung-one answer and the
   larger change.
4. Ruled 2026-10-09 (UTC): a ceiling, frozen at today's value per file, that only falls. Zero is not
   the goal for this lint. Checking every `+` in page-table code costs thousands of edits on the
   hottest path, and the cast lints and Kani proofs already cover the overflow that matters. The
   question was whether `arithmetic_side_effects` in the memory scope ratchets to zero.

## Where it sits in the ranking

The customer path is vacant, so the tie breaks toward the fatal risks. This serves risk 7 (the
confinement claim is false), since a truncated address is a confinement break's raw material. It
ranks third of the five, behind the panic proposal (which shares its overflow half) and the
`static mut` proposal, which is smaller and touches risk 5 directly.

## Index row

Narrowing casts and unchecked arithmetic on syscall arguments and addresses are counted by clippy's
cast and arithmetic lints, held per file by the lint ratchet so the count only falls.
