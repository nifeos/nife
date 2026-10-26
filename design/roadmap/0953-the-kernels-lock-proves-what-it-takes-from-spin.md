---
status: BUILT
built: 2026-10-26
raised: 2026-10-25
promoted_from: the-kernels-lock-proves-what-it-takes-from-spin
milestone_dependencies: none
decision_dependencies: 46
machine_requirements: none
specific_machine: none
needs_person: no
---
# 953. The kernel's lock proves what it takes from spin

Raised 2026-10-25 (UTC) by the lane maintainer/kernel-dependency-unsafe-listing, which wrote the
listing that §46 (thin primitives or whole subsystems; we write everything in between) asks for in
its kernel bar, condition 4. Title and slug are drafts.

Ruled 2026-10-25 (UTC): option 1. The kernel's lock becomes a small in-tree crate with loom-ready
atomics, checked by loom models with two and three threads and a falsification that fails when the
release store goes Relaxed, and the kernel drops `spin`. #2734 built it.

Reuse: `spin` 0.12.3 itself, whose protocol the new crate keeps; loom, which the tree already uses
for `work_steal_slot` and `thread_wake_handshake`; Kani, already pinned. Rule 1 of §46 decided
against taking `spin`, because loom cannot check a crate that uses `core`'s atomics.

## The gap

`IrqSafeMutex` (`kernel/src/sync.rs`) supplies most of the kernel's happens-before edges, through
its lock's Acquire compare-exchange and Release store
([notes/memory-ordering.md](../../notes/memory-ordering.md)), and §9 (locking) and §11 (per-CPU run
queues) lean on them. When this block was raised, that lock was `spin::Mutex`, and nothing in nife
proved it gave exclusion: the `unsafe` that turns those orderings into a `&mut T` belonged to a
dependency, checked by upstream Miri and a text pin here on the two orderings.

The bar asks nife to prove any claim that rests on a dependency's `unsafe`: either Kani harnesses
against the crate's public API, or no claim rests on it. Kani has no threads, so it cannot prove
exclusion between cores. Loom can, but only over atomics it substitutes, and `spin` used `core`'s.

## Options

1. Chosen: write the lock as a crate with loom-ready atomics, and drop `spin`. The tree already
   moved three protocols this way (`work_steal_slot`, `thread_wake_handshake`,
   `memory_corruption_canary_gate`): a small crate, `cfg(loom)` atomics, a loom model of the
   protocol, and the kernel calling the checked type. Rule 1 of §46 is the reason the bar names for
   a structure Kani cannot reach.
2. Keep `spin`, prove what Kani can, and model the protocol beside it. The proof would cover a copy,
   not the crate.
3. Record the C11 argument and stop, with the text pin as the only mechanism.

## Built 2026-10-26 (UTC)

#2734 built option 1.

- `crates/spin_lock` (name provisional) is a test-and-set spinlock with the protocol `spin` used:
  a weak Acquire compare-exchange in `lock`, a strong one in `try_lock`, a Relaxed load while
  waiting, and a Release store in the guard's drop. Under `cfg(loom)` the flag and the data cell
  are loom's.
- Five loom models run in `script/interleaving-check`: two takers exhaustively, three takers at
  preemption bound 1, `try_lock` refusing while held, a write before the release reaching the next
  holder, and the falsification, which runs the crate's own unlock with `Relaxed` and passes only
  when loom reports the race. [notes/interleaving/what-was-modeled.md](../../notes/interleaving/what-was-modeled.md)
  records why the three-taker model stops at one preemption.
- `IrqSafeMutex` wraps `spin_lock::SpinLock`. `cargo tree -e normal -p kernel` shows no `spin` on
  aarch64, riscv64 or x86_64, with or without `--all-features`, and `spin` has left `Cargo.lock`.
- [notes/kernel-dependency-unsafe.md](../../notes/kernel-dependency-unsafe.md) lists the crate's
  `unsafe` sites and what checks each.

## Exit test

All four hold, with the evidence in #2734. `cargo tree -e normal` shows no `spin` on any kernel
target. A loom model in the new crate explores two and three contending threads, and fails when the
release store turns Relaxed. `script/test` passes on all three architectures. `script/bench --check`
keeps every row within the 10% tripwire, and the LOCK test in
`notes/benchmarks/host-reordering-under-tcg.md` moves by less than its own run-to-run spread.

## BUGS

- A loom proof is a C11 proof, not a proof about aarch64 or riscv64 hardware;
  notes/memory-ordering.md carries that limit.
- `force_unlock` sits outside the loom models by construction.
- The crate's name is provisional, as are `SpinLock` and `SpinLockGuard`.

## Follow-on

- **Recorded.** The C11 limit of a loom proof lives in `notes/memory-ordering.md`, BUGS.
- **Recorded.** `force_unlock`'s place outside the models lives in
  `notes/kernel-dependency-unsafe.md`.
- **Recorded.** The crate's provisional name lives in the `Name:` block of `crates/spin_lock/src/lib.rs`
  until an architect names it.

## Index row

The kernel's lock is `crates/spin_lock`, an in-tree test-and-set lock whose exclusion and
happens-before edge loom checks with two and three threads and a Relaxed-release falsification, and
the kernel no longer links `spin`.
