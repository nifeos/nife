# The answer: a kernel stack freed under its owner (2026-08-17)

*An appendix to [`notes/stack.md`](../stack.md), which explains the stack and is the page to read.
This file holds the answer to the 2026-08-16 faults, a kernel stack freed under its owner (its BUGS
are on the main page). The five appendices read in the order the main page's table lists them, so
"above", "below" and "this file" in them mean that whole sequence. The directory `notes/stack/` and
this file's stem are provisional names; naming is calef's.*


Everything above this line about "two addresses, six faults" reached the right verdict on depth
and the wrong reason for it, and the wrong reason sent a lane hunting a stray store that does not
exist. This section is what the faults actually were. It supersedes "The guard-page faults of
2026-08-16, which were not overflows" wherever the two disagree; that section's *conclusion* (not
depth, not a frame over the guard, not the class milestone 124 closed) stands.

In brief: a supervised corpse is marked `Dead` and its death message delivered while it is still
executing on its own kernel stack. An out-of-band reaper on another core is allowed to free that
stack in the few hundred instructions before the corpse reaches `switch_to`. The corpse then stores
to unmapped memory, and the exception vector's own frame store faults on the same stack. The vector
walks `sp` down one 272-byte frame at a time until it lands in the mapped stack of the slot below.
The address that gets reported is the last step of that walk, and arithmetic pins it to the slot's
base every single time.

## Why the address never moved, which is the fact the reopening was built on

It never moved because it *cannot* move. Nothing about it is evidence of a fixed-site writer.

aarch64's `SAVE_CONTEXT` opens `sub sp, sp, #272` and then stores `stp` pairs at `[sp, #16*0]`
through `[sp, #16*16]`, walking upward from `sp`. If a level of the walk faults, the next level
starts 272 lower and stores upward again. Let `G` be the guard-page base the report names.

- A level faults iff some store address lands at or above `G` (unmapped), i.e. iff `sp + 256 >= G`.
- A level succeeds iff `sp + 256 < G`, so the walk terminates at the first `sp < G - 256`.
- The terminal *failing* level therefore has `sp` in `[G - 256, G)`. It stores from offset 0 upward,
  so it faults at the first offset `k` with `sp + k >= G`.
- `sp` is 16-byte aligned, the offsets step by 16, and `G` is page-aligned, so `G - sp` is a
  multiple of 16 and there is always an offset landing on `G` exactly.

