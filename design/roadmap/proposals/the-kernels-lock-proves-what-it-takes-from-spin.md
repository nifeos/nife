---
status: PROPOSED
raised: 2026-10-25
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The kernel's lock proves what it takes from spin

Raised 2026-10-25 (UTC) by the lane maintainer/kernel-dependency-unsafe-listing, which wrote the
listing that §46 (thin primitives or whole subsystems; we write everything in between) asks for in
its kernel bar, condition 4. Title and slug are drafts.

Reuse: `spin` 0.12.3 itself, which the kernel already links; loom, which the tree already uses for
`work_steal_slot` and `thread_wake_handshake`; Kani, already pinned. Writing a lock in the tree is
one of the options below, not the default.

## The gap

`IrqSafeMutex` (`kernel/src/sync.rs`) wraps `spin::Mutex`. [notes/memory-ordering.md](../../../notes/memory-ordering.md)
says most of the kernel's happens-before edges come from that lock's Acquire compare-exchange and
Release store, and §9 (locking) and §11 (per-CPU run queues) lean on them. The `unsafe` that turns
those orderings into a `&mut T` with no data race is `spin`'s guard `Deref` and `DerefMut`
(`src/mutex/spin.rs:384`, `:391`) and its `Send` and `Sync` impls. Nothing in nife proves the lock
gives exclusion. [notes/kernel-dependency-unsafe.md](../../../notes/kernel-dependency-unsafe.md) lists
every site and what checks it: upstream Miri over the crate's tests, and a text pin here on the two
orderings, which fails `script/lint` if a bump weakens either.

The bar asks nife to prove any claim that rests on a dependency's `unsafe`: either Kani harnesses
against the crate's public API, or no claim rests on it. Kani has no threads, so it cannot prove
exclusion between cores. Loom can, but only over atomics it substitutes, and `spin` uses `core`'s.

## Options

1. Recommended: write the lock as a crate with loom-ready atomics, and drop `spin`. The tree
   already moved three protocols this way (`work_steal_slot`, `thread_wake_handshake`,
   `memory_corruption_canary_gate`): a small crate, `cfg(loom)` atomics, a loom model of the
   protocol, and the kernel calling the checked type. A test-and-set spinlock with a guard is about 80 lines. Rule 1 of §46 is the
   reason the bar names for a structure Kani cannot reach. Cost: one more crate on the
   verification path, and the lock measurement in
   `notes/benchmarks/host-reordering-under-tcg.md` rerun against it.
2. Keep `spin`, prove what Kani can, and model the protocol beside it. Sequential Kani harnesses
   against `spin::Mutex`'s public API (a held lock refuses `try_lock`, a dropped guard releases it,
   `force_unlock` releases it), plus a loom model of a copy of `SpinMutex`'s protocol kept honest by
   the existing text pin. The proof covers the copy, not the crate, and says so.
3. Record the C11 argument and stop. The text pin stays the only mechanism, and the BUGS entry
   in notes/memory-ordering.md stays open.

## Exit test

For option 1, four things hold. `cargo tree -e normal` shows no `spin` on any kernel target. A
loom model in the new crate explores two and three contending threads, and fails when the release
store turns Relaxed. `script/test` passes on all three architectures. `script/bench` floors move by
no more than their recorded noise, or the block explains each difference.

For option 2: the harnesses run in `script/verify`, each falsified by weakening one ordering or
removing the guard's release.

## Index row

Most of the kernel's happens-before edges come from `spin::Mutex`, and nothing in nife proves that
lock gives exclusion. Proposed: write it as a loom-checked crate, or prove what Kani can and model
the rest.
