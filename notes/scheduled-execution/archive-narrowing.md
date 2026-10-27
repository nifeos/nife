# Narrowing the archive: where the program set is computed

*Name: provisional, minted 2026-10-11 (UTC) by lane/final-longest-notes, for the parent's "Where the
set is computed, and why not at the spawn site". calef names files.*

An appendix of [scheduled execution](../scheduled-execution.md), for its section "The archive it
holds is narrowed to the plan", under §212 (a prose budget). The
parent keeps what is narrowed and why. This file keeps why the set is not computed at the spawn
site, how a written list is held equal to the plan, and the audit line that measures the result.

## Where the set is computed, and why not at the spawn site

The obvious shape is for the spawn site to register the document itself and read `programs()` off
the result. The kernel cannot do that, and the reason is worth knowing before anyone tries again.

`timetable::Registry` is a `[Row; MAX_ENTRIES]`. A `Row` carries an `Admission`, which carries a
kilobyte of `grant_plan::Endowment`. So `Registry::register` compiles to a 21632-byte stack frame,
and the `grant_plan::plan` underneath it to a further 12048.

In `components/src/timetable.rs` those numbers are fine and stated. It is a process with a 32-page
stack, and `system_tests/src/user/timetable_tests.rs` says why it maps 32 pages. In the kernel they
are exactly what `script/stack-frame-check` refuses. A frame larger than the 4096-byte guard page
can move `sp` from inside a thread's stack to below the guard in one step, touching nothing in
between. The guard never faults, and the write lands in the neighboring thread's stack. This tree
measured that on riscv64 on 2026-08-14, at 4088 bytes below the bottom of a 4096-byte guard. Eight
more bytes would have produced no fault at all. See notes/stack-high-water.md.

Neither number is a bug in `timetable`. A plan you can print is a plan you have to hold. Holding one
entry's worth of authority costs a kilobyte because that is what the authority *is*. What it means
is that the plan is computed where there is stack for it, which is the host and the program, and
never in the kernel.

## A written list, held equal to the plan by a test

So the spawn site carries the program set as a written list, `PLANNED_PROGRAMS`. What keeps it
equal to the plan is a host test rather than a comment.
`the_archive_a_timetable_holds_is_measured_against_what_it_will_build` registers the same
`components/timetable.conf` against the same `timetable::SHIPPED_HELD`. It asserts the plan is
exactly that set, by name. Editing the document without editing the list fails in milliseconds
with no emulator. The program's own audit line then fails the cross-ISA test as well, so a wrong
list goes red twice.

That is rung two of AGENTS.md's ladder where the first draft of this reached for rung one. The
demotion is forced rather than chosen: the rung-one version does not fit on a kernel stack.

## The audit line

A process cannot narrow its own endowment, so what `timetable` does instead is measure it.
`timetable::Audit` compares the names the archive carries against the plan's program set. It prints
one of two sentences after the plan:

```text
timetable: the archive it holds carries exactly the 2 programs its plan names
timetable: the archive it holds carries 57 programs, 55 of them beyond its plan
```

The second one is what the shipped program printed before this landed. Keeping both is the point
rather than politeness: the width of an endowment should be a line on the console rather than a
fact only the spawn site knows. `system_tests/src/user/timetable_tests.rs` asserts the first
sentence and asserts the second never appears. So a spawn site that quietly went back to handing
over the initrd fails a test rather than passing one it no longer earns.
