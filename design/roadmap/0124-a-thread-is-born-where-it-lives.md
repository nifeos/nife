---
status: BUILT
raised: 2026-08-14
built: 2026-08-17
---
# 124. A thread is born where it lives: the spawn path's copies

Built 2026-08-14, reopened 2026-08-16 when the same banner came back on both
ISAs, and closed 2026-08-17 when the banner turned out not to be this milestone's bug at all.
Minted by calef out of the riscv64 stack overflow milestone 108 was held on. The hold turned out to
be the wrong suspect three times over, and the third time is the interesting one.

The structural half is built (pull request #248, 2026-08-16) and the status did not move, which
is the honest reading rather than an omission. A per-CPU interrupt stack now exists on both
ISAs, 16 KiB per core over its own guard page, with the handler chain running there instead of on
whatever thread it interrupted. Three things deliberately stay on the interrupted stack: the trap
frame, a trap from user mode, and the deferred `schedule()`. The design's one rule is that
nothing on an interrupt stack may context-switch away from it. It is held three ways: a
`debug_assert!`, the doc at the thing itself, and a static CI proof that no context switch is
reachable from the interrupt-stack entry point on either ISA.

What that buys, stated as the lane stated it rather than as the headline suggests: the static
worst-case bound on a thread stack improved by only 256 bytes (13712 to 13456 on aarch64). What
remains is `schedule()`'s own chain, which is the cost of scheduling at all and which a
thread already pays to block in `ipc_recv`. The defensible claim is structural: a preemption now
costs the interrupted thread a trap frame plus the same scheduler tail a voluntary block costs,
and the handler is bounded on a stack of its own. The measured watermark did not move at all,
because in this suite the deepest byte of every stack is reached by ordinary code rather than by
an interrupt landing on top. That is what a watermark can and cannot say about a rare worst case.

The reopening's own reason was untouched, which is why the block stayed `PARTIAL` until
2026-08-17: the two faults were a store at a *fixed address*, not depth, and nothing here explained
them. `warn_if_guard_page` now prints the interrupted `sp` from the frame rather than the live one,
which after this change would have named the wrong stack.

## Reopened 2026-08-16, closed 2026-08-17: the stack was freed under its owner

The same overflow banner came back on both ISAs on 2026-08-16, in the same supervision test. It was
not an overflow. A supervised corpse is published as `Dead` while it is still executing on its own
kernel stack, and an out-of-band reaper on another core is allowed to free that stack. The exception
vector then walks `sp` to the slot base, which is why the address never moved.

The fix is one clause in `reap_region_objects`: refuse while `Handshake::on_cpu`, without arming a
kill, because the thread is already dead and the refusal clears one context switch later. Both
supervision tests now retry their reclaim. The fault was reproduced on the desk by widening the
window in `depart` with a spin loop, which was not committed. The better fix, deleting the window
rather than refusing inside it, is milestone 331 (a thread is published `Dead` while it is still executing on its own kernel stack) in the Follow-on.

[`reopened-and-closed.md`](0124-a-thread-is-born-where-it-lives/reopened-and-closed.md) holds both
sections in full: the evidence, the refuted depth hypothesis, why only this test and only CI, and
what this milestone's own work is worth.

## The number, and why it is the interesting kind of number

`sched::spawn_on` carries a 4592-byte frame, and the guard page under every kernel thread stack is
4096 bytes. It is generic over the spawned closure, so every service that spawns gets its own
instantiation: ten of them, 3888 to 4592 bytes, over the guard page on both ISAs, measured with
`script/stack-frame-check`.

A frame larger than the guard page is not merely close to overflowing. It can move `sp` from inside
the stack to below the guard in a single step, touching nothing in between, so the guard never
faults and the write lands in the neighboring thread's stack instead. The overflow stops being a
legible fault and becomes corruption that surfaces somewhere else entirely, arbitrarily later.

That is not hypothetical here. On 2026-08-14 a `thead-c906` run faulted 4088 bytes below the stack
bottom on a 4096-byte guard. Eight more bytes and there would have been no fault at all.

## Where the bytes are, measured rather than guessed

| symbol | frame |
|---|---|
| `sched::spawn_on::<fs_service::spawn_fs_server>` | 4592 |
| `sched::spawn_on::<compositor_service::start>` | 4576 |
| ... eight more instantiations ... | 3888 to 4560 |
| `Thread::spawn::<fs_service::spawn_fs_server>` | 2720 |
| `capability::CSpace<cap::Object, 16>`, one field of `Thread` | 384 bytes of type |

The per-instantiation spread is only about 700 bytes and tracks `size_of::<F>()`, so the closure is
the small part. Roughly 3900 bytes is constant, and it is the `Thread` traveling by value.

## What the work was

This section proposed the wrong fix, and what replaced it is worth reading. The proposal was
`insert_in_place` on `crates/slots`, on the belief that `Table::insert_with` stored the `Thread`:
`self.slots[slot] = Some(f(name))`, so the closure builds it, returns it, it is wrapped in `Some`,
and then stored.

The table does not store a `Thread`. It stores a `TcbPtr`, because a TCB lives on its own page,
and `Threads::insert_at` is where the value actually lands:

```rust
let ptr = crate::arch::mmu::phys_to_virt(page) as *mut Thread;
self.table.insert_with(|tid| { unsafe { ptr.write(f(tid)) }; TcbPtr(ptr) })
```

So `crates/slots` was never touched. The copies were all between `Thread::spawn`'s frame and that
`ptr.write`: build a `Thread`, return it by value, hold it in `spawn_on`, move it into a closure,
return it from the closure, write it. Five hops, each a real memcpy in a debug build, and a debug
build is what CI runs.

What shipped instead:

```rust
// kernel/src/thread.rs
pub unsafe fn spawn_into<F: FnOnce() + Send + 'static>(f: F, id: Tid, dst: *mut Thread) -> bool

// kernel/src/sched.rs
fn insert_in_place(&mut self, build: impl FnOnce(Tid, *mut Thread) -> bool) -> Option<Tid>
```

`Thread::spawn` survives as a thin `MaybeUninit` wrapper over `spawn_into`, because `sched::init`'s
idle thread and `spawn_blocked` hold no TCB page when they build. They keep the copies, and nothing
on their paths is near a guard page.

The decline path is the part to read twice. `insert_at_in_place` mints the name before `build` runs,
so a build that fails has to give it back, and `Table::remove` is the right primitive rather than a
leak: the slot holds a `TcbPtr`, and dropping that drops a pointer. The `Thread` drop lives in
`Threads::remove`, which this path never reaches because no `Thread` was ever constructed.

## A hypothesis that was measured and refuted, kept so nobody repeats it

The first proposal was that the closure's opening `let mut thread = thread;` cost a whole `Thread`
copy, and that deleting the rebind would fix it. It changes nothing. Removing it and rebuilding
produced byte-identical frames across all ten instantiations (4592 to 4592, 4576 to 4576, and so on):
the compiler already elides that rebind. The copies are in the value-passing chain, not in the
rebinding, which is why this milestone proposes an API change rather than an edit.

## Why this is not just a stack-size question

Raising `STACK_PAGES` from 4 to 8 would buy headroom and is one number. It is the wrong lever here
for a reason specific to this shape: the guard page stays one page no matter how large the stack
is, so a 4592-byte frame can still step over it. Growing the stack moves the overflow further away
without restoring the mechanism that makes an overflow *legible*. Shrinking the frame below 4096
restores it.

Growing the stack is still worth considering on its own merits, and the two are independent.

## What "done" meant, and what it measured

Every `spawn_on` instantiation under 4096 bytes on both ISAs, and the `RATCHET` entry deleted rather
than lowered. Both hold: the worst went 4592 to 1040, and the gate now reports "no ratchet" on
aarch64 and riscv64. The `slots` harnesses needed no reading, for the reason above.

The independent confirmation is the better evidence. The icount tripwire failed this branch on an
*improvement*: `spawn_reap` moved 154725 against a 173742 baseline, 10.9% fewer instructions, and
12.9% on riscv64. A benchmark with no idea what changed measured the memcpy that is no longer there.
Both baselines were updated from CI's own numbers rather than computed, and the riscv64 case earned
that discipline: scaling the aarch64 ratio would have written 25382 against a real 24835, which is
both wrong and still outside the tolerance band.

## Prior art

A design to copy: seL4 retypes a TCB out of untyped memory *in place*, at an address the caller
names, so there is no "construct then move" step for the same reason this milestone exists. The
object is born where it lives.

A mistake to avoid: treating this as a debug-build artifact worth ignoring because release builds
elide the copies. CI runs debug, the guard-page fault is real in debug, and a demonstrator whose
proofs and tests run in a configuration nobody checks is not demonstrating.

## BUGS

- The prediction that `crates/slots` was the main cost was wrong, and it is kept here because it
  is the plausible reading of `Table::insert_with` and the next person will make it too. The table
  stores a `TcbPtr`; the `Thread` is on its own page. No harness moved.
- `Thread::spawn` still carries the copies for `sched::init`'s idle thread and `spawn_blocked`,
  which hold no page when they build. Nothing on those paths is near a guard page today, and if one
  ever is, this is where to look first.
- `-Z emit-stack-sizes` measures frames, not call chains, so "every instantiation under 4096"
  bounds one frame and not the depth of the path it sits in. The watermark in
  notes/stack-high-water.md is the other half, and neither is sufficient alone.
- The ratchet is gone, so nothing now holds these frames except the gate's own 4096-byte ceiling,
  which is the state a ratchet exists to reach and then be deleted from.
- The riscv64 overflow is still unexplained, and this milestone did not prove it fixed. #157
  fixed the aarch64 one by shrinking `reap_region_objects`. The riscv64 fault was a different chain on
  a different slot, and this was the prime suspect rather than a demonstrated cause. What closed here
  is the separate hazard that ten frames could step over the guard entirely. If it recurs, the
  call-graph walker is the next instrument and nothing in the tree has one, which is now the answer
  to "why did we not catch this" twice over.

  Closed 2026-08-17, and not by this milestone. It was never an overflow on either ISA: the
  stack was freed under its owner and the vector walked `sp` to the slot base. See
  [`reopened-and-closed.md`](0124-a-thread-is-born-where-it-lives/reopened-and-closed.md). The walker did get built and did earn its keep, by refuting depth rather than by measuring
  it.
- The refusal is a race a caller can still see, one context switch wide, and it is a
  `NotPermitted` rather than a corrupted kernel. Any caller reclaiming a region that holds a
  just-dead thread must retry; `wait_for` is the in-tree idiom and both supervision tests now use
  it. The window itself is closed only by the `Departing` change, milestone 331 in the Follow-on.
- The `on_cpu` guard is a condition in one function, which is rung two of AGENTS.md's ladder.
  Rung one would be a type that cannot name a thread still standing on its stack, and this tree does
  not have one. The next out-of-band remover can forget it exactly as this one did.

## Follow-on

- **Recorded.** In `design/roadmap/0124-a-thread-is-born-where-it-lives.md`'s own BUGS: the refusal
  is a race a caller can still see, one context switch wide. Any caller reclaiming a region that
  holds a just-dead thread gets a `NotPermitted` and must retry, and `wait_for` is the in-tree idiom
  both supervision tests now use.
- **Recorded.** In `design/roadmap/0124-a-thread-is-born-where-it-lives.md`: the `on_cpu` guard is a
  condition in one function, which is rung two of AGENTS.md's ladder. Rung one would be a type that
  cannot name a thread still standing on its stack, and this tree has no such type, so the next
  out-of-band remover can forget the check exactly as this one did.
- **Recorded.** In `design/roadmap/0124-a-thread-is-born-where-it-lives.md`: `Thread::spawn` still
  carries the copies for `sched::init`'s idle thread and `spawn_blocked`, which hold no TCB page
  when they build. Nothing on those paths is near a guard page today, and this is where to look
  first if one ever is.
- **Recorded.** In `design/roadmap/0124-a-thread-is-born-where-it-lives.md`: the ratchet is deleted
  rather than lowered, so nothing holds these frames except the gate's own 4096-byte ceiling. That
  is the state a ratchet exists to reach and then be deleted from.
- **Recorded.** In `notes/stack-high-water.md`: `-Z emit-stack-sizes` bounds one frame and not the
  depth of the path it sits in, so "every instantiation under 4096" and the watermark are each half
  an answer and neither is sufficient alone.
- **Recorded.** In `design/roadmap/0124-a-thread-is-born-where-it-lives.md`: the prediction that
  the slot table, `nifeos/generational_table` (this block's prose still calls it
  crates/slots, its name before the rename), was the main cost was wrong. It is kept because it is
  the plausible reading of `Table::insert_with` and the next person will make it too. The table stores a pointer; the thread
  is on its own page.
- **Refused.** Raising `STACK_PAGES` from 4 to 8 as the fix. The guard page stays one page however
  large the stack is, so a frame bigger than 4096 bytes can still step over it in a single move and
  the overflow stays illegible. Growing the stack moves the fault further away without restoring the
  mechanism that makes it a fault at all; shrinking the frames below 4096 restores it. Growing the
  stack remains an independent question on its own merits.
  This refusal is milestone 454 (design/roadmap/0454-the-stack-sizes-nobody-has-sized.md), which
  carries it with the condition that would change it.
- **Milestone 331.** Delete the
  window in which a thread is published `Dead` while it still executes on its own kernel stack,
  instead of refusing inside it. Mark it `Departing` in `depart()` and promote it to `Dead` from
  `finish_switch`, which already holds `SCHED` and already runs at the instant the stack goes free.
  No caller would then see the transient `NotPermitted` this block records as a race. It touches the
  death protocol and `RunState` in `crates/wake_handshake`, where loom searches the transitions, so
  it wants a lane rather than a hotfix.

## Index row

`sched::spawn_on` carries a 4592-byte frame and the guard page under every kernel thread stack is
4096, so ten instantiations can step from inside the stack to below the guard in one move,
touching nothing, and the guard never fires. Measured on both ISAs; a `thead-c906` fault landed
4088 bytes below the bottom of a 4096-byte guard. The `Thread` travels by value from `Thread::spawn` into `Table::insert_with`, which a debug build copies rather than elides; the fix
is an in-place build (`Thread::spawn_into` into `Threads::insert_in_place`), and the Kani-verified
`crates/slots` was never touched. Raising `STACK_PAGES` is the wrong lever: the
guard page stays one page. Reopened 2026-08-16 and closed 2026-08-17: the returning banner was
never an overflow. A supervised corpse is published `Dead` while still executing on its own kernel
stack, and an out-of-band reap frees that stack under it; the exception vector then walks `sp` to
the slot base, which is why the address never moved. Refused while `handshake.on_cpu`
