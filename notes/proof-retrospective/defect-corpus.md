# The defect corpus, row by row

*Name: provisional, minted 2026-10-11 (UTC) by lane/sixth-ten-longest-notes, after the parent's own
heading.*

Six rows of [the corpus table](../proof-retrospective.md#the-defect-corpus) in
[`notes/proof-retrospective.md`](../proof-retrospective.md) deserve more than a cell. This is the
long form of each. Every counterfactual here is a judgment, as it is in the parent.

## 7. The timer re-arm drift, and the one property this study can price exactly

The defect. `kernel/src/arch/aarch64/timer.rs`'s module header says it plainly: *"We shipped this
bug and then measured it."* Re-arming with `TVAL`, a relative countdown, means every tick starts its
countdown late by however long the trap took, and the lateness is never recovered. Measured under
QEMU: 100 Hz configured, about 70 Hz observed. Thirty percent of the kernel's preemptions, gone,
with nothing saying so.

The property. `next_deadline = fired_deadline + interval`, except that a deadline in the
past re-anchors and costs exactly one fire rather than a backlog.

Provable? Yes, and the proof already exists in this tree, over a different subsystem.
`crates/timetable`'s `next_after(prev, period, now)` is the same three arguments and the same law, and
its five harnesses state it: `a_fire_is_strictly_in_the_future`, `a_fire_is_at_least_one_whole_period_on`
(`next >= prev + period`, which is the drift law), and `a_stall_costs_one_fire_and_not_a_backlog`
(`next <= now + period`, which is the re-anchor). Put `rearm`'s twelve lines of arithmetic behind that
function and the milestone-6 defect becomes unrepresentable.

The honest qualification, and it matters. The *original* defect was one register below the
arithmetic: `TVAL` versus `CVAL`, relative versus absolute, with no `next` computed at all. A pure
function proof does not see a register's semantics, so it would not have caught the first form.
It would have caught the second. Milestone 62 (tests that assert on time) injected exactly that second form,
`sbi_set_timer(now + interval())`, with the grid still maintained correctly beside it.
`notes/instruction-clock.md` records what happened: the instruction-count instrument went green with
every number byte-identical to a clean run. That instrument compares each arrival against the
deadline that fired, which a re-anchoring kernel satisfies forever. A `timetable`-shaped proof is the
one form of evidence in this tree that would have gone red on that injection without needing an
emulator.

Judgment, and it is the strongest one in this study: this is a defect a proof would have caught,
the proof is already written, and the only thing missing is that the timer does not call it. That
is the Phase-2 extraction pattern `memory_regions`, `inter_process_communication`, `dma_validator` and `paging::domain` all
took, applied to a subsystem that has not had it yet.

## 5 and 6. The two concurrency defects, and why one tool found one of them

Both are races. Neither is a Kani property as this tree uses Kani, and `notes/verification.md` says
so before either was found, in its list of what a green check does not mean. The parent quotes that
passage whole.

That paragraph is the model for how this project should talk about its proofs, and it was written
before this retrospective existed.

The seqlock defect (#6) is the corpus's one unambiguous green for a formal method, and the method
is loom rather than Kani. The writer claimed the sequence with a compare-exchange and then wrote the
data with nothing ordering the claim ahead of them. Three of four harnesses failed on the first run.
The instructive part is the table of fixes that do not work. `Acquire`, `AcqRel` and `SeqCst` on the
compare-exchange all still fail. An ordering on an RMW orders accesses around itself, and what a
seqlock writer needs is a store-store barrier between its claim and the stores that follow. Nothing
else in the tree could see it: the ten host tests, the kernel clock tests on both ISAs,
`script/verify`, `script/fuzz` and `script/undefined-behavior-check` all passed before the fix and all
pass after it.

What this says about coverage. The class Kani cannot reach is precisely the class loom can, and
this project noticed and built the second tool. Five of the eighteen defects are concurrency and
`crates/` now holds five loom-checked protocols. That is the verification program working as a map
even where it did not work as a net.

## 9, 11 and 12. Three defects in crates that already had harnesses

These are question 3's interesting outcome, and each one fails for a different reason.

`nifefs::write_image` and the NUL name. `nifefs` has two harnesses, both about the *reader*:
`the_validation_implies_reads_slice_is_in_bounds` and `a_short_image_is_refused_not_indexed`. The
crate's reader is proved total with no bound on the image size at all, which is strong. The defect was
in the writer: a name containing a NUL went into a NUL-padded field, so `"a\0b"` was written and read
back as `"a"`, and two names agreeing up to their first NUL collapse into one entry. Nothing panicked,
so no totality proof could have found it. `notes/fuzzing.md` states the reason correctly: *"it is a
property violation, and the property had never been written down."* The property is one line
(`for every accepted name, read(name) returns what write stored`) and it is Kani-shaped. Worklist.

`dtb::Region::end`. Four harnesses, all on `be32`/`be64`. `end()` was `self.start + self.size` on
two `u64`s straight out of the blob, and the kernel calls it on every RAM region the device tree
declares, in a dev-profile build with overflow checks on. A `/memory` node claiming
`start = size = u64::MAX` panics on the boot path; in release it wraps, which is worse. Nobody had
thought to look at `end()`. That is not a limit of the tool, it is a gap in what anyone chose to
state. Worklist.

`gpt::parse`, the best near-miss in the corpus. Nine harnesses, including
`create_never_lays_out_a_table_parse_would_reject`, which sounds exactly like the property the defect
violated. It is not. That harness asserts that whenever `Gpt::create` succeeds, its output satisfies
the four inequalities `parse` checks, and it spells those inequalities out as the code writes them
(`h.last_usable_lba + span < h.alternate_lba`). The defect was a wrong-*accept* in `parse`: `>` where
`>=` was needed at the backup-array boundary, putting one usable block inside the backup entry array
where a partition would overwrite it. `create` makes tight tables, so a create-then-parse harness
never reaches the equality, and a harness that restates the reader's own inequality cannot detect that
the inequality is wrong. `notes/mutation-testing.md` draws the same conclusion from its own side:
*"an exhaustive sweep proves rejection, not rejection for the right reason."*

This is verification.md's failure mode 1, on the record, in a shipped crate: a proof proves what
you asserted, not what you meant. The fix is to state the property absolutely rather than relative
to the writer: *no accepted table places a usable block inside either entry array*. Worklist.

## 13 and 14. Resource accounting, a class nothing here is aimed at

`login::mint()` never freed its own copy of the caretaker's construction-region capability on the
success path. `login`'s cspace is fixed at 16 slots and 8 are spent at rest, so the leak bounded the
service to exactly eight successful logins ever, and a ninth correctly-authenticated login was
answered `DENIED`, indistinguishable from a wrong password.

`login::connect()` had the same shape one level out. It made two rendezvous objects per connect,
retyped from a shared region and released with `cap_delete`. That drops login's reference but leaves
the kernel object in a global 512-slot registry until the region is destroyed. Roughly twenty-nine
test-run connects were enough to fail an unrelated later test with "out of rendezvous points: 512
live at once".

Both are "every path through this function returns the resource to its entry occupancy". That is a
provable property and nothing in this tree proves it for any function. It needs the slot bookkeeping
lifted out of `components/src/login.rs` into a crate, which is a real extraction rather than a
harness. It is also the class most likely to recur: sixteen slots is a small ceiling and every
service program has one. Worklist, and the largest item on it.

## 16. The defect a proof would have got wrong

`pci::place_bars` trusted any nonzero BAR as already placed and already mapped. True on the two
device-tree architectures, where nothing runs before this kernel. False on x86_64's PVH boot, where
QEMU resets `-device nvme`'s BAR0 to a live address of its own choosing, unrelated to the window
`mmu::map_everything` actually mapped, and the first register touch page-faults.

`crates/pci` has five harnesses, one of which is `ecam_offset_stays_inside_the_window`. A sixth
stating "a BAR this function accepts is inside the mapped window" would have been written, in
2026-07, against the predicate the code used, because that predicate was believed. A proof would
have encoded the bug. The thing that found it was attaching a real device to a real boot path, which
is the ranking function at the top of `AGENTS.md` doing its job.

Recorded because a study of proofs that only lists cases where a proof would have helped is
propaganda.

## 17 and 18. The non-code defects, which fail the same way one level out

Nine milestones (81, 82, 85, 94, 96, 100, 101, 109, 110, 113) had merged implementation pull requests
while both roadmap records read `NOT-STARTED`. `script/roadmap --check` was green throughout, because
it compares two records and the tree is a third record nothing compares against. A developer spent
its entire budget re-running mutation tests for milestone 85, which had merged hours earlier.

A block quote in `design/roadmap/0074-cycle-counters.md`, attributed to `notes/benchmarks.md`, quoted
arithmetic that milestone 101 had re-measured and retracted. The roadmap was citing a retraction as
the current record and nothing could see it, because a prose block quote attributed to another file is
a citation no gate resolved. `script/citations` exists because of it.

The shape is identical to the code cases and is worth naming as one thing: a checker compares two
artifacts and the truth lives in a third. `parse` against `create`, a roadmap file against a README,
a quote against nothing at all. The generalization for the proofs is that a harness written against
the code's own predicate is that shape exactly.
