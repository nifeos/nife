---
status: AMENDED
raised: 2026-07-26
decided: 2026-07-26
ratified_by: calef
---

# 16. Object revocation: reclaim the objects a process built (extends §13)

(four amendment blocks below: milestones 31 and 28, the job pool's holes on 2026-10-03, and a running
root on 2026-10-05.)

<!-- writing-standards: exception. Marked 2026-10-03 (UTC) by the maintainer session. Reason: this change
adds a dated amendment that carries no bold of its own, and touches two existing lines only to point
at it. Bringing a 1,800-word section to 4 bold spans per 1,000 words is a rewrite for the section's
owner, not something to hide inside an at-merge record (the measured class is recorded in
design/roadmap/0586-a-prose-ratchet-in-lint.md). Remove this marker when that rewrite lands. -->

§13 revoked **frames**. This extends the same idea to **kernel objects** (TCBs, address spaces,
endpoints), so a process can be torn back down and its memory returned, the reclamation a
run-workloads-that-come-and-go system needs. Full reasoning in notes/object-revocation.md.

## The model

**Region ownership plus generational staleness, not a capability derivation tree.** An object's
lifetime is its backing region's lifetime; reclaiming a region frees each object's registry slot, and
because objects carry generational names (§14, `crates/slots`), every outstanding capability to them
goes stale on next use with no capability to hunt. This is coarse (reclaim a region, kill every object
in it at once; no per-delegation revocation) and that is the right authority semantic here. The CDT is
a later, purely-additive layer if fine-grained revocation is ever wanted; the rework to add it is
near-zero, because the per-object teardown and region reclamation it would call are what we build now.

## The trigger, and the lock constraint that shaped it

Reclamation is explicit: `Untyped::DESTROY`, invoked by the region owner (who holds the untyped
cap), never automatic on thread exit, because a region belongs to its owner, not to the thread that
occupies it, and reclaiming memory is an authority a capability system grants only through a capability
held. Thread exit does the live-state teardown; the owner's destroy does the memory. `destroy` must
never take `SCHED` (it is reachable from `AddressSpace::Drop` under the reaper's `SCHED`), so the
`SCHED`-taking object reap is a separate caller-driven step and `destroy` stays `SCHED`-free.

## Two new methods on the Untyped object (the surface stays three syscalls)

- `SPLIT` carves a child untyped off a parent's budget (seL4's untyped-retype-into-untyped), so a
  spawner gives each child its own reclaimable region. A parent with live children cannot be destroyed
  (freeing its run would double-free a child's pages), tracked by a child count so the parent becomes
  destroyable again once its children are gone. Return-of-pages is LIFO: a child destroyed at the
  top of the parent's watermark gives its pages back to the parent's budget (un-bump), which is exactly
  what a spawn-then-reap loop does, so a split parent is *not* committed for its lifetime; a child freed
  out of order leaves a hole until the parent itself is destroyed. This is the LIFO half of seL4's
  return-to-parent without the derivation tree that would handle the general case. (Amended
  2026-10-03: a hole is now reclaimed once nothing above it is held. See the last amendment.)
- `DESTROY` reclaims a region and every object retyped from it. Refuses (NotPermitted) while a live
  thread occupies it, an endpoint in it has a blocked waiter, or it has been split.

## Also

Region indices became generational (`destroy` reuses the slot), retiring the old cap where the
kernel could create only 256 regions in its whole lifetime. Endpoint revocation wakes a blocked
waiter with an error: revoking an endpoint drains its wait queues, marks each waiter aborted, and
wakes it, so its blocking `ipc_recv`/`ipc_send` returns an error (the endpoint is gone) rather than
stranding the reclaim or dangling on a freed page. `endpoint_of` became fallible so a stale endpoint
capability fails cleanly instead of panicking; the check folds into the existing IPC locks, so the
hot path does not regress. The EL0 `lat_proc` spawn benchmark also landed (notes/benchmarks.md):
nife builds a process faster than Linux or macOS, with the honest caveat that a
capability-microkernel process is a lighter object than a Unix one.

## Amendment (milestone 31): untyped becomes delegable by rights *inheritance*, with a delegable root

`SPLIT`'s child untyped was minted with `WRITE` alone (`cap::untyped_cap`), where every other
creation path gives its creator full rights: `RETYPE` mints a frame `READ|WRITE|GRANT`, `RETYPE_OBJ`
mints an endpoint, aspace, or TCB the same. Because `SEND_CAP` and `CAP_INSERT` both gate on `GRANT`,
that under-grant silently made untyped the one object type **no process could delegate**: a split
budget could be spent by its holder and handed to no one. That foreclosed "untyped budgets as
first-class grants," milestone 31's headline: a shell that endows a child N pages from its own budget
must delegate an untyped, and could not.

The first fix was wrong, and the way it was wrong is the interesting part. Minting the `SPLIT`
child `READ|WRITE|GRANT` unconditionally is a *rights escalation*. `SPLIT` gates only on `WRITE`, so a
process holding a deliberately `GRANT`-less untyped (one delegated to it spend-only) could `SPLIT` it
and receive a `GRANT`-bearing child over the same memory, manufacturing the very right its capability
withheld. That violates the model's derive-never-widens invariant, and it does so at a *fresh mint*
site the Kani proofs (which cover `derive`) do not reach.

**The right fix is rights inheritance, not a rights default.** Two coordinated changes:

1. A `SPLIT` child inherits **the invoking capability's rights, never more**
   (`untyped_cap_rights(child, cap.rights)` at the mint site). A spend-only untyped splits into
   spend-only children; `GRANT` is passed down only if the parent held it. This makes `SPLIT` honor
   derive-never-widens by hand, the same discipline `derive` enforces.
2. The root untyped the kernel hands init at boot becomes `READ|WRITE|GRANT`
   (`cap::untyped_root_cap`, at the three init-boot grant sites). Delegating budgets to the children
   it builds is init's job, so the root of the budget tree carries `GRANT`. This was the actual bug:
   the `WRITE`-only root, not the `SPLIT` default, is what left no delegable untyped anywhere and
   forced the escalating workaround.

Rights then narrow monotonically from the root down: root (`GRANT`) -> init's `SPLIT` (inherits
`GRANT`) -> `CAP_INSERT` into the shell (narrowed to `WRITE|GRANT`) -> shell's `SPLIT` (inherits) ->
`CAP_INSERT` into the spawned child (narrowed to `WRITE`, spend-only). `untyped_cap` (`WRITE` only)
stays the constructor for a spend-only leaf budget; nothing manufactures authority at any step.

