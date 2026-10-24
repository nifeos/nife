# Object revocation: tearing a process back down

Retype from a memory region (milestone 19) let a process build its own kernel objects from EL0: an address
space, a TCB, a rendezvous, all carved out of a region it holds. What it could not do was
tear them back down. Every object permanently pinned its region, so the system could build
processes but never fully reap them, untenable for the thesis (run real workloads on real machines):
a workload that comes and goes must be able to leave.

This is that teardown. The assumed prerequisite was "retype to EL0" (already shipped); the real
one was reclamation.

## The model: region ownership, and generational staleness instead of a derivation tree

To reclaim an object you must guarantee no live capability can still reach it. seL4 does this with a
capability derivation tree (CDT): every copy/mint/grant records a parent-child edge, and revoking
a capability walks the subtree deleting every descendant. It is powerful (revoke one delegation, leave
its siblings) and it is the machinery milestone 19 (run a real workload) declined to build.

nife already had a different lever, and it decided the design:

- **Objects carry generational names.** A `ThreadControlBlock`, `Rendezvous`, or `AddressSpace` capability holds a
  `(generation, slot)` name into a registry (`crates/generational_table`, notes/generational-names.md). Free the
  registry slot and every outstanding capability to that object stops resolving, *forever*, because
  its generation no longer matches. Invalidation is a side effect of freeing the object, with no
  capability to hunt down: the milestone-14 machinery, reused for teardown.
- **Frames do not carry names.** A `PageFrame` capability holds a raw physical address, so frame
  revocation under §13 (untyped reclamation) must find and delete every matching capability, which is why frames need a
  revocation *log* and objects do not.

So the model is: **an object's lifetime is its backing region's lifetime, and generational names make
every delegated capability safely stale the moment the region is reclaimed.** Revocation is
region-ownership-scoped: you reclaim by destroying a region you own, which invalidates every object
in it at once. You cannot revoke "just the copy I handed to B" the way a CDT can. Where revocation
is authority the region owner retains over what it built, that coarseness is the right semantic. If
per-delegation revocation is ever wanted, a CDT earns its cost then.

How much is thrown away if we later add the CDT? Almost nothing. The per-object teardown (unblock
waiters, unbind an address space, free the registry slot) and the region reclamation are needed in
both models; the CDT only changes *when* a reclaim fires, not the teardown it fires. The CDT is a
purely additive layer on top (derivation-edge recording at delegation sites, a subtree-walk revoke).

## The trigger: explicit destroy by the owner, and why not auto-on-exit

The reclaim trigger is `MemoryRegion::DESTROY`, invoked by whoever holds the region's capability, not
automatic on thread exit. The reason is the capability model itself: **a region belongs to whoever
created it, not to the thread that happens to live in it.** The thread is an occupant; the owner holds
the untyped cap; the occupant never does. Reclamation destroys memory, which is an authority, and in
this kernel authority *is* a capability you hold. Letting an occupant's death free memory it holds no
capability to would reintroduce the ambient authority the design removes. Three consequences follow.
The owner is usually not done when the child exits (it may want to read a result or reuse the
space). The bump allocator can only free a whole region, so auto-on-exit could not be per-object
anyway. And an owner-driven loop (build, run, destroy, build again) is deterministic where
scattered reaper-time frees are not.

Thread *exit* still does the automatic half: the reaper tears down the thread's live state (drops it
from the run queue, frees its kmem kernel stack, runs its `Drop` chain). It does not free the *memory* of a
region-backed object, which waits for the owner's destroy. The split was
already latent in the code (the reaper recycled kmem TCB pages and explicitly left region-backed ones
"for region destroy").

## The lock structure forced the shape

`destroy` is reachable from inside `AddressSpace::Drop`, which runs inside `Threads::remove`, which the
reaper calls while holding `SCHED`. So if `destroy` (or the object reaping it triggers) took
`SCHED`, it would self-deadlock against the reaper. That single fact split the implementation:

- `reap_region_objects` (takes `SCHED`) is the caller-driven step. It scans the thread and rendezvous
  registries for objects whose backing page lies in the region. It refuses if any is still in use (a
  Ready/Running/Blocked thread, or a rendezvous with a blocked waiter), and otherwise tears the
  dead ones down. `reap_address_spaces_in_region` does the same for unbound address spaces under the address-space
  registry lock, a second lock domain, sequenced never nested.
