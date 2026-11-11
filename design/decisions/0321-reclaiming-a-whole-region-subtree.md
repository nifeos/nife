---
status: PROPOSED
raised: 2026-11-11
---

# 321. Reclaiming a whole region subtree

*(The section number, the title and every name below are provisional.)*

Raised by the per-subtree ceilings of #3417, which bound what one region subtree holds but do not
give it back. This file reads the region code on main and on that branch
(`nifeos/nife@0f9675218`).

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

No capability names the children any more, so no `DESTROY` can reach them, and the parent refuses
until they go. The pages, the region slots and the objects stay held until reboot.

#3417 gives every region a live count and a ceiling for each kind of kernel object and for child
regions. A creation refuses when the region or any ancestor is at its ceiling. One stranded subtree
can hold at most a sixteenth of its root's ceilings. The ancestors keep its counts, though, so a
parent that runs such programs one after another fills its own ceiling, and then nothing under that
parent can create that kind of object.

## The premise, checked in the tree

The gap is not only a hostile program's. Three places in the tree already meet it.

- **The supervision tree's own builder does steps 1 and 2.** `components/src/spawner.rs`
  (`build_labelled`) splits an instance region off its budget, builds the child in it, and deletes
  its capability to the region. The instance comes back only through `rendezvous::REAP` of its
  corpse, which reaches the region through the dead thread rather than a capability. While any
  instance lives, `DESTROY` of the spawner's budget refuses. [§40 (a supervisor's death is its
  subtree's death)](0040-no-reaper-of-last-resort.md) rests on "destroying `spawner`'s region
  reclaims `spawner` *and everything built from it*, in one act", and adds that when the supervision
  route closes "the memory is still reclaimed when the region is destroyed". Neither holds for a
  child built in a split region: the destroy refuses for as long as the child lives.
- **The shell's `--mem` budget.** [Milestone 933 (every job is built the way a supervised job
  is)](../roadmap/0933-every-job-is-built-the-way-a-supervised-job-is.md) records that a job which
  splits its budget leaves the budget unreclaimable for the boot. The ruling of 2026-11-01 (UTC) made
  that a contract for job authors that nothing enforces, and left recursive teardown unbuilt until a
  real program needs to split its budget. `release_budget` in `components/src/swish/jobs.rs` detects
  the case through `USAGE`'s `CHILDREN` record and prints one sentence.
- **A split refused for a full table** leaves a child count with no child behind it, so its parent
  refuses `DESTROY` forever ([region-split-on-a-full-table.md](../../notes/region-split-on-a-full-table.md)).
  The table state differs; the effect on the parent matches.

Reading `reclaim_region` also turned up a race that every option below has to close.
`reclaim_region` reaps the region's objects, unpins the region and then claims it. A holder of the
region's capability outside the region, on another core, can retype a kernel object into it after
the reap has scanned the tables and before the unpin. The unpin then clears that object's pin, the
claim succeeds, and the region frees a page that a live object occupies. A second holder can also
split or retype between the unpin and the claim. The claim then refuses, `destroy` returns nothing,
and `reclaim_region` still reports success. The analysis comes from reading the code; no test
reproduces it. The BUGS section of [region-reaping.md](../../notes/region-reaping.md) records it.

## What makes a subtree cheap to find here

Three facts from the code shape every option's cost.

- **A subtree is one physical span.** A child's run lies inside its parent's run, however deep the
  tree goes. `reap_region_objects`, `user::reap_address_spaces_in_region` and
  `revoke::revoke_region` already select by physical span, so one pass over the subtree root's span
  finds every object and mapping in every descendant. Finding the objects needs no tree walk at all.
