---
status: BUILT
raised: 2026-08-30
built: 2026-08-30
---
# 193. Put `kernel/src` within reach of the prover, because today the proofs cannot see it

Minted the same day by calef, from milestone 191's (did the proofs
catch the bugs?) finding, and built in PR #591; notes/kernel-proofs.md is the record. *(Number
provisional until the merge queue lands it.)*

## What was built, and it met the bar as stated

Two properties, over code that lives in `kernel/src/syscall.rs` today, proved by harnesses
`script/verify` runs. Nothing was moved into a crate first, which was the bar precisely so this
milestone could not be satisfied by doing the other option under this name.

- `the_run_end_is_exact_and_refuses_exactly_what_does_not_fit`: for every `va` and every
  `count >= 1`, `run_end_va`'s `Some(last)` is exactly `va + (count-1)*PAGE_SIZE` with no wrap, and
  `None` is exactly a run that does not fit in a `u64`. Stated in `u128` so the harness cannot repeat
  the implementation back at itself.
- `every_page_between_the_checked_ends_is_itself_a_user_page`: both mapping paths check the two ends
  and then walk `va + k*PAGE_SIZE` with nothing re-checking the middle. `k` is symbolic, so every
  page of every run is covered with no unwind bound.

Both were falsified before they were believed, which is the point given milestone 191's finding
that no harness in this tree had ever caught a defect. Re-introducing milestone 142's real
`(count - 1).wrapping_mul(PAGE_SIZE)` defect turns both red. **That is the counterfactual milestone
191 said the tree did not have**, and it arrived hours after the study that named its absence.

Five stoppers, four fixed by a `cfg`. This block predicted three from `cargo check`; `cargo kani`
found two more, a duplicate `panic_impl` lang item (Kani links `std`) and global asm, handled by
`#[cfg(not(kani))]` and `--ignore-global-asm` in `script/verify`'s `kernel` case only. The list is
short because DECISIONS §4 rule 1 held: three `asm!` sites outside `arch/`.

The x86_64-Linux worry below was a false premise. Every job in `verify.yml` runs on
`ubuntu-24.04-arm`, so `aarch64-cpu` compiles and the ELF section names were only ever rejected by
the Mach-O host. The real constraint is its mirror image and is now recorded: the `kernel` row
needs an aarch64 host, and if that runner label changes the row breaks.

Cost: about 10 seconds on `script/verify`'s ~650, almost all compile rather than solver.

A note on how this landed, because a gate caught it. The status was flipped to `BUILT` on the
records pull request that minted this block, one merge ahead of the work itself. `script/lint`
refused the work's own branch for changing nothing here, and it was right to. §90 (the status flip
is a gate)'s rule is that a
milestone branch lands its status flip in the same merge as the work, precisely so `main` never
claims something is built while its code is still on a branch. For about an hour, `main` did. The
ordering was the integrator's mistake rather than the lane's.

A second hole closed on the way. `crates/jh7110_entropy` carried three harnesses and appeared
nowhere in `script/verify`. They were run first (all pass), then the row was added, and `script/lint`
grew the check that makes it the last time: every crate with proof harnesses is in the verify table,
derived from `cargo metadata`. The packer already asserted table to shards; this is the missing tree
to table.

**What it did not do**, and the BUGS below still stand: `kernel/src/arch/` remains unreachable, and
so do `user/` and `xtask`, for exactly the reason the kernel was.

In brief. Milestone 191 asked whether the Kani harnesses had ever caught a defect and found that
none has, after the day it was written. The cause is not the harnesses. It is one line in
`script/verify`'s own header:

> `cargo kani -p <crate>` never compiles the kernel, the user programs, or xtask.

So 64,818 lines of `kernel/src` are out of reach by construction, and that is exactly where every
concurrency, hardware-contract and resource-accounting defect in the corpus lived. DECISIONS §14
(a verified-Rust capability microkernel that runs real workloads) promises a verified core. Today the verified part is the pure crates and the
core is not in it.

## The distance is much shorter than anyone assumed, and this is measured

Run on 2026-08-30, `cargo check -p kernel --target aarch64-apple-darwin`, which is the host on the
development machine. Three errors, all shallow:

1. `unwinding panics are not supported without std`, which is a profile setting (`panic = "abort"`).
2. `invalid Mach-O section specifier` at `STACKS` in `kernel/src/interrupt_stack.rs`, an
   `#[unsafe(link_section = ".interrupt_stacks")]` on a static.
3. The same at `SECONDARY_STACKS` in `kernel/src/smp.rs`, `.secondary_stacks`.

That is the whole list. The kernel very nearly compiles for a host target already, and the
reason is DECISIONS §4 (kernel shape: monolithic, deferred) rule 1 working exactly as designed. `asm!` appears outside
`kernel/src/arch/` in only two files and three sites (`cpu.rs` once, `user.rs` twice).
`kernel/src/arch/` is 14,744 of the 64,818 lines, leaving roughly 50,000 lines of ordinary Rust.

**Take that as an encouraging measurement and not as a schedule.** `cargo check` is not `cargo
build`, neither is `cargo kani`, and the three errors above are the ones that surface first rather
than the ones that are hardest.

## What is actually going to be hard, named rather than discovered