So `FAR_EL1` is exactly `G`, always, for any overflow that reaches the vector cascade. RISC-V's
`trap_entry` does the same thing with `sd` at `1*8(sp)` through `31*8(sp)`, which gives `G` when
`sp < G` and `G + 8` when `sp == G` exactly. Those are precisely the two riscv64 addresses ever
recorded, and this file said so in the 2026-08-15 section ("The repeated fault address is a
signature, not a coincidence") forty lines before the 2026-08-16 section asserted the opposite.

## And `ELR_EL1` says the same thing, in all three aarch64 dumps

The walk is not a hypothesis about these faults. Each dump's `ELR` names the exact `stp` that
faulted, and `stp`'s offset says where `sp` was:

| when | run | `ELR_EL1` | `ELR mod 0x800` | the instruction | implied `sp` |
|---|---|---|---|---|---|
| 2026-08-13 | notes/frames.md | `0xffff00004012fa34` | `0x234` | `stp x24, x25, [sp, #16*12]` | `G - 192` |
| 2026-08-15 | 31907966383 | `0xffff000040130214` | `0x214` | `stp x8, x9, [sp, #16*4]` | `G - 64` |
| 2026-08-16 | 31920141776 | `0xffff00004013a228` | `0x228` | `stp x18, x19, [sp, #16*9]` | `G - 144` |

`VBAR_EL1` requires 2048-byte alignment and `vectors.s` is `.balign 0x800`, so `ELR mod 0x800` is
the offset into the table; `0x200` is the Current EL, SP_ELx, Synchronous entry, which is the
exception every one of these dumps reports. Entry 4 begins with `sub sp, sp, #272` at `+0x200` and
then one `stp` every four bytes, so `+0x214`, `+0x228` and `+0x234` are the 5th, 10th and 13th
`stp`: store offsets 64, 144 and 192. Every implied `sp` lands inside the `[G - 256, G)` window the
arithmetic above predicts, and every one is a multiple of 16 below `G`. Three dumps, three
independent confirmations, and the middle one was already confirmed by a container rebuild.

One of those three is where this file went wrong, and the mistake is worth naming. The 2026-08-16
section reasoned that `ELR = 0xffff00004013a228` was "ordinary Rust roughly 550 KB further into
`.text`" because two local builds place `exception_vectors` at `0xffff0000400b4000`. But its own
2026-08-15 section had already established that CI's vector base was `0xffff000040130000`, half a
megabyte away from the local one, for a build a day older. A local address was used to read a CI
address in a file that had already measured the two to be different. This file's own rule covers it:
take the number from the run, not from here.

## The fourth occurrence, which is the one that names the mechanism

CI run 31960738448 (merge queue, pull request #249, 2026-08-16 17:09Z), same test, slot 87
again, now at `0xffff001000261000` because `STACK_PAGES` had gone 4 to 6 and the slot span with it.
The slot number survived a change to the address arithmetic, which is what says the thing being
repeated is a position in the allocation sequence rather than an address.

This was the first firing after milestone 124 added the conservative `.text` scan, and the scan
faulted:

```
  *** KERNEL STACK OVERFLOW ***
  0xffff001000261000 is in THREAD stack slot 87's guard page (thread.rs).
  bottom 0xffff001000262000, ...
  Words on the dead stack that point into .text (...), deepest first,
  as `bottom+offset: word` ...
                                          <- nothing, and then a NEW exception:
  ESR_EL1   0x0000000096000007            <- DFSC 0x07, translation fault, WnR 0: a READ
  FAR_EL1   0xffff001000262000            <- exactly `bottom`
  x8        0xffff001000262000            <- exactly the scan's cursor
```

Slot 87's stack is not mapped. The scan's first read of the first word took a translation fault. A
live thread cannot be running on an unmapped stack and cannot overflow one, so this is not a thread
running out of room. The stack was freed, and the walk that produced the guard-base address came
down through seven unmapped pages to reach the mapped stack below.

That nested fault also destroyed `ELR_EL1` and `FAR_EL1` before either was printed, and suppressed
the `sp` line the same change had added. The instrument ate its own report on its first real
firing. Both halves are fixed in `stack.rs` (the scan runs last, and asks `mmu::translate` before
each page), and its `BUGS` section carries the story.

## The mechanism, in `sched.rs`

`depart()` is a thread's last act. For a supervised thread it does this, and the comment two lines
above the release already knows the danger:

```rust
{
    let mut guard = SCHED.lock();
    ...
    t.handshake.state = State::Dead;
    deliver_death(sched, current, ep, msg);   // wakes the supervisor, possibly on another core
    // "Not requeued and not removed: we are still on this stack."
}                                             // <-- SCHED released HERE
schedule();                                   // <-- switch_to is still ahead of us
```

Between that closing brace and `switch_to` inside `schedule()`, the corpse is `Dead` in the thread
table and still standing on its own kernel stack. The supervisor it just woke can, on another
core, receive the death message and reclaim the child's region. That path is
`reap_supervised` -> `reclaim_region` -> `reap_region_objects`, whose refuse phase asks only about
`state`:

```rust
matches!(t.handshake.state, State::Ready | State::Running | State::Blocked)
```

`Dead` is none of those, so nothing refuses, the removal phase runs `Threads::remove`, that drops
the `Thread`, and `KernelStack::drop` unmaps all six pages with a real `tlbi`. The corpse's next
store faults.

The kernel already has the flag that answers this and the reap path never asks it.
`Handshake::on_cpu` means exactly "a core is standing on this thread's stack"; it is set until that
core's successor runs `finish_switch`, which is the two-part reaper's whole design and is documented
in those words: *"Dropping the `Thread` unmaps its stack and frees its address space, which is
exactly why it must not happen while any core still stands on it."* `finish_switch` obeys that.
`reap_region_objects` does not, because it reasoned from `state` and `Dead` genuinely does mean
"never runs again". Never runs again is not the same as off its stack, and that is the whole bug.

`Finished` and `Embryo` are removed by the same phase and have the same exposure; `Dead` is simply
the one a supervisor can reach on purpose, at speed, from another core.

## Why this test, why intermittent, and why only CI

`supervision_tests::a_faulting_child_reports_to_its_supervisor_and_is_reaped_then_respawned` is the
only place in the suite where a supervisor is woken by a corpse and reclaims that corpse's region
immediately, with four assertions between the `ipc_receive` and the `reclaim_region`. That is a race
between a few hundred instructions on the test's core and a few hundred on the corpse's. That is why
it is one run in six on a loaded 2-core runner and zero runs in 45 on an idle laptop. Every other
reap in the suite either goes through `wait_for` or reaps a thread that has long since switched
away.

It is also why no fix moved it. Shrinking `reap_region_objects` (#157), rebuilding the spawn path
(milestone 124), and moving the handler to a per-CPU interrupt stack all changed depth, and depth
was never the variable.

## What was ruled out, and how

- Thread-stack depth. `script/stack-depth-check` bounds the deepest chain at ~13.5 KiB against a
  24 KiB stack, the graph is acyclic, and no frame over the guard page is reachable from a thread
  entry point. Independently: slot 87's stack is *unmapped* in the fourth dump, and depth cannot
  unmap a page.
- A frame larger than the guard page. `script/stack-frame-check` gates every frame under 4096
  bytes on both ISAs and reports "no ratchet".
- A stray store from a fixed site. The address is forced by the vector walk's arithmetic, which
  this file derives above and which predicts `G` on aarch64 and `G`/`G+8` on riscv64 without any
  writer at all. The search for "the three places that can name a slot base" was answering a
  question that had no fault behind it.
- A pointer treating a stack top as inclusive. Same: the offsets are the walk's, not a
  neighbor's-top store's. The earlier reading of `x8 = 0xffff0010001b7a90` ("1392 bytes below slot
  87's top, so the stack has not run out") was the right objection to the depth story and is exactly
  what a use-after-free predicts: the corpse was *shallow* on its own stack when the pages vanished.
- An ordering bug in the weak memory model. Not needed. Every step here happens under `SCHED`
  on one side and outside it on the other; the window is plain mutual exclusion missing a
  precondition, not a reordering.

## The fix, and the one it is not

The fix is one clause: an out-of-band reaper refuses a thread that is still `on_cpu`, whatever
its state, and refuses it *without* arming a kill (it is already dead). The corpse is off its stack
one context switch later, so the caller's retry succeeds. `reclaim_region`'s contract is already
refuse-and-retry, and the supervision test's own last assertion already wraps a reclaim in
`wait_for`; the failing one now does the same.

The better fix, not taken here, is worth writing down. The window exists because a thread is
published as `Dead` before it is off its stack. Marking it `Departing` in `depart()` and promoting
it to `Dead` from `finish_switch` (which already holds `SCHED`, and which already runs at exactly
the instant the stack is free) would delete the window instead of refusing inside it. No caller
would ever see a transient refusal. That is a change to the death protocol and to `RunState`, which
lives in `crates/thread_wake_handshake` where loom searches the transitions, so it is a lane and a
decision of its own rather than a hotfix. See milestone 124's block.

## What this file's numbers cite

Glossed here because the split made this a file of its own, and `script/citations` asks each
file to say once what a number cites. Each gloss is the record's own title.

- milestone 124 (A thread is born where it lives: the spawn path's copies)
