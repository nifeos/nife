---
status: DECIDED
raised: 2026-11-11
decided: 2026-11-11
ratified_by: calef
---

# 321. Reclaiming a whole region subtree

*(The section number and title are provisional until the merge lands them. The new method's name
and number are provisional and the architect's to rule.)*

Raised by #3417's per-subtree ceilings, which bound what a region subtree holds but do not give it
back. The code read is main and that branch (`nifeos/nife@0f9675218`).

## Background

A memory region is a run of physical pages that a program holds a capability to. The program
retypes pages out of it into frames and kernel objects: threads, rendezvous, notifications, timers,
processes and address spaces. `SPLIT` carves part of a region into a child region, and the child's
pages lie inside the parent's run. `DESTROY` (`sched::reclaim_region`) tears down every object whose
page lies in the region, revokes every mapping of its pages and returns the run: a root's pages go to
the frame allocator, a child's go back to its parent. [§16 (object
revocation)](0016-object-revocation.md) holds that model. No capability derivation tree exists:
objects carry generational names, so freeing an object makes every capability to it stale.

`DESTROY` refuses a region with live children (`ReclaimRefusal::HasChildren`), because freeing the
parent's run would free pages a child still owns. A program can therefore leave a subtree that
nobody can reclaim:

1. It splits child regions from its own region and creates objects in them.
2. It deletes its capabilities to the children, or exits holding them.
3. Its supervisor calls `DESTROY` on the program's region and gets `NotPermitted`, on every retry.

No capability names the children any more, so nothing can reclaim them, and the parent refuses
until they go. Their pages, region slots and objects stay held until reboot.

#3417 gives every region a live count and a ceiling for each kind of kernel object and for child
regions. A creation refuses when the region or any ancestor is at its ceiling. One stranded subtree
can hold at most a sixteenth of its root's ceilings. The ancestors keep its counts, though, so a
parent that runs such programs one after another fills its own ceiling, and then nothing under that
parent can create that kind of object.

## The decision

Option (a2), ruled 2026-11-11 (UTC), and not yet built:

- A separate `MemoryRegion` method reclaims a region and everything below it: every descendant
  region, every object in any of them and every mapping of their pages. Its name and method number
  stay provisional (`DESTROY_SUBTREE` here) until the architect rules on them.
- `DESTROY` keeps its refusal of a region with live children.
- Every reclaim, through either method, first marks the region closing, so no object creation or
  split lands in it while the reclaim runs.

## The premise, checked in the tree

Three places in the tree already meet the gap.

- The supervision tree's own builder does steps 1 and 2. `components/src/spawner.rs`
  splits an instance region off its budget, builds the child in it, and deletes
  its capability to the region. The instance comes back only through `rendezvous::REAP` of its
  corpse, which reaches the region through the dead thread rather than a capability. While any
  instance lives, `DESTROY` of the spawner's budget refuses. [§40 (a supervisor's death is its
  subtree's death)](0040-no-reaper-of-last-resort.md) rests on "destroying `spawner`'s region
  reclaims `spawner` *and everything built from it*, in one act", and adds that when the supervision
  route closes "the memory is still reclaimed when the region is destroyed". Neither holds for a
  child built in a split region: the destroy refuses for as long as the child lives.
- The shell's `--mem` budget. [Milestone 933 (every job is built the way a supervised job
  is)](../roadmap/0933-every-job-is-built-the-way-a-supervised-job-is.md) records that a job which
  splits its budget leaves the budget unreclaimable for the boot. The ruling of 2026-11-01 (UTC) made
  that a contract for job authors that nothing enforces, and left recursive teardown unbuilt until a
  real program needs to split its budget. `release_budget` (`components/src/swish/jobs.rs`) prints
  one sentence when it sees the case.
- A split refused for a full table leaves a child count with no child behind it, so its parent
  refuses `DESTROY` forever ([region-split-on-a-full-table.md](../../notes/region-split-on-a-full-table.md)).
  The table state differs; the effect on the parent matches.

