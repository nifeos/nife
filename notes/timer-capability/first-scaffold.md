# The first scaffold: what the fourth shape costs, 2026-09-05

*Name: provisional. For the 2026-09-05 scaffold the parent's "What the fourth shape costs" measured.
calef names files.*

An appendix of [can a userspace process hold a timer?](../timer-capability.md). The parent keeps the
shape and the three headline figures. This file keeps the scaffold's method, the tables and what the
scaffold did not price. The scaffold was built and deleted on 2026-09-05.

## What was already priced, and what was not

The bookkeeping was already priced and is not re-measured here. `notes/timed-wait.md` measured any
deadline structure at one comparison per idle tick: 1.000 comparisons and 0.000 writes over 100,000
ticks for a scan and for a sorted list, and 1.004 for a wheel. It priced a per-thread
`deadline: u64` at zero bytes, because TCBs are page-resident with 3,352 bytes of slack. What was
unpriced is the object: a new `Object` variant, its creation from untyped, its dispatch arm, and
whether any of it lands on the IPC fastpath.

## Method

A throwaway scaffold was built on 2026-09-05 and then deleted. It held:

- `Object::Timer(u64)`, and an `abi::timer` module with `ARM` and `CANCEL`;
- a `RETYPE_OBJ` arm minting one from the caller's own untyped;
- two `#[inline(never)]` dispatch bodies;
- a 64-entry armed-timer table with a cached earliest deadline;
- an `expire` call from `sched::on_tick` that signals through the existing `sched::irq_notify`.

`script/test` passed with it wired in (205 passed, 69 skipped, all three ISAs plus the OVMF leg)
before it was removed. So the numbers are from a working system and not a half-compiled one.

## The object is free in bytes

`kernel/src/cap.rs` asserts `size_of::<Object>() == 24` and `size_of::<Cap>() == 32` at compile
time. Both assertions still hold with the variant added, because `PageFrame(u64, NonZeroU64)` is
already the widest arm. So a `Timer` capability costs zero additional bytes per capability slot,
zero per capability table, and zero per TCB. That is the whole of the per-thread and per-process
cost.

## The IPC fastpath is untouched, measured

`script/fastpath-footprint`, before and after, on the same machine within twenty minutes:

| ISA | `ipc_fastpath` before | after | `syscall_entry` before | after | delta |
|---|---|---|---|---|---|
| aarch64 | 7,028 | 7,028 | 1,504 | 1,516 | +12 B (+0.8%) |
| riscv64 | 5,936 | 5,936 | 1,828 | 1,986 | +158 B (+8.6%) |
| x86_64 | 8,122 | 8,122 | 1,637 | 1,733 | +96 B (+5.9%) |

`ipc_fastpath` does not move at all on any architecture, which is the number that matters: an IPC
round trip fetches exactly what it fetched before.

`syscall_entry` moves because that half is measured *flat*. The decoder's own bytes are on every
syscall, and its other arms are not on this path. So one more `Object` arm in `invoke` is one more
decode step's worth of bytes for every syscall in the system, whether or not it is a timer.

The riscv64 figure is the one to argue about. 158 bytes is 8.6% of that ISA's entry set, and
`script/fastpath-footprint`'s own `BUGS` names this exact mechanism as an open problem
(`design/roadmap/0368-a-flat-entry-set-counts-bytes-no-syscall-fetches.md`). Three instances were
closed with `#[inline(never)]` in two days on 2026-09-04. Both scaffold bodies here already carry
it, so this is the cost *after* that mitigation rather than before it.

## The kernel grows by under a quarter of a per cent

Sum of symbol sizes in the release kernel (`llvm-nm --print-size`), before and after:

| ISA | before | after | delta |
|---|---|---|---|
| aarch64 | 1,009,252 | 1,009,990 | +738 B (+0.073%) |
| riscv64 | 1,011,788 | 1,014,054 | +2,266 B (+0.224%) |
| x86_64 | 962,936 | 964,322 | +1,386 B (+0.144%) |

That is the whole feature: object variant, two methods, retype arm, expiry table, tick hook, and the
signaling call. The riscv64 figure is again the largest, for the same reason its entry set is.

## What this does not price

- The scaffold's expiry table is a 64-entry array with a linear rescan on every arm. That is the
  crudest of the three structures `notes/timed-wait.md` modeled, chosen because it is the smallest
  thing that runs. A real implementation would put the deadline on the object's own page, the way
  every other page-resident object in this kernel works, and the arm would not rescan. The size
  figures above therefore over-estimate the table and under-estimate the revocation and
  generational-naming machinery a real object needs.
- Revocation was not built. A real `Timer` is retyped from untyped and must die when its region
  is destroyed (`MemoryRegion::DESTROY`, object revocation), which is bookkeeping the scaffold has
  none of. Priced 2026-09-13 in [second-scaffold.md](second-scaffold.md): it is 268 bytes and it
  is not optional.
- Nothing was measured in time, only in bytes. `notes/timed-wait.md`'s +30/+31 instructions per
  tick is the executed-path number and it still stands; nothing here changes it.
- The kernel-side consumer was not priced, because on 2026-09-05 nobody had found one. Milestone
  106's own census found one on 2026-09-13 (`kernel/src/soak.rs`'s supervisor), and
  [second-scaffold.md](second-scaffold.md) prices it.
