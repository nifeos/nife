---
status: NOT-STARTED
raised: 2026-09-16
promoted_from: a-usurper-that-reports-instead-of-hanging
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 417. A usurper that reports instead of hanging, so row 26 can be falsified

Promoted from the proposal `a-usurper-that-reports-instead-of-hanging`,
filed 2026-09-16 by milestone 305, which wrote the honest defect for
`notes/confinement-claims.md`'s row 26 and could not use the result. *(Number provisional until the
merge queue lands it.)*

[milestone 106 (a wait that ends on either the interrupt or the deadline)](0106-deadline-wait.md)
built `Timer::ARM`, and [§101 (notification objects)](../decisions/0101-notification-objects.md)'s
binding ends a blocked `RECEIVE_CAP` at the deadline it signals.
`kernel/src/sched/notification.rs` records that the binding wakes `RECEIVE`, `RECEIVE_CAP` and
`Irq::WAIT` alike. The reporting
usurper is therefore buildable on surface that exists: arm a timer, bind the notification, and the
receive returns either a capability it should not have or the deadline. The first-class
non-blocking receive this block priced stays unwritten until a consumer that cannot arm a timer
asks for it, which is the same discipline
[§149 (may the kernel answer on an endpoint)](../decisions/0149-kernel-served-console-endpoint.md)
applied to the console.

Premise re-checked 2026-09-19 and still true. Row 26 of `notes/confinement-claims.md` still
answers no in the falsified column, and `hot_swap_contract::try_receive_cap` still invokes `RECEIVE_CAP`
directly, so the name still promises a try that the syscall does not offer.
`fixtures/src/swap_boundary_witness.rs` still calls it at one site. Nothing in the syscall surface
has gained a non-blocking or timed receive.

## In brief

Row 26 is *"a client of a rendezvous cannot become its server."* Its test,
`kernel::user::live_swap_tests::a_client_of_the_stable_rendezvous_cannot_become_its_server`, hands an
attacker exactly what the honest client holds and requires `NotPermitted` when it tries `RECEIVE_CAP`.
The kernel enforces that in three lines of `kernel/src/syscall.rs`.

Deleting those three lines, which is the complete break of the claim, does not fail that
assertion. Measured 2026-09-16 on aarch64: the run came back as a 60-second watchdog reading "no
progress ... a lost-wakeup hang" with a thread dump, and nothing in it about impersonation.

The reason is structural rather than a flaw in the patch. `RECEIVE_CAP` is a blocking receive, so an
attacker the kernel fails to refuse does not come back and report an escape. It takes the message the
honest server was waiting for, or parks on the rendezvous, and the run deadlocks. The assertion that
states the claim is reachable only in the world where the kernel *does* refuse.

So the row is `unfalsified` and the test has never been shown able to fail.

## Why this is worth a block rather than a `BUGS` line

It already has a `BUGS` line, in `notes/confinement-claims.md` and in the test's own
`Falsification: unfalsified` record. §71's promotion trigger is met by the second half: this is not a
limitation of the system, it is a confinement test that cannot fail, which is the exact defect
milestone 305 found once already on riscv64 and fixed. One instance is a bug; two is a pattern with a
cause, and the cause here is a missing primitive rather than a mistake anyone made.

## What it would take

Milestone 202 solved the same shape for §31 by adding `wait_for_report`, a bounded wait, so that a
stall is reported where it means something instead of surfacing 234 seconds later as a watchdog. The
equivalent here is that the usurper's attempt must return, so `swap_boundary_witness`'s `usurp()`
can report "the kernel let me receive" as a verdict rather than blocking on it.

`hot_swap_contract::try_receive_cap` is named `try_` and is not one: it invokes `RECEIVE_CAP` straight, and
`RECEIVE_CAP` blocks. That naming is itself worth calef's attention.

Options, costed as far as this lane could without building them:

1. A non-blocking `RECEIVE_CAP`, or a flag on it. Smallest kernel change; a new syscall behavior
   every future program is written against, which is exactly the category AGENTS.md calls expensive.
2. A timed receive. §106 (take)'s block already priced a timed wait and found the mechanism about thirty
   lines, with the authority question the whole problem. That work is adjacent.
3. **Leave the syscall alone and give the attacker a watchdog thread**, so the *test* bounds the wait
   rather than the kernel. No surface change and no benefit to anything but this test, which is the
   argument against it. The same gap will be hit by the next component that wants to ask a rendezvous
   a question it might not be allowed to ask.

**Recommendation: none**, deliberately. AGENTS.md draws the line at the syscall surface, and a
recommendation there is most of the decision already made.

## What is blocked until it is answered

Row 26 of `notes/confinement-claims.md` stays `unfalsified`, and `design/fatal-risks/README.md`'s risk 7
keeps one claim whose test has never been shown able to fail. Nothing else waits on this.

## Index row

Row 26 of `notes/confinement-claims.md` is that a client of a rendezvous cannot become its server,
and **deleting the three lines of `kernel/src/syscall.rs` that enforce it does not fail the test
that states it**. Measured 2026-09-16, the run came back as a 60-second watchdog reading "a
lost-wakeup hang" with nothing in it about impersonation. The cause is structural rather than a flaw
in the patch. `RECEIVE_CAP` blocks, so an attacker the kernel fails to refuse never returns to report
the escape, and the assertion is reachable only in the world where the kernel does refuse. The
attempt returns on surface that exists.
[Milestone 106 (a wait that ends on either the interrupt or the deadline)](0106-deadline-wait.md)
built the timer, and [§101 (notification objects)](../decisions/0101-notification-objects.md)'s
binding ends a blocked `RECEIVE_CAP` at its deadline. So the falsification is a lane's work. The
first-class non-blocking receive stays unwritten until a second consumer that cannot arm a timer
asks for it, and the `try_receive_cap` naming below is still calef's.