- `memory_region::destroy` (never takes `SCHED`) is the memory half: it frees the region's frames and
  removes its slot. It stays `SCHED`-free the way it always was (its `revoke_region` takes the
  revocation lock, not `SCHED`, and `AddressSpace::Drop` forgets its own records first so the
  scheduler-taking cap-deletion path is never reached).

`sched::reclaim_region` sequences them: refuse-if-children, reap objects, reap aspaces, unpin, destroy.
It must run outside any `Drop`.

## Region subdivision: SPLIT, and the one honest tradeoff

For a spawner to reclaim per child, each child needs its own reclaimable region. `MemoryRegion::SPLIT`
carves pages off a parent region's unspent budget into a new child region (seL4's
untyped-retype-into-untyped). The child is independently reclaimable.

A naive version has a double-free trap: a child's pages are part of the parent's run, so if both the child and the parent were destroyed, the parent's
`destroy` would free the child's pages a second time. The fix without a CDT: a parent counts its
live children and refuses `destroy` while any remain (so it can never double-free a live child's
pages). A child returns its pages to the *parent*, not the allocator, when it is destroyed.

Return-of-pages is LIFO. A child destroyed at the top of the parent's watermark gives its pages
straight back to the parent's budget (the watermark un-bumps), so they are re-splittable. A
spawn-then-reap loop (split, run, destroy, split the next) therefore runs forever on a budget of one
child. The benchmark proves it: a 100-iteration spawn loop runs on a 64-page budget that could hold only ~6
children at once. A child freed *out of order* leaves a hole until no child above it is live, and the
last such return reclaims it, rather than the parent's death, which would leak the progenitor's job
pool (`notes/swish-check-flake.md`). Reusing a hole under a live child is what a
derivation tree buys, and we still do not build one.

## Region slots became generational, retiring the lifetime cap

The region table is a generational `generational_table::Table`: `destroy` removes a region's
slot, bumping its generation (so every stale `MemoryRegion` capability fails, the same stale-safety as
everything else), and the next `create` reuses it. The bound is on *concurrent* regions, which turns
a one-shot reclaim into a repeatable one. Not a fixed array whose slots are never reused, because
its entries would bound region *creations over the kernel's lifetime*, and a system that spawns and
reaps without end would wedge after 256 regions ever.

## Rendezvouses: wake a blocked waiter with an error

A rendezvous in a reclaimed region must be torn down too, or its page is freed while the registry
still points at it. Revoking one drains its wait queues: each blocked thread is popped
off (which frees its intrusive link), marked aborted, and woken, then the rendezvous is removed from the
registry, its generational name going stale. The woken thread's blocking `ipc_receive`/`ipc_send` returns
an error (the rendezvous is gone), so a waiter blocked forever cannot pin a region forever, and the
reclaim always makes progress.

### The sweep runs before the refusal, and that ordering is the whole of milestone-scale bug

The sweep runs *before* the live-thread refusal. A `Blocked` thread never reaches `schedule()`, so
it never spends the kill the refusal arms. With the sweep after the refusal, a region holding a
server parked in `RECEIVE` would be refused on every pass forever, its memory gone until the machine
stopped, and `DESTROY`'s documented contract ("the owner retries and reclaims") would be false. The
ordinary case is a server that blocks.

The aarch64 test boot shows the cost. `the_console_server_builder_brings_up_a_console_that_prints_for_it` builds a console
server out of the progenitor's 2048-frame budget, and that server blocks in its serve loop, so
without the sweep those 2048 frames are unreclaimable. With the sweep after the refusal, six such
tests left the boot with 216 free frames of 29307 and no free run longer than 117, failing as
`Unmappable(OutOfFrames)` in whichever later test asked for a long run. notes/frames.md is the full
receipt.

Sweeping first needs no new mechanism: removing a rendezvous already aborts and wakes its
waiters, the transition a doomed resident needs before it can die. The region's rendezvouses go on
every pass, refusal or not.

The objection worth answering is that this makes a *refused* reclaim destructive. It already was, and
`reclaim_region`'s BUGS section says so: a refusal arms §16's kill on every live resident. Ending the
region's rendezvouses in the same pass is the same commitment one object over, and the caller has by then
said the region is going away. It must not be a surprise, hence this note and a long comment at the
sweep.

