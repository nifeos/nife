---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 331. A thread is published `Dead` while it is still executing on its own kernel stack

Premise re-checked 2026-09-19 and it holds. `crates/thread_wake_handshake`'s `RunState` has six variants (`Embryo`, `Ready`, `Running`,
`Blocked`, `Finished`, `Dead`) and none of them is `Departing`; `depart()` in `kernel/src/sched.rs` still publishes `Dead`, and the window is still
guarded rather than closed, by `region_reap_verdict(state, on_cpu)` refusing while `on_cpu` is set.
The crate is still the one loom searches.

No decision is owed and nothing else is missing. It touches the death protocol and
`RunState` in `crates/wake_handshake`, where loom searches the transitions, so it wants a lane with
the loom search in its gate rather than a hotfix on somebody's way past.

**In brief.** `depart()` publishes a thread as `Dead` at a moment when the thread is still running
on its own kernel stack. The code that would otherwise free that stack refuses inside the window
instead. Close the window rather than guard it. Mark the thread `Departing` in `depart()`,
and promote it to `Dead` from `finish_switch`. The promotion point already holds `SCHED` and already
runs at the instant the stack goes free. A remover then never observes a thread whose state says the stack is
free while the stack is in use, and the transient `NotPermitted` that milestone 124 records as a
race stops existing rather than being handled.

## Why this matters

The current shape is rung two of AGENTS.md's ladder, and milestone 124 wrote that down about
itself: the `on_cpu` guard is a condition in one function, and *"the next out-of-band remover can
forget the check exactly as this one did"*. It already happened once. A new state makes the wrong
observation unrepresentable, which is rung one. It does so at no ongoing cost because the
promotion point is a function that already runs at exactly the right instant with exactly the right
lock held.

The visible symptom today is a caller getting `NotPermitted` from an operation that is not actually
forbidden, only early. That is a defect with a bad shape. It is timing-dependent, it reports the
wrong reason, and a caller that retries succeeds. That is the pattern that survives review and
then wedges a machine once under load.

`RunState` is loom-searched, so the transition set is the thing under test, and adding a state is a
change loom will check rather than a change loom will miss. That is the argument for doing it in a
lane: the mechanism to prove the new protocol correct is already wired to the crate being changed.

## Index row

`depart()` publishes a thread as `Dead` while it still runs on its own kernel stack, and the code
that would free that stack refuses inside the window. Marking it `Departing` in `depart()` and
promoting it to `Dead` from `finish_switch`, which already holds `SCHED` at the instant the stack
goes free, closes the window. A remover then never sees a state that says the stack is free while it
is in use, and the transient `NotPermitted` that milestone 124 (a thread is born where it lives)
records as a race stops existing. `RunState` is loom-searched, so loom checks the new state.