Reading `reclaim_region` also turned up a race. It reaps the region's objects, unpins the region
and then claims it. A holder of the
region's capability outside the region, on another core, can retype a kernel object into it after
the reap has scanned the tables and before the unpin. The unpin then clears that object's pin, the
claim succeeds, and the region frees a page that a live object occupies. A second holder can also
split or retype between the unpin and the claim. The claim then refuses, `destroy` returns nothing,
and `reclaim_region` still reports success. No test reproduces it. The BUGS section of [region-reaping.md](../../notes/region-reaping.md) records it.

## What makes a subtree cheap to find here

Three facts from the code shape every option's cost.

- A subtree is one physical span. A child's run lies inside its parent's run, however deep the
  tree goes. `reap_region_objects`, `user::reap_address_spaces_in_region` and
  `revoke::revoke_region` already select by span, so one pass over the root's span finds every
  object and mapping in the subtree, with no tree walk.
- The table keeps parent links and no child links. Finding the descendants means scanning the
  region table (1,024 slots on #3417's branch) and walking each parent chain.
  `RegionTable::spent` already makes that scan under `REGIONS` for any `USAGE` caller.
- The ceilings bound the subtree. The progenitor's root gets each table's capacity less an
  eighth, and a split region gets a sixteenth of that. A region split below the progenitor's root
  therefore holds at most 56 regions, 14 threads, 28 rendezvous, 14 notifications, 14 timers, 14
  processes and 14 address spaces in its whole subtree, and its depth cannot pass 56. A root the
  kernel creates without ceilings has no such bound.

`REGIONS` is an `InterruptSafeMutex`: the holding core keeps interrupts masked for the whole hold,
so what a hold costs is how many parent links it follows.

## The options

### (a) Reclaiming a region reclaims its whole subtree

The holder of a region's capability ends everything below it in one act: every descendant region,
every object in any of them, every mapping of their pages. This is seL4's `Revoke` on an untyped and
Fuchsia's job kill, expressed as a region operation.

It has two shapes. (a1) changes `DESTROY` itself, so it no longer refuses a region with
children. (a2) adds a separate method on `MemoryRegion` (provisionally `DESTROY_SUBTREE`) and
leaves `DESTROY`'s refusal as it is.

The kernel's work, in order:

1. Close the subtree: under one `REGIONS` hold, mark its root as closing. Every object creation
   and split already walks each ancestor in `RegionTable::charge`, under the hold that carves the
   page, so the mark refuses them at no added cost. Frame retypes need no check, because the claim
   kills every name before the revoke. The same mark on a single region closes the race above.
2. Reap the span: `reap_region_objects` and the address-space sweep run over the root's span,
   unchanged. The ceilings bound the victims to what one region could already hold, so this costs
   no more than reaping a full region today.
3. Claim the subtree: under one `REGIONS` hold, mark every live descendant, then remove the marked
   entries and the root. The root's `live` counts already sum its subtree, so one ancestor walk
   gives everything back, as `claim_for_destroy` does now. Below the progenitor's root each scan
   follows at most 1,024 × 57 parent links. Under a root without ceilings a hold could follow close
   to a million; that is arithmetic, not a measurement. One descendant per hold bounds each hold at
   one scan.
4. Revoke and return: one `revoke_region` over the span outside `REGIONS`, then the root's run goes
   back as today. The descendants' pages lie in that run and need no `return_to_parent`.

No step recurses: each table loop stops after `N` steps, since a longer chain would be a cycle, and
step 3 needs no scratch array. The mark costs a field in `Region`. #3417's Kani harnesses peak at
3.6 GB, and a symbolic flag in `Region` once took the model past 37 GB, so the field needs a
measured harness run.

Threads and address spaces. Every thread in the span gets today's verdict
(`region_reap_verdict`). A runnable thread has its kill armed and the call refuses; the holder
retries, as `DESTROY` callers do. A blocked thread ends in place. Spaces rooted in the span go with
the registry sweep. A refused pass leaves the subtree closed, as a Fuchsia job stays killing, so no
new object races in. Waiters outside the subtree get `Gone`, as `reclaim_region`'s BUGS describes.

Ceilings. One ancestor walk returns every count, and the ceilings bound the work of one call.

The ABI. (a1) changes what `DESTROY` means for every caller. `release_budget` reads the refusal as
"the job left children", `memory_grant_reclaim_tests` asserts it, and a builder destroying its
budget would now end a child it lent. (a2) adds one method and changes nothing that exists.

### (b) Deleting the last capability to a region reclaims it

When the last capability naming a region goes, the kernel reclaims the region. seL4 destroys an
object when its final capability goes, and EROS-style systems do the same for a bank's objects.

The kernel's work:

- A count nife does not keep. Capabilities live in thread tables, process tables and a thread's
  parked `outgoing_capability`. A per-region count would move at every `derive`, `insert`, `put`,
  `delete`, `delete_matching` and IPC transfer, and wherever a table dies. Each site is a place to
  be wrong. The alternative is a sweep of every table on each delete.
- A deferred reclaim. The last delete often happens inside `Threads::remove` under `IPC_TABLES`,
  or inside a `Drop`, where `reclaim_region` cannot run. So the delete would queue the region for a
  new kernel reaper thread.
- The kernel's own names. A thread's `thread_control_block_region`, an address space's `Lent`
  backing and the supervised reap reach a region by its bare name, not by a capability. The spawner's
  instances are the live case: the spawner deletes its last capability right after `START`, so (b)
  would reclaim the instance before it ran, unless those names counted too.

Threads and address spaces. A running resident refuses the reclaim, so the reaper retries until
the kill lands, holding the queue entry. Deleting a capability becomes destructive: a program that
drops its only capability to its own heap region kills itself. Today
`capability::CapabilityTable::delete` documents the opposite: "The object may still exist; we
simply can no longer name it."

Ceilings. Each region returns its own counts at its own reclaim. A region waiting in the reaper's
queue keeps its counts until the reaper reaches it.

Authority. §16 chose an explicit trigger on purpose: reclaiming memory is an authority, and a
capability system grants it through a capability someone holds and invokes. (b) makes it fire on the
absence of any holder.

### (c) Leave it as it stands

`DESTROY` keeps refusing, the 2026-11-01 contract keeps covering the shell's jobs, and the ceilings
keep confining the damage to one parent's share.

The kernel pays nothing. The costs fall elsewhere. A parent that runs stranded programs one after
another loses its ceiling for the boot. §40's cascade holds only for supervisors that never split,
and the spawner's budget stays unreclaimable while an instance lives. The race above still needs a
fix of its own.

### (d) A budget that withholds a new right cannot split

Add a right that `SPLIT` requires and leave it out of `abi::rights::SPEND_ONLY`, so a leaf cannot
strand a subtree. It tightens now and relaxes later compatibly, restrictive-first's own shape. It
loses here because the builders that split by design (the spawner, the shell, the progenitor) keep
the gap, and §40 and the race stay as they are. It also spends a bit of the rights word.

## Prior art

This lane read each source where named, on 2026-11-11 (UTC), unless marked recalled.

- **seL4** (Reference Manual 17.0.0, sections 2.4.1, 3.2 and 10.1.9; `src/object/cnode.c` at
  `seL4/seL4@c98d70adf`). Retyping an untyped makes the new capabilities its children in the
  capability derivation tree. To reuse a region, user code calls `seL4_CNode_Revoke` on the original
  untyped capability, which deletes every capability derived from it. Deleting the final capability
  to an object destroys the object. An untyped with children refuses a reset with `seL4_RevokeFirst`:
  the same refusal `DESTROY` makes today. `cteRevoke` deletes one child per iteration and calls
  `preemptionPoint()` after each, so a long revoke yields and restarts instead of holding the kernel.
- **Fuchsia** (zx_task_kill and zx_job_create references; `zircon/kernel/object/job_dispatcher.cc`
  and `process_dispatcher.cc` at `fuchsia@a42bac6ad`). `zx_task_kill` on a job "kills the given
  process or job and its children recursively". `JobDispatcher::Kill` moves each job to `KILLING`,
  takes its children into lists, and walks the lists iteratively, so the kill never recurses on the
  stack. A killing job accepts no new children. A job tree's height stops at `kRootJobMaxHeight`, 32. A
  started process outlives its last handle; its threads keep it alive (`on_zero_handles`).
- **EROS** (Shapiro, Smith and Farber, *EROS: a fast capability system*, SOSP 1999, section 5.1). Space
  banks form a hierarchy, each drawing storage from its parent. Destroying a bank deallocates the
  objects of that bank and all its sub-banks or returns them to the parent, and the paper names
  destroying a subsystem's bank as the way to make sure the subsystem is dead. KeyKOS's space bank is
  the ancestor (recalled, not read: the extracted KeyKOS paper text did not reach that section).
- **Genode** (Genode Foundations 19.05, "Resource trading"). A parent destroys a child by closing its
  sessions. Destroying a PD session destroys every dataspace allocated from it and returns its
  remaining budget to the reference account. Children subdivide their own budgets for grandchildren,
  so destroying a child reaches its grandchildren. That last step is this file's inference, not a
  sentence in the book.
- **Linux cgroup v2** (kernel documentation, `cgroup-v2.rst`). `rmdir` removes only a cgroup with no
  children and no live processes, today's `DESTROY`. Writing `1` to `cgroup.kill` kills every process
  in the cgroup and all its descendants, and leaves the removal to a later `rmdir`.

## Where nife differs

- seL4 finds a subtree's objects through the derivation tree. nife finds them by physical span,
  because a child's run lies inside its parent's. Option (a) inherits seL4's semantics without its
  bookkeeping.
- seL4 bounds a revoke by preemption and restart. nife has no restartable kernel operation. The
  ceilings bound the work instead, and the holder's retry, already the `DESTROY` protocol, stands in
  for the restart.
- Fuchsia and seL4 both count references; nife does not, so (b) costs far more here.
- Fuchsia caps the job tree's height at 32. #3417's region ceiling caps a split subtree at 56
  regions, and so its depth, but nothing bounds a root without ceilings.

## A concrete example

A parent region P sits directly under the progenitor's root, so its subtree may hold 56 regions, 28
rendezvous and 14 notifications. A supervisor splits a 64-page budget B from P and runs job J in it.
J splits three 8-page regions C1, C2 and C3 from B, creates one rendezvous and one notification in
each, deletes its three capabilities to them and exits.

- (c), today. The supervisor's `DESTROY` of B refuses. P's subtree keeps 4 regions, 3
  rendezvous and 3 notifications, and 64 pages, for nothing. After four such jobs P holds 12
  notifications. The fifth job's second notification takes P to 14, its third refuses, and from then
  on every program under P gets `OutOfMemory` for a notification. The kernel's refusal line names P
  as the region at its ceiling.
- (a2). The supervisor calls `DESTROY_SUBTREE` on B. The kernel closes B and ends the three
  rendezvous and three notifications in one pass over B's 64-page span. It removes C1, C2, C3 and B
  from the table, gives P back 4 regions, 3 rendezvous and 3 notifications in one walk, and returns
  B's run to P.
- (b). J's three deletes queue C1, C2 and C3 for a reaper, and `DESTROY` of B refuses until it
  runs.

## Why (a2)

The decision refuses (b) and leaves (d) as a separate narrowing. The restrictive-first
reading, option by option:

- (b) is the loosest. It turns a delete into a destruction and adds a trigger that no holder pulls,
  and once programs count on it, taking it back breaks them.
- (a2) adds the one authority §40 and §16 already assume a region holder has: the holder of a run
  can end everything built in it. It changes nothing a current caller does. Folding it into
  `DESTROY` later, if the separate method proves redundant, relaxes a refusal and breaks nobody;
  splitting (a1) apart later would.
- (c) keeps the surface narrowest. Against a hostile program alone, the ceilings would make it fair.
  The spawner puts the gap in the supervision tree itself, and §40's guarantee fails while it stands.

At equal cost the choice stands. (a1) and (a2) differ only in reversibility, and (a2) is the
reversible one; (c) loses on §40's guarantee, not on effort.

## What follows

- (a2) is additive: a new method under `invoke`, with no new syscall number. Its name and number
  are the architect's, and nothing builds on the provisional spelling.
- Building it serves milestone 933's recursive-teardown follow-on and §40's guarantee for
  supervisors that build in split regions.
- The closing mark is also the fix for the race in `reclaim_region` that
  [region-reaping.md](../../notes/region-reaping.md)'s BUGS records.
