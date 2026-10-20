---
status: PROPOSED
raised: 2026-10-20
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A futex word is walked and touched under the revocation lock

Raised by the 2026-10-20 security audit
([its report](../../audit-reports/2026-10-20-the-window-the-supply-chain-audit-left-unread.md),
finding 3). Name: provisional.

Reuse: the lock the revocation path already takes over a space's mappings. Nothing new is written
beyond holding it.

## The failure

`SYS_EXIT_THREAD` clears the thread's clear-word: `kernel/src/sched/futex.rs` translates the user
address with `translate_user`, then stores zero through the direct map, holding no lock across the
two. A sibling thread holding `WRITE` on the region behind the word can destroy it in between
(`kernel/src/sched/region_reaping.rs`). If the frame is then reused, four zero bytes land in a
frame the exiting space no longer owns, which may be another process's page or a kernel object.

`WAIT`'s load has the same unlocked walk, and [notes/futex.md](../../../notes/futex.md)'s `BUGS`
records it as a one-bit observation. The store is a write, which is worse. Neither is shown
reachable: the window is a few instructions, and the audit did not confirm that a destroyed
region's frames can be reused before the store lands.

## The fix

Walk and touch the word under the lock the revocation path takes, for both the load and the store.
Settle first whether a destroyed region's frames can be handed out again inside that window; if
they cannot, record why and close this as moot.

## Exit test

A kernel test that races a region's destroy against a thread exit whose clear-word lies in it, and
checks that no byte outside the exiting space's frames changes.