- **The table keeps parent links and no child links.** Finding the descendants means scanning the
  region table (1,024 slots on #3417's branch) and walking each entry's parent chain.
  `RegionTable::spent`, which answers `USAGE` for a holder with `ENUMERATE` alone, already does
  exactly that scan under `REGIONS`.
- **The ceilings bound the subtree.** The progenitor's root gets each table's capacity less an
  eighth, and a split region gets a sixteenth of that. A region split below the progenitor's root
  therefore holds at most 56 regions, 14 threads, 28 rendezvous, 14 notifications, 14 timers, 14
  processes and 14 address spaces in its whole subtree, and its depth cannot pass 56. A root the
  kernel creates without ceilings has no such bound.

`REGIONS` is an `InterruptSafeMutex`, so interrupts stay masked on the holding core while it is held.
Its cost is a matter of how many parent links a hold follows.

## The options

### (a) Reclaiming a region reclaims its whole subtree

The holder of a region's capability ends everything below it in one act: every descendant region,
every object in any of them, every mapping of their pages. This is seL4's `Revoke` on an untyped and
Fuchsia's job kill, expressed as a region operation.

It has two shapes. **(a1)** changes `DESTROY` itself, so it no longer refuses a region with
children. **(a2)** adds a separate method on `MemoryRegion` (provisionally `DESTROY_SUBTREE`) and
leaves `DESTROY`'s refusal as it is.

The kernel's work, in order:

1. **Close the subtree.** Under one `REGIONS` hold, mark the subtree root as closing. Each object
   creation and each split already walks every ancestor inside `RegionTable::charge`, under the hold
   that carves the page, so a closing mark anywhere on the chain refuses it with no added walk.
   Plain frame retypes do not walk the chain, and need not: the final claim kills every name in the
   subtree before the revoke runs, so a frame retyped before it is revoked and one retyped after it
   fails. The same mark, set on a single region, closes the race above for today's `DESTROY` too.
2. **Reap the span.** `reap_region_objects` and the address-space sweep run over the root's span,
   unchanged, under `IPC_TABLES` and then the registry's lock. Each rescan for one victim at a time
   costs one pass over a kernel table, and the ceilings bound the victims to the same counts one
   region could already hold, so this step costs no more than reaping a full region does today.
3. **Claim the subtree.** Under one `REGIONS` hold, mark every live descendant (scan the table,
   follow each parent chain), then remove each marked entry and the root. The root's `live` counts
   already sum its subtree, so one walk up the ancestors gives the whole subtree back, the same walk
   `claim_for_destroy` makes now. For a region split below the progenitor's root, the two scans
   follow at most 1,024 × 57 parent links each. Under a root without ceilings the depth bound is the
   table, so a hold could follow close to a million links. That figure is arithmetic, not a
   measurement. Splitting the claim into one descendant per hold bounds each hold at one scan, at
   the price of more holds.
4. **Revoke and return.** One `revoke_region` over the root's span, outside `REGIONS`, and then the
   root's run goes back as it does today. The descendants' pages lie inside that run, so they need
   no return of their own and no `return_to_parent`.

No step recurses. Each loop in the table already runs at most `N` times, because a parent chain
longer than the table would be a cycle. Step 3 needs no scratch array, which matters on one of the
kernel's deepest frames, but the mark costs a field in `Region`. #3417 measured its Kani harnesses at
up to 3.6 GB, after a symbolic `pinned` flag had pushed the old model past 37 GB, so the new field
should go in with a measured harness run.

**Threads and address spaces.** Every thread in the span gets today's verdict
(`region_reap_verdict`). A thread that can still run has its kill armed and the call refuses; the
holder retries, as `DESTROY` callers already do. A blocked thread ends in place. A thread mid-switch
refuses without arming. Address spaces whose root lies in the span go with the registry sweep. A
refused pass leaves the subtree closed, as Fuchsia's job stays in its killing state, so a retry
cannot race a new object into it. Callers outside the subtree that wait on its rendezvous get
`Gone`, which `reclaim_region`'s BUGS already describes for one region.

**Ceilings.** Clean. A subtree reclaim returns every count it held in one ancestor walk, and the
ceilings bound how much work one call does.

**What it costs in the ABI.** (a1) changes what `DESTROY` means for every caller. A caller that
counts on the refusal loses it: `release_budget` reads it as "the job left children",
`memory_grant_reclaim_tests` asserts it, and a builder that destroys its own budget while a child it
lent still serves someone would now end that child. (a2) adds one method and changes nothing that
exists.

### (b) Deleting the last capability to a region reclaims it

When the last capability naming a region goes, the kernel reclaims the region. seL4 destroys an
object when its final capability goes, and EROS-style systems do the same for a bank's objects.

The kernel's work:

- **A count nife does not keep.** Capabilities live in every thread's table, in every process's
  table and in a thread's parked `outgoing_capability`. A count per region would have to move at
  every `derive`, `insert`, `put`, `delete` and `delete_matching`, at every capability transfer over
  IPC, and wherever a thread or process frees its whole table. That is a reference count on the
  capability path, which nife does not have, and every one of those sites is a place to be wrong.
  The other way, a sweep of every table on each delete, costs what the revocation sweeps cost per
  frame, on every delete of a region capability.
- **A deferred reclaim.** The last delete often happens where a reclaim cannot run: inside
  `Threads::remove` under `IPC_TABLES`, or inside a `Drop`. `reclaim_region` must take `IPC_TABLES`
  and run outside any `Drop`, so the delete would queue the region for a kernel reaper thread, which
  is a new kernel mechanism and a new place for a reclaim to wait.
- **The kernel's own names.** A thread's `thread_control_block_region`, an address space's `Lent`
  backing and the supervised reap reach a region by its bare name, not by a capability. The spawner's
  instances are the live case: the spawner deletes its last capability right after `START`, so (b)
  would reclaim the instance before it ran, unless those names counted too.

**Threads and address spaces.** A running resident refuses the reclaim, so the reaper retries until
the kill lands, holding the queue entry. Deleting a capability becomes destructive: a program that
drops its only capability to its own heap region kills itself. Today
`capability::CapabilityTable::delete` documents the opposite: "The object may still exist; **we
simply can no longer name it.**"

**Ceilings.** Each region returns its own counts at its own reclaim, so nothing changes in the
ceilings. A queue of regions waiting to be reaped holds its counts until the reaper runs.

**Authority.** §16 chose an explicit trigger on purpose: reclaiming memory is an authority, and a
capability system grants it through a capability someone holds and invokes. (b) makes it fire on the
absence of any holder.

### (c) Leave it as it stands

`DESTROY` keeps refusing, the 2026-11-01 contract keeps covering the shell's jobs, and the ceilings
keep confining the damage to one parent's share.

The kernel pays nothing. The costs fall elsewhere: a parent that runs stranded programs one after
another loses its ceiling for the boot; §40's cascade holds only for supervisors that never split;
the spawner's budget stays unreclaimable while an instance lives; and the race above still needs a
fix of its own.

### (d) A budget that withholds a new right cannot split

A narrower budget closes the gap at its source: add a right that `SPLIT` requires, and leave it out of
`abi::rights::SPEND_ONLY`. A leaf that cannot split cannot strand a subtree. This tightens now and
relaxes later compatibly, which is restrictive-first's own shape. It loses as an answer to this
question because it does not reach the builders that split by design (the spawner, the shell, the
progenitor) and leaves §40 and the race untouched. It also spends one bit of the five-bit rights
word. It stays on the table as a separate narrowing.

## Prior art

Each was read where named, on 2026-11-11 (UTC), unless marked recalled.

- **seL4** (Reference Manual 17.0.0, §2.4.1, §3.2 and §10.1.9; `src/object/cnode.c` at
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
  stack. A killing job accepts no new children. Job height is capped (`kRootJobMaxHeight` is 32). A
  started process outlives its last handle; its threads keep it alive (`on_zero_handles`).
- **EROS** (Shapiro, Smith and Farber, *EROS: a fast capability system*, SOSP 1999, §5.1). Space
  banks form a hierarchy, each drawing storage from its parent. Destroying a bank deallocates the
  objects of that bank and all its sub-banks or returns them to the parent, and the paper names
  destroying a subsystem's bank as the way to make sure the subsystem is dead. KeyKOS's space bank is
  the ancestor (recalled, not read: the extracted KeyKOS paper text did not reach that section).
- **Genode** (Genode Foundations 19.05, "Resource trading"). A parent destroys a child by closing its
  sessions. Destroying a PD session destroys every dataspace allocated from it and returns its
  remaining budget to the reference account. Children subdivide their own budgets for grandchildren,
  and that the parent's destruction of a child reaches its grandchildren is this file's inference
  from that structure, not a sentence in the book.
- **Linux cgroup v2** (kernel documentation, `cgroup-v2.rst`). `rmdir` removes only a cgroup with no
  children and no live processes, today's `DESTROY`. Writing `1` to `cgroup.kill` kills every process
  in the cgroup and all its descendants, and leaves the removal to a later `rmdir`.
- **Redox** (recalled, not read). Its kernel owns process memory and frees it at exit; it has no
  user-held region tree, so it offers nothing to compare here.

## Where nife differs

- seL4 finds a subtree's objects through the derivation tree. nife finds them by physical span,
  because a child's run lies inside its parent's. Option (a) inherits seL4's semantics without its
  bookkeeping.
- seL4 bounds a revoke by preemption and restart. nife has no restartable kernel operation; the
  ceilings bound the work instead, and the region holder's retry, already the `DESTROY` protocol,
  plays the part a restart plays in seL4.
- Fuchsia and seL4 both count references (handles, capabilities). nife does not, which is what makes
  (b) expensive here when it is cheap there.
- Fuchsia caps the job tree's height at 32. #3417's region ceiling caps a split subtree at 56
  regions, and so its depth, but a root without ceilings is unbounded.

## A concrete example

A parent region P sits directly under the progenitor's root, so its subtree may hold 56 regions, 28
rendezvous and 14 notifications. A supervisor splits a 64-page budget B from P and runs job J in it.
J splits three 8-page regions C1, C2 and C3 from B, creates one rendezvous and one notification in
each, deletes its three capabilities to them and exits.

- **(c), today.** The supervisor's `DESTROY` of B refuses. P's subtree keeps 4 regions, 3
  rendezvous and 3 notifications, and 64 pages, for nothing. After four such jobs P holds 12
  notifications. The fifth job's second notification takes P to 14, its third refuses, and from then
  on every program under P gets `OutOfMemory` for a notification. The kernel's refusal line names P
  as the region at its ceiling.
- **(a2).** The supervisor calls `DESTROY_SUBTREE` on B. The kernel closes B, ends the three
  rendezvous and three notifications in one pass over B's 64-page span, removes C1, C2, C3 and B
  from the table, gives P back 4 regions, 3 rendezvous and 3 notifications in one walk, and returns
  B's run to P.
- **(b).** Each of J's three deletes was the last capability to its region, so the kernel queues C1,
  C2 and C3 for its reaper. The supervisor's `DESTROY` of B succeeds once the reaper has run, and
  refuses until then. In the spawner, the same rule would reclaim each instance at `START`.

## Recommendation

**(a2): a separate method that reclaims a region's whole subtree, with the closing mark on every
reclaim.** Refuse (b). Keep (d) as a narrowing to weigh on its own.

The restrictive-first reading, option by option:

- (b) is the loosest. It turns a delete into a destruction and adds a trigger that no holder pulls,
  and once programs count on it, taking it back breaks them.
- (a2) adds the one authority §40 and §16 already assume a region holder has: the holder of a run
  can end everything built in it. It changes nothing a current caller does. Folding it into
  `DESTROY` later, if the separate method proves redundant, relaxes a refusal and breaks nobody;
  splitting (a1) apart later would.
- (c) keeps the surface narrowest, and if the gap were a hostile program's alone, the ceilings would
  make it a fair answer. The spawner shows the gap in the supervision tree itself, and §40's
  guarantee does not hold while it stands.

Would this still be the choice if every option cost the same? Yes. At equal cost (a1) and (a2)
differ only in reversibility, and (a2) is the reversible one; (c) loses on §40's guarantee, not on
effort.

## Reversibility, and what waits

- (a2) is additive: a new method under `invoke`, with no new syscall number. Nothing has acted on
  it. The method's name and number are an architect's call like any syscall-surface addition.
- (a1) changes a method every region holder already calls.
- (b) is the one-way door here: a delete that destroys becomes something programs rely on.
- (c) is where the tree is today.

Nothing is blocked until this is answered. Waiting on it: milestone 933's recursive-teardown
follow-on, §40's guarantee for supervisors that build in split regions, and the shape of the fix for
the race in `reclaim_region`, which the closing mark of (a) would provide and which a smaller fix can
close alone.