A kernel test pins the invariant at the mint site (`syscall.rs`,
`split_inherits_the_parent_capabilitys_rights_never_widening`): a `GRANT`-less untyped splits into a
`GRANT`-less child that cannot be delegated, while the delegable root splits into delegable children.
This is a bug fix to this section's intent (untyped is delegable in seL4, the model we borrow
guarantees from), recorded here rather than as a new section. See `kernel/src/syscall.rs`'s `SPLIT`
handler, `cap::untyped_root_cap`, and notes/grant-expression.md.

## Amendment (milestone 28): DESTROY force-kills a live resident thread, it no longer only refuses

`DESTROY` refused (NotPermitted) while a live thread occupied the region, on the reasoning that "its
owner must let it finish first." That is right for a cooperative child, and wrong for the exact case
§24 built the forcible tier of `^C` for: **a runaway that never finishes.** A thread spinning at EL0,
never yielding and never checking its interrupt endpoint, would refuse `DESTROY` forever, so the
shell's escalation had nothing to escalate *to*. §24 named the forcible tier "§16's revocation" and
said "no new kernel primitive"; this is the small change to `DESTROY` that makes that true.

The refusal now arms a kill. When `DESTROY` finds a live (`Ready`/`Running`/`Blocked`) resident
thread, it marks it `killed` and still refuses this pass. A killed thread never runs again: the
scheduler converts it to a `Finished` corpse at its next preemption instead of requeueing it, and
the ordinary reaper tears down its stack and address space exactly as a clean exit would. So the
owner that retries `DESTROY` (the shell's escalation loop already retries, for the exit sliver)
reclaims the region once the runaway has been torn down.

**Why a flag and a retry, not a synchronous kill.** Yanking a thread out of a run queue needs an
arbitrary-remove the intrusive `Fifo` deliberately does not have, and stopping a thread `Running` on
another core needs a cross-core IPI and a rendezvous. The killed flag needs neither: a runaway is
preemptible by construction (DECISIONS §5), so each core converts its own killed thread on the
timer, and the whole mechanism is one branch in `schedule()` plus one flag in `DESTROY`. The cost
is that reclamation is not instantaneous (the runaway runs to the end of its timeslice, then dies),
which is exactly the semantics the shell wants: a bounded escalation, not a stop-the-world.

Scope, honestly. This tears down the runaway (`Running`/`Ready`), which is §24's stated target. A
thread that only ever *blocks* is never scheduled to hit that preemption, so the flag alone will not
reap it; that case is the cooperative tier's job (send the program its interrupt endpoint, which by
definition it is listening on), not the forcible tier's. A single kernel test builds a one-instruction
EL0 runaway and reclaims its region out from under it, on both ISAs (`user.rs`,
`destroy_force_kills_a_runaway_and_reclaims_its_region`). See `kernel/src/sched.rs` (`schedule`,
`reap_region_objects`) and `Thread::killed`.

