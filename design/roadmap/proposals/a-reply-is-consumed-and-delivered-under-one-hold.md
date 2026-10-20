---
status: PROPOSED
raised: 2026-10-20
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A reply is consumed and delivered under one hold

Raised by the 2026-10-20 security audit
([its report](../../audit-reports/2026-10-20-the-window-the-supply-chain-audit-left-unread.md),
finding 2). Name: provisional.

Reuse: the existing `IPC_TABLES` hold that `REPLY_CAPABILITY` already takes to file its carried
capability; no new object or method.

## The failure

A plain `REPLY` copies the `Reply(tid)` capability out of the slot (`kernel/src/syscall.rs`,
`invoke`), delivers through `sched::ipc_reply`, and only then deletes the slot. `REPLY_CAPABILITY`
has the same order in `kernel/src/syscall/object_methods.rs`. `ipc_reply` checks that the caller is
parked waiting for a reply, not that it is parked waiting for *this* reply
(`kernel/src/sched/rendezvous.rs`).

Since milestone 812 (`std::thread::spawn` runs real threads in one address space), threads of one process share one capability table. So two server threads can
both read the same Reply slot. One answers the client; the client wakes, calls an unrelated server
and parks for that answer; the second thread's stale copy is then delivered as the unrelated
server's reply. A compromised server can do this on purpose. `notes/suspend-and-resume.md` names
stale-Reply forgery as a class the kernel rules out, and this is a route to it the shared table
opened.

## The fix

Take the slot and deliver under one hold: remove the capability from the table, check it was still
`Reply(tid)`, then deliver. A second thread finds the slot empty and gets `NoSuchSlot`. Or stamp the
Reply with the call's sequence number and have `ipc_reply` match it, which also covers a copy taken
by any other route.

## Exit test

A kernel test with two threads of one process holding one Reply slot: the second `REPLY` is
`NoSuchSlot`, and a client parked on a second call receives that call's answer only. The fast path
is on this route, so the change records `syscall_entry`'s measured footprint before and after.
