# The port-range tests

An appendix to [`notes/x86-port.md`](../x86-port.md) for `system_tests/src/user/x86_port_tests.rs`.
It holds why each test is shaped the way it is. The claims they prove are rows 27 to 29 and 33 of
[`notes/confinement-claims.md`](../confinement-claims.md).

## Why a ring-3 `out` is the only honest test

A `PortRange` capability is enforced entirely at the context switch, by the TSS I/O permission
bitmap. Nothing on the syscall path can be asserted on. So each test runs a hand-assembled ring-3
child that executes `out`, and reads the outcome from the child's supervision endpoint: a word if
the CPU allowed the `out`, an `EVENT_FAULT` if it did not.

## The same core

The bitmap is per core. On one core, a non-holder that runs after a holder proves the hand-off by
order alone. On two cores, the non-holder can start on the core the holder never touched and fault
there whatever the switch did. Without a check, the falsification left the holder's grant on cpu 0,
the non-holder ran on cpu 1, faulted, and the test passed on the two-core direct boot
(`NIFE_SMP=2`). Only the real-firmware leg, which boots the suite on one core, went red.

So the holder reports the core it wrote on. The non-holder executes its `out` only when it is on
that core, and otherwise faults at a different pc, which the test counts as a miss and retries.
How many tries it allows, and the landing rate measured behind that number, is at
`NON_HOLDER_ATTEMPTS`.

The check does not cover a thread preempted and moved between its core read and its `out`. That
is three instructions, so the holder's bracket would need two preemptions and a steal to lie.

## Revoke and delete

A revoked holder is parked in `RECEIVE` while its range is revoked, then woken, and the `out` it
executes next must fault.

The delete test is milestone 313's audit (2026-09-17). The first two properties show the grant
following the capability across a switch and across a revoke. Neither could see whether it followed
the capability out of the thread's own table. The grant is a cached field the switch installs, so
`SYS_CAP_DELETE` has to clear the cache as well as the slot. Otherwise a thread that dropped its
port capability would keep the ports for life, which is §12's "a consumed capability cannot be used
again" failing for the one object enforced outside the table. The progenitor makes exactly this
drop on every x86 boot (`cap_delete(g.uart_dev)`).

The take-back test is the 2026-09-24 audit's. `PortRange::REVOKE` spares the invoker's own
capability, so the invoker's own core must keep its bitmap installed. A sparing that held in the
table and not in the TSS would be false of the hardware until the next switch-in.

## Exit, never report

A child whose `out` might be wrongly permitted exits rather than reporting. A report is a `SEND`,
and on a kernel with the defect it would park on a rendezvous nobody receives from, so the run would
hang instead of going red. That is row 26's shape in `notes/confinement-claims.md`.

In the delete test, a permitted `out` means the child exits cleanly. The supervisor's message is
then `EVENT_EXIT`, and the first `assert_eq!` is the one that fails, holding `EVENT_EXIT` where it
wanted `EVENT_FAULT`. The pc equality below it guards the other direction: a `syscall` that
faulted, or a bad slot, would report a different pc.

## Rights

Rows 27 to 29 claim things about holding a port capability, revoking one and deleting one. None of
them claims that the capability's rights gate the `in` and `out`. The rest of the tree treats
`WRITE` as the right that drives a port: `build_child` grants the driver `WRITE`, and
`capability_plan` maps a `Use` component to `WRITE` and a `Serve` component to `READ`. So a thread
handed a `PortRange` narrowed to `READ` alone should fault on `out`, as a non-holder does.

The TSS bitmap cannot grant `in` without `out`. `sched::thread_control_block_insert_from`
therefore installs `Thread::port_range_grant` only for a `PortRange` carrying `WRITE` (milestone 768,
provisional, calef's ruling of 2026-10-05 UTC). The escape this closed is written up in
[`notes/confinement-outsider-pass-2.md`](../confinement-outsider-pass-2.md), and the claim is row 33.
