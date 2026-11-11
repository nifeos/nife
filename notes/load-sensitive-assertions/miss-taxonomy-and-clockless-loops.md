# The miss taxonomy and clockless loops: 2026-08-15 and the fourth round, 2026-08-16

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

## `the_handler_keeps_up_when_no_lock_is_held` (aarch64): the taxonomy its comment promised

Resolved 2026-08-15, the day merge-queue runner load made it urgent. The assertion broke three
unrelated pull requests in one afternoon (#204, #210, #215). Parallel queue-group builds saturate
the shared runners, and a descheduled emulator misses deadlines the guest cannot tell from a slow
handler. The comment above the assertion had already written the taxonomy. Re-armed less than one
interval late is a slow handler and our bug. A whole interval or more is the emulator descheduled,
and says nothing about this kernel. The assertion now applies it. The deschedule shape prints its
numbers and passes, loudly labeled; the slow-handler shape still fails. The riscv64 tour has no
direct twin of this assertion (checked when the scheduler smoke line was fixed the same day); if one
grows, it takes the same taxonomy.

*Correction (2026-08-16): that last sentence was wrong, and the twin was a `#[test_case]` rather
than a tour line. See the fourth round below.*

## The fourth round, 2026-08-16: the block's three were already done, and three more were not

This round started from the evidence table of milestone 78 (the load-sensitive assertions). The
table names three assertions that report a negative discrepancy and asks for three arguments. The
first step was to check them against the tree rather than the table, because a diagnosis written
days ago here has usually been overtaken. All three had been:

| the block's three | site | state on 2026-08-16 |
|---|---|---|
| reaper count, `left: 5, right: 6` | `sched.rs`, `a_finished_thread_is_reaped_and_its_memory_returned` | rescoped; per-`ThreadId` `is_thread_present` waits and `used() <= before` |
| address-space frames, "-52" and "-19" | `user/tests.rs`, `a_dead_user_thread_frees_its_whole_address_space` | rescoped, the same two changes |
| frame hygiene, margin of two frames | `user/live_swap_tests.rs` | removed 2026-08-03 in PR #46, with the analysis |

So the block's "the split that makes this two problems" section is history, not a worklist, and its
"What is left" section is the current word. [The first verdicts](global-baselines-and-the-drift-law.md)
already said so. This round adds that a reader coming to the block first will be sent to three
finished sites. That is the kind of stale pointer §71 (a limitation is promoted when it becomes a plan) exists
to catch.

What was left came from reading the same two questions across the files this lane could touch:
`sched.rs` and the two arch timer files, with three other lanes holding the test wiring. Three sites
answered wrong, and one of them was the note contradicting itself on a single page.

### `the_handler_keeps_up_when_no_lock_is_held` (riscv64): the twin the record said did not exist

The 2026-08-15 verdict above ends "the riscv64 tour has no direct twin of this assertion". The old
note's BUGS section said the handler-latency assertions are unfixed on both ISAs. Both cannot be
true, and the BUGS line was the correct one. The twin is in `kernel/src/arch/riscv64/timer.rs`: a
`#[test_case]` in the arch timer module, five tick periods and a bare `assert_eq!` on the
missed-tick delta. It is exactly the assertion that broke #204, #210 and #215 on the other ISA.

What was checked that day was the boot tour. That is a different artifact from the test suite, and
it genuinely has no such line; the conclusion was then written down about the ISA. That is rung four
of the ladder failing in its usual way: a fact living in one sentence, with nothing comparing it to
the code.

The fix is the aarch64 one, ported without variation, which is what rule 5 asks for. A `miss_detail`
module sits behind `#[cfg(test)]`, recording `(now, next, count)` from inside `rearm`: three relaxed
stores in trap context per §9 (locking). A `deadline()`-style accessor sits beside
`missed_ticks()`. The assertion now applies the taxonomy the aarch64 comment wrote. Re-armed less
than one interval late is a slow handler and this kernel's bug, and it still fails. A whole interval
or more is the emulator having been descheduled, and that prints its numbers and passes, labeled.
Nothing about the kernel's sensitivity changed on either ISA. What changed is that one ISA was
carrying a flake the other had already stopped carrying.

### `reclaim_frees_an_embryo_tcbs_region` (`sched.rs`): the reaper count's defect, three tests away

The reaper count's fix was called "the third appearance of this exact fix". It is the fourth, and
the fourth site is in the same file as the third. The test bracketed
`crate::sched::thread_count()` around a retype and a reclaim:

```rust
let threads_before = crate::sched::thread_count();
// ... create the region, retype an unstarted TCB out of it ...
assert_eq!(thread_count(), threads_before + 1, "the embryo should be in the table before reclaim");
// ... reclaim ...
assert_eq!(thread_count(), threads_before, "the TCB's table slot must be freed by reclaim");
```

The headcount is the size of the whole table. A neighboring thread finishing its teardown between
the baseline and the first read lands the count at `threads_before`, one below what the assertion
demands. The run then goes red accusing an embryo that is present and correct. Negative direction, a
global baseline, a claim about one object: it is the milestone's signature. It was unrecorded
against this site only because nothing had happened to fall on it yet.

The rescope is also the stronger claim. `create_tcb` returns a generational `ThreadId`, so
`is_thread_present(tid)` asks the narrow question the test is responsible for ("is this embryo in
the table"), and it is immune to neighbors by construction. The old second assertion could pass
with the embryo still in the table, as long as somebody else's thread left in the same window. The
new one cannot. `is_thread_present`'s own doc comment has argued this since it was written.

The frame half of the same test (`assert_eq!(free_frames(), frames_before)`) was checked against the
same question and left alone deliberately. Its window is a region create and a reclaim with no wait
in it: microseconds of guest execution, not the seconds a `wait_for` spans. The claim "reclaim
returns the region's memory exactly" is the property under test, not a proxy. Loosening it to `>=`
would trade real coverage for an exposure nothing has ever hit. The same reading applies to the other
four `reclaim_frees_*` and `split_returns_*` frame equalities in that file. They were left for the
same reason: they bracket synchronous operations, not waits.

### `kernel_stacks_do_not_touch_the_frame_allocator_in_steady_state` (`sched.rs`): the whole family in three lines

This is the clearest single specimen, and it had all three failure modes at once:

```rust
let baseline = super::thread_count();
for _ in 0..6 {
    super::spawn(|| {}).expect("spawn failed");
    while super::thread_count() > baseline { super::yield_now(); }
}
assert_eq!(memory::stats().unwrap().free(), free_before, "...");
```

1. A global baseline, `thread_count()`, which is the reaper count's defect again.
2. An unbounded yield loop with no clock at all, wrong in both directions. A neighbor reaping first
   puts the count at or below the baseline, and the loop exits immediately, leaving this batch's
   stacks in flight when the frame count is read. A neighbor's thread outliving the batch holds the
   count above the baseline, and the loop never exits. It spins until the harness's 90 s per-test
   ceiling and reports a hang in a test about kernel stacks. Nothing in it was bounded.
3. A global frame equality. The reaper test and the address-space test both traded that for `<=`,
   on the argument that a neighbor's late teardown can only free frames.

All three take fixes already argued elsewhere in this register. Each spawn is followed to its own
reap by `is_thread_present` on the `Tid` it returned, bounded by the module's `wait_for`. The final
assertion becomes `free() >= free_before`, waited on, which is `used() <= before` in the other
units. The defect this test guards spends allocator frames on kernel stacks, driving `free` down and
keeping it there. So a real regression times the wait out and fails with the frame count in the
message. A dead `REAPED` static, stored to and never read, went with it.

### The instrument, and the loop this round ran

The old note's recipe is eight spinners and then the matrix. This round deliberately did not use it.
Five other lanes were gating on the same laptop, and eight spinners would have failed somebody
else's run with exactly the family under study. That is worse than a slower measurement. The machine
supplied its own contention instead: a one-minute load average between 8 and 12 on eight cores for
the whole loop, the condition the recipe manufactures.

Twelve consecutive full `script/test` runs were made on the changed tree, both ISA legs each
(aarch64 and riscv64 kernels under QEMU, plus the host-logic crates). All 12 were green, at a
one-minute load average between 4.2 and 22.2. The heaviest was run 9 at 22.2, which is where the
eight-spinner recipe puts an eight-core machine. The milestone report has the table. The result is
stated here because a flakiness claim with no run count is what this milestone exists to delete.

The honest half of that number: in all twelve runs the taxonomy branch was never taken. Not once, on
either ISA, did a miss land inside the five-period window. So twelve green runs are evidence that
nothing regressed and no evidence at all that the new classification works. That is the trap
[the second round](measurement-windows-and-the-load-recipe.md) recorded ("two clean matrices in a row
proved nothing about them"). So the branch was proven by injection instead:

| probe (riscv64, reverted after) | what it produced | result |
|---|---|---|
| force a real miss, then stamp `miss_detail` with a sub-interval lateness | `late_by` 0, interval 100000 | **red**, "the handler itself is slow, which is this kernel's bug" |
| force a real miss, leave the recorded lateness alone | `late_by` 82830, interval 100000 | **red**, same message |

The second row was not the expected answer. A lock held across two and a half tick periods produces
a lateness between half and one and a half intervals. The handler runs at `lock + 2.5i`, and `next`
is `D + i`, where `D - lock` is somewhere in `(0, i]`. So it straddles the threshold, and this run
landed under it. The taxonomy's cut is at one interval of `now - next`, and `now - next` is already
the lateness beyond the first missed period. So the classification really reads: one to two periods
late is called a slow handler and fails; two or more is called the emulator and passes.

A host deschedule between one and two tick periods is still a red run on both ISAs. The aarch64 fix
narrowed that window rather than closing it, and porting it faithfully carries the same residual.
That belongs in BUGS, not in a wider riscv64 threshold that would break parity with the twin.

### What this round did not do

The icount instrument was still recommended and still not built, for both remaining claims: that SBI
actually fired at the riscv64 software grid's deadlines, and that the handler takes fewer than N
instructions. Both need `-icount shift=0,sleep=off`, which lives in the bench harness rather than
the test suite, and other lanes held the harness files this round. That is the milestone's "What is
left" section, unchanged. (Built 2026-08-17: see [the icount claims](the-icount-claims.md).)

The other 34 sites in the scope note were not audited, again. This round adds a sharper reading
order than "check them against the same question". The three sites here were all found by grepping
the allowed files for a global count taken as a baseline (`thread_count()`, `free_frames()`,
`stats().free()`) and for a loop with no clock in it. Those two greps are cheap, and between them
they caught every site this round changed.