- **The host in CI is not the host here.** This machine is aarch64, so the `aarch64-cpu` dependency
  compiled. CI is x86_64 Linux and it very likely will not, which probably forces that dependency
  behind a target `cfg`. Check this early; it can change the shape of everything else.
- Kani cannot model `asm!`. Three sites outside `arch/` and all of `arch/` are unreachable to
  the prover no matter what compiles. The answer is a stub boundary, not a rewrite, and where that
  boundary sits is the interesting design question in this milestone.
- MMIO and fixed addresses. A raw pointer to a device register is not something CBMC can reason
  about, and the same stub argument applies.
- **CBMC does not scale to 50,000 lines wholesale.** Nobody should imagine a proof over the kernel.
  The deliverable is *reachability*: harnesses can be written against the kernel's own code, one
  property at a time, the way they already are for the crates.
- `script/verify` already costs about 42 minutes. Anything added here lands on that budget, and
  the `--affected-since` machinery exists precisely because it is expensive.

## The fork, which the measurement should decide rather than precede

Option A: make the `kernel` crate itself Kani-reachable, with the arch layer stubbed. Direct,
and it puts the prover where the defects are.

Option B: keep lifting pure logic into host-compilable crates, which is what AGENTS.md already
prescribes (*"Pure logic ... belongs in crates that compile for the host"*) and what the tree has
been doing incrementally. Milestone 191's own worklist has an instance: lifting `login`'s cspace
bookkeeping into a crate answers two recorded leaks.

**These are not exclusive and the honest answer is probably both**, with the split decided by where
a property naturally lives. A scheduling policy wants to be a crate. `syscall.rs`'s dispatch does
not, and never will be one.

## The proof that this milestone worked

One property, over code that lives in `kernel/src` today, proved by a harness `script/verify`
runs. Not a survey, not a plan, and not a proof of something that was moved into a crate first,
because that would be option B wearing this milestone's name.

The candidate worth trying first is milestone 191's sharpest counterfactual: the milestone 6 timer
re-arm drift (100 Hz configured, ~70 Hz delivered) has its property already proved in
`crates/timetable`'s `next_after`, over already-written code, and the timer does not call it. That
is a case where the property, the proof and the defect all already exist and only the reachability
is missing.

## BUGS

- **This block does not price the work**, and the three-error measurement above is the kind of
  number that invites a bad estimate. What was measured is that the front door opens, not that the
  building is empty.
- It says nothing about the user programs or `xtask`, which the same `script/verify` line puts
  out of reach for the same reason. `user/` holds real parsers over untrusted input and has at least
  as good a claim on the prover as the kernel does.
- A stub is a hole in a proof, and a proof with an unexamined stub is worse than no proof,
  because it reads as coverage. Whatever boundary this lands on needs the stubs enumerated where a
  reader meets the harness.
- `kernel/src/arch/` stays unreachable under every option here, which means the architecture
  layer is not what this fixes. Saying so plainly is what keeps this milestone from being quoted as
  more than it is.

## Follow-on

- **Milestone 197.** `user/` and `xtask`, which this block's BUGS names as out of reach for exactly
  the reason the kernel was. The `user/` half is built; `xtask` was argued there and refused, with
  the shape that would reverse the refusal named.
- **Milestone 197.** The timer re-arm seam. This block nominates the milestone 6 (threads, the context
  switch, and preemption) drift as the
  sharpest candidate (the property is already proved in `crates/timetable`'s `next_after` and the
  timer does not call it). This lane could not use it, because `rearm` lives in
  `kernel/src/arch/*/timer.rs` and reads the counter through `asm!`. Milestone 197 (`user/` and
  `xtask` are out of reach of the prover) carries it as a fork rather
  than as work. Where the seam goes is the design question: too high and the arch layer keeps the
  bug, too low and every ISA restates it.
- **Recorded.** `notes/kernel-proofs.md`. `kernel/src/arch/` is unreachable and will stay
  unreachable, so nothing here covers the architecture layer, which is where the VisionFive 2's
  undelivered-wake defect actually lived. The stub boundary is enumerated in that note, under "The
  stub boundary, enumerated", because a proof with an unexamined stub reads as coverage.
- **Recorded.** `notes/kernel-proofs.md`. The `kernel` row needs an aarch64 host and nothing
  enforces it: `kernel/Cargo.toml` depends on `aarch64-cpu` unconditionally, and the row works only
  because every job in `verify.yml` runs on `ubuntu-24.04-arm`. A change to that runner label breaks
  the row, and the fix if it ever happens is putting the dependency behind a target `cfg`.

## Index row

From milestone 191 (did the proofs catch the bugs?)'s finding: no Kani harness has ever caught a
defect after the day it was written, because `script/verify`'s own header says `cargo kani` never
compiles the kernel. 64,818 lines of `kernel/src` are out of reach by construction, and that is
where every concurrency, hardware-contract and resource-accounting defect lived. Measured the same
day: `cargo check -p kernel` against a host target fails with three shallow errors (a `panic =
"abort"` profile setting and two ELF-style `link_section` names Mach-O rejects), because §4 rule 1
held and `asm!` appears outside `arch/` at only three sites. The deliverable is one property
proved over code that lives in `kernel/src` today.
