---
status: NOT-STARTED
raised: 2026-09-19
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 439. A job-mix sweep goes quiet for a whole subrun, so a wedge timer has to guess

Minted 2026-09-19 by the integrator, from milestone 324's parts 2 and 3
lane, which built the sweep recognizer and found the limitation it could not fix from where it
stood. *(Number provisional until the merge queue lands it.)*

It is a `println!` from a timer in the supervisor, and the marker it prints is
already a shared constant. Nothing here is anybody's decision.

## The problem, and it is a comparison the tool cannot make

`crates/serial_console` can tell a finished job-mix sweep from a wedged one, since milestone 324's
part 2. **What it cannot do is tell a wedged sweep from a slow one**, and the reason is that a sweep
emits nothing between subruns.

A soak does not have this problem. `kernel/src/soak.rs` prints a heartbeat on a wall clock, so the
watcher's quiet timer is sized against *the heartbeat interval*, which is a number the kernel
chooses. A sweep's quiet timer has to be sized against the slowest subrun anyone has seen. That
is a number nobody chooses, which changes with the board, and which moved by half again in one
day. The longest subrun on QEMU measures 4.0 seconds (249,234,771 ticks on a 62.5 MHz counter)
against a 60-second default. The ratio is fifteen to one, picked to be safe rather than right.

That figure is the argument, and this block first stated it wrong. It was minted quoting 2.6
seconds and a ratio of twenty to one, taken from a capture made hours earlier. Milestone 168's lane
had meanwhile taken the sweep from three repeats of a five-kind mix to 21 of a seven-kind one. A
re-capture from the merged kernel on 2026-09-19 gave 4.0 seconds. So a quarter of the watcher's
margin was spent by a change that went nowhere near the watcher. Nobody could have noticed,
because the margin is a constant on one side and an emergent property of the workload on the other.

A board slower than fifteen times QEMU reads as wedged when it is merely slow. There is no
way to tell the two apart from outside. radon is the case that matters, because it is the board the
sweep exists to run on and nobody has watched one there yet.

## What the work is

A heartbeat from the supervisor on a timer rather than only between subruns, the same shape
`kernel/src/soak.rs` already ships. Then `serial_console`'s quiet timer is sized against a chosen
interval instead of against a measured worst case. That is the whole point.

It is a kernel change, which is why milestone 324 (the bench console cannot speak to any of the
three boards)'s lane recorded it rather than taking it. The
supervisor has to print from a timer, and that is outside a milestone about a console tool.

## What it is not

**Not a new exit status.** `serial_console` already reports a quiet run as `2`; what changes is the
threshold's justification, not the vocabulary. Milestone 324's lane deliberately added no status and
this should not either.

**Not a change to the sweep's output format.** The point and subrun lines are output two programs
read. And 2026-09-19 is the day that cost got paid once already. Milestone 168 (a multi-tasking
workload benchmark)'s lane changed those
fields on `main` while milestone 324's lane wrote a parser for the old ones, and every gate stayed
green on both branches. A heartbeat is a new line rather than an edited one for exactly that reason.

## Follow-on

- **Recorded.** *The limitation as it stands today*, in the `BUGS` sections of `script/job-mix`,
  `crates/serial_console/src/progress.rs` and `notes/job-mix.md`, each naming the fifteen-to-one
  ratio and the fact that no sweep has been watched on a board.
- **Milestone 324.** Where the measurement comes from, and where the ratio was corrected from twenty
  to fifteen when the fixture was re-captured against the merged kernel.

## Index row

`crates/serial_console` can tell a finished job-mix sweep from a wedged one since milestone 324. It
cannot tell a wedged one from a slow one, because a sweep prints nothing between subruns. Its quiet timer is
therefore sized against the slowest subrun anyone has measured. That is 4.0 seconds on QEMU against a
60-second default. Fifteen to one is a guess rather than a choice. That ratio was
twenty to one until milestone 168 took the sweep to 21 repeats of a seven-kind mix, which spent a
quarter of the margin without going near the watcher. That is the clearest argument that a threshold
derived from a workload is the wrong shape. `kernel/src/soak.rs`
already solves this by printing a heartbeat on a wall clock, which makes the watcher's threshold a
number the kernel picks. The work is that heartbeat in the supervisor, and it is a kernel change,
which is why the lane that found it recorded it instead. The board it matters for is radon, where no
sweep has been watched at all.