## Amendment (2026-10-03): an out-of-order hole is reclaimed once nothing above it is held

Recorded at merge by the maintainer from [#1517](https://github.com/nifeos/nife/pull/1517)
(reclaim a region's out-of-order holes: the riscv64 progenitor out-of-memory), which changed the
semantics and asked for this record.

The original text said a child freed out of order leaves a hole until the parent itself is
destroyed. That leaked. The progenitor's job pool is a split parent that is never destroyed, and
every pipeline ends its producer first, so each `wc gate.txt | wc` stranded a region; on CI's
slower riscv64 runner the strays accumulated until `std_exerciser`'s 384-page region no longer fit
(`notes/swish-check-flake.md` has the trace and the reproduction).

Now, when a child returns to its parent, the parent's watermark drops past the LIFO un-bump to
the highest page anything still holds: a live child's end, or the parent's own retypes. So a hole
lasts only until nothing above it is held. The rule is `memory_regions::coalesced_watermark`, with a
Kani proof and a falsification patch.

Only when no sibling is tearing down. A sibling that has been claimed but not yet returned has
left the region table while its pages are still being revoked, and lowering the watermark under it
would let the parent carve pages a peer may still map, the use-after-free §13 (capability
revocation and untyped reclamation) exists to prevent.
The parent's child count drops at the return rather than at the claim, so a count above the live
children the scan finds means one is in flight, and then only the LIFO un-bump happens. The hole is
not lost: the next return with nothing in flight reclaims it.

No ABI change: no new method, number or refusal; `SPLIT` and `DESTROY` mean what they meant, and
a program only gets pages back sooner. What stays: a hole *under* a child that stays live is still
not reused, because a carve only bumps, and a phantom child left by a split refused for a full
table stops the reclaim for that parent for good. Both limits are recorded in the `BUGS` at
`RegionTable::return_to_parent` (`crates/memory_regions/src/table.rs`).

## Amendment (2026-10-05): a thread whose space is rooted in the region dies as a resident

Ruled by calef on 2026-10-05 (UTC), quoted: "A, queue it after #1665"
([comment on #1665](https://github.com/nifeos/nife/pull/1665#issuecomment-5988177177)), on the
options in `design/roadmap/0765-a-destroyed-region-cannot-free-a-running-root.md`. Recorded by the
maintainer.

`CONFIGURE` binds a space to a TCB without requiring both to come from one region. So a space can be
rooted in region R while its thread's TCB lives elsewhere, and `DESTROY(R)` reaped neither: it
handed the root back to the allocator while the thread still translated through it. The next owner
of that page would write the thread's top-level translations, which breaks fatal risk 7's
confinement claim outright.

Now a live thread whose bound address space has its root in the destroyed span is a resident of
that span, exactly as a thread whose TCB page is there. This pass refuses, the thread is marked
killed, and the retry reclaims, root included. A holder of R can therefore end a thread whose TCB
it does not own; it already owns the memory the thread translates through, so the thread could not
have kept running whatever the answer.

Refused: refusing without killing, because a borrower that never finishes would pin R forever, the
shape the amendment for milestone 28 (a solid terminal: the line discipline as a component) removed. Refused: making `CONFIGURE` reject a space rooted outside
the TCB's region, because it adds a refusal to the syscall surface and forbids a split a loader may
want. Refused: both together, because once the kill exists the `CONFIGURE` refusal buys no safety.

No ABI change: no new method, number or refusal. Not yet built; milestone 765 (a destroyed region cannot
free the root a running thread walks) builds it, after milestone 95 (an unmap primitive). Until then the hole stands as recorded at `revoke::revoke_region`.