The remaining limit: a service blocked on a rendezvous *not* in any region being reclaimed still
cannot be woken, so cannot be reclaimed. `user::holding::Holding` reports
that as a failed release, and the practical answer is to give a service's
rendezvouses a region of its own, which is what the net wiring now does.

The delicate part was the IPC core, the block-and-wake path where the lost-wakeup hang once lived.
Two things kept the hot path intact. First, `rendezvous_of` became fallible: a stale
`Rendezvous` capability (its rendezvous reclaimed out from under a holder) used to reach a name that always
resolved, so a miss panicked; now it returns `None` and the caller aborts cleanly. Second, the abort
is routed through a per-thread `ipc_aborted` flag rather than changing `ipc_receive`/`ipc_send`'s
return types (66 callers). The IPC primitive sets the flag inside its existing lock and the syscall
layer reads-and-clears it, so no extra lock lands on the fast path and the IPC benchmark does not move.
Kernel-side IPC callers never set the flag (their rendezvouses are never revoked), so they are untouched.

## What the tests prove

Each piece is nailed by a test that watches the free-frame count return exactly to baseline, the
witness that memory came back rather than leaked:

- an embryo TCB's region reclaims (the mechanism in isolation);
- an unbound address space's region reclaims (name stale, ASID freed);
- a started, run, and exited child's regions reclaim, and reclaim is *refused* while it is still live;
- `SPLIT` carves reclaimable children and commits the parent;
- 320 create+destroy cycles (past the old 256 cap) all succeed, proving slot reuse;
- an idle rendezvous's region reclaims, and a region whose rendezvous has a blocked waiter reclaims
  too, waking that waiter with its IPC aborted rather than refusing
  (`a_blocked_waiter_wakes_with_an_error_when_its_rendezvous_is_revoked`). An earlier draft of this line said it
  "refuses until the waiter is gone", which the test beside it has contradicted for some time.

## The surface, and what remains

New EL0 methods on the `MemoryRegion` object (the syscall boundary stays the three calls; these are methods
under `invoke`): `SPLIT` (subdivide) and `DESTROY` (reclaim). See `crates/abi` for the contract and
`design/decisions/` for the record.

The follow-ons this note once listed are all done: the EL0 spawn benchmark (`lat_proc`, notes/
benchmarks.md), LIFO return-of-pages-to-parent, and error-return for a blocked waiter (both above).
What is left is the general, non-LIFO case of return-to-parent, which a full CDT buys, and we
still have no reason to build one.

## DESTROY force-kills a runaway (milestone 28 (a solid terminal: the line discipline as a component), DECISIONS §16 (object revocation) amendment)

`DESTROY` used to refuse outright while a live thread occupied the region, which is correct for a
cooperative child but leaves the shell's `^C` escalation (§24) nothing to escalate to: a thread
spinning at EL0 would refuse `DESTROY` forever. The fix is small and avoids two hard problems (removing a node from the intrusive `Fifo`, and stopping a thread
running on another core):

- `DESTROY` on a live resident thread now marks it `killed` and still refuses that pass.
- `schedule()` converts a killed thread to a `Finished` corpse at its next preemption instead of
  requeueing it, so the ordinary reaper tears it down (stack, address space) just like a clean exit.
- The owner retries `DESTROY` (the shell already retries for the exit sliver); once the runaway is
  preempted and reaped, the retry reclaims the object-free region.

A runaway is preemptible by construction (§5), so **each core reaps its own killed thread on the
timer**: no cross-core IPI, no queue surgery, one branch in `schedule()` and one flag. Reclamation
waits one timeslice, the bounded escalation the shell wants, not a stop-the-world. A thread that only ever *blocks* is never scheduled
to hit that preemption, so it is the cooperative tier's job (it is listening on its interrupt
endpoint by definition), not the forcible tier's. Proven on both ISAs by
`destroy_force_kills_a_runaway_and_reclaims_its_region` (`kernel/src/user.rs`): a one-instruction EL0
runaway, reclaimed out from under itself.

## Two names for one region, and the double free that found it

