---
status: BUILT
built: 2026-10-03
raised: 2026-10-03
promoted_from: the-sibling-recv-cap-paths-get-a-receiver-first-test
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 714. The sibling RECEIVE_CAP paths get a receiver-first test

*Title and slug are drafts.*
Raised by milestone 634 (a plain SEND received by RECEIVE_CAP never hands the receiver a
sender-chosen slot), which fixed the plain-SEND leak and left two sibling paths reasoned from the
code rather than measured.

## The finding

Milestone 634 made `RECV_CAP` return `NO_CAP` in `x1` unless a capability was installed for the
delivery, through a `Thread::cap_delivered` default in `kernel/src/sched.rs`. Two other receive
paths flow through the same default and so are now also correct on the receiver-first order. One is
an interrupt signal's `x1` (which was `0` on that order before, `NO_CAP` on the other). The other
is a death message's `x1` (which carried the dead thread's id on that order). Neither has a test
that drives it through the receiver-first rendezvous order. The guarantee rests on reading the
code, the grade `notes/confinement-claims.md` asks a reader to apply to an unmeasured verdict.

## What a test would do

The shape is milestone 634's `a_plain_send_to_recv_cap_delivers_no_cap_whichever_side_parks_first`,
one per sibling: a receiver parks in `RECV_CAP`, the signal or the death is delivered second, and
the test asserts `x1 == NO_CAP`. It should fail on the receiver-first order against a tree with the
`cap_delivered` default reverted (the milestone 634 falsification is the starting point) and pass
with it. That holds on all three ISAs (DECISIONS §19 (architectural parity is a tenet)).

## What is built

Two tests in `system_tests/src/user/recv_cap_attack_tests.rs`, each driving both arrival orders so the
receiver-first red is read against a green control (the shape of milestone 634's test):

- `an_interrupt_signal_received_by_recv_cap_delivers_no_cap_whichever_side_parks_first` parks a
  thread in `RECV_CAP` on an interrupt-bound rendezvous and then calls `irq_notify`; the control
  counts the signal first. Both must return `w0 = 1`, `x1 = NO_CAP`.
- `a_death_message_received_by_recv_cap_delivers_no_cap_whichever_side_parks_first` parks a
  supervisor in `RECV_CAP` and then lets a supervised child fault; the control lets the corpse park
  first. Both must return the fault event, `x1 = NO_CAP` and the child's id in `x2`.

Both are shared scheduler code and run in the same suite on all three ISAs (DECISIONS §19).

**Both paths were correct.** Neither test found a defect. The tests are the measurement 634 asked for.

### Falsification, replayed 2026-10-03 (UTC) on aarch64

Each patch under `system_tests/falsifications/` reverts milestone 634's `cap_delivered` default
(the same hunk as 634's, one record per test so each is replayable by name). Replayed with
`xtask test --arch aarch64 --test <name>`, one test selected each time:

| test | exit | red at |
|---|---|---|
| interrupt | 1 | `an_interrupt_signal_received_by_receive_capability_delivers_no_capability_whichever_side_parks_first` in `receive_capability_attack_tests.rs`, `an interrupt signal handed the receiver x1 = 0 on the receiver-first order` |
| death | 1 | `a_death_message_received_by_receive_capability_delivers_no_capability_whichever_side_parks_first` in `receive_capability_attack_tests.rs`, `a death message handed the supervisor x1 = 4294967302 on the receiver-first order` |

Unpatched, the filter `recv_cap_attack` ran 7 tests, 7 passed, exit 0. The controls (signal-first,
corpse-first) are green under the patch, so the red is about the order and not the fixture. The first
cut of the death test asserted `x2` before `x1`. Its replay went red at `x2`, not the predicted
line, so the checks were reordered to make `x1` the headline and the replay was repeated.

### Recorded

The riscv64 and x86_64 legs were built (`cargo test -p system_tests --no-run`) and not booted here;
CI boots them. The falsification records are replayed on aarch64 only, like every other record in
this family.

`cargo xtask ...` run through the rustup proxy builds the std exerciser against the unpatched std in
this checkout. The proxy prepends the real toolchain's `bin` to `PATH`, so xtask's child `cargo`
ignores `RUSTUP_TOOLCHAIN=<farm>`. The build then fails in `std`'s `cfg_select!`. Running
`target/debug/xtask` directly with `CARGO_MANIFEST_DIR=$PWD/xtask` exported worked. Not diagnosed
further; recorded in `notes/std/caveats.md`.

## Follow-on

- **Recorded.** The unpatched-std exerciser build is in `notes/std/caveats.md`, the appendix
  `notes/std.md`'s `BUGS` section links.

## Index row

Milestone 634 left an interrupt signal's and a death message's `RECV_CAP` results reasoned from the code rather than measured. BUILT: a test drives each through the receiver-first rendezvous order, with a replayable falsification; both paths were correct.