Fixed 2026-08-18. `destroy_reclaims_a_region_whose_resident_is_blocked_in_receive` panicked once
in 45 full-suite runs on riscv64 under load, in milestone 62's acceptance run
(notes/load-sensitive-assertions.md):

```
[PANIC] panicked at crates/frames/src/lib.rs:315:9:
double free of frame 0x82a3e000
```

That is `Frames::free`'s deliberate assertion: a kernel that keeps running past a double free
corrupts memory elsewhere and blames innocent code. The note recorded it as unexplained, with the wake path as a hypothesis. **The hypothesis was
wrong**, and the real cause is one step earlier: the region had two owners, and neither knew it.

### The cause, and it takes two ingredients

**One. A user-built address space was freeing a region it did not own.** `user_aspace_create` (the
`RETYPE_OBJ(ASPACE)` engine, milestone 19b) builds a space *in a region the caller already holds an
`Untyped` capability to*, unlike `AddressSpace::new`, which carves its own. Both stored a bare
`u64`, and `AddressSpace::drop` called `untyped::destroy` on it unconditionally. So a region reached
by `Untyped::DESTROY` and one reached by a dying thread's address space were the same memory under
two names.

The old safety argument was the pin: `retype_object_page` pins the region, `destroy` refuses a
pinned region, and only `sched::reclaim_region` unpins. That holds for a drop *inside*
`reap_region_objects`, under `SCHED`, but not for the drop that matters. `sched::finish_switch`
hoists a dead thread's address space out of the table, releases `SCHED`, and drops it
afterwards, because `AddressSpace::drop` runs a §13 revocation sweep that takes `SCHED` itself. By
then `reclaim_region` may already have run `unpin`, and the refusal the argument relied on does not
happen.

**Two. `untyped::destroy` was not a single-winner claim.** It checked the region under the `REGIONS`
lock, released the lock, revoked, freed every page, and removed the slot *last*. Two callers that
both entered that window both passed the refusal check and both ran the free loop over the same
pages. That is the double free, arriving one frame at a time until `Frames::free` caught it.

Both were needed. Either alone is survivable: with one owner there is no second caller, and with
an atomic claim the loser finds a stale name and returns. Hence one run in 45, and two cores.

### The fix, both halves at rung one

`user::Backing` is a two-variant enum carrying the region name, `Owned` or `Lent`, so **an address
space cannot be constructed without saying who frees its region** and the borrower's `Drop` cannot
free memory it does not own, pin or no pin. And `untyped::destroy` now removes the slot under the
same lock hold that decided to destroy it, so the generation bump happens before anything is freed:
a second caller's name no longer resolves. That also makes a sentence already in the tree true,
`reap_supervised`'s "a racing reap got there first and the name is now stale", which described a
mechanism that did not yet exist.

The gate is `force_kill_tests::an_address_space_never_frees_a_region_it_was_lent`, and it is
deterministic: it stages the window by hand (`unpin`, then drop the space) instead of racing for
it. Verified it can fail: with the ownership guard removed it
returns four frames it was only lent and trips its own assertion, on aarch64, which also answers the
old note's question about whether the bug was riscv64-only. It was not; riscv64 is only where the
timing exposed it.

## BUGS

- **The regression gate proves the ownership half; the overlap half is loom's** (updated
  2026-08-18, milestone 135). The literal double free needs two `untyped::destroy` calls to overlap
  between the refusal check and the slot removal, and no test in this tree can schedule that. The
  deterministic test asserts the property that makes the overlap harmless (a lent region is never
  freed by its borrower) rather than the overlap itself. The single-winner claim is the whole of
  `RegionTable::claim_for_destroy` in `crates/memory_regions`, which decides and removes the name in one
  `&mut self` borrow, and `script/interleaving-check` searches every interleaving of it. The
  falsification is standing rather than remembered: one harness reconstructs the pre-fix protocol
  and passes only when loom finds its double free. See [interleaving.md](interleaving.md).

  **What is still argued rather than checked**, because a caveat replaced is not a caveat removed.
  Loom models C11 rather than aarch64 or riscv64. The model assumes mutual exclusion and says
  nothing about `IrqSafeMutex` delivering it or about the rank order being right. And it searches
  who wins the claim, not whether the winner then frees the right pages, which is `destroy_outcome`'s
  Kani proof and a separate argument.
