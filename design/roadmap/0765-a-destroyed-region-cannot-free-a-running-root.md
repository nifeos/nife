---
status: NOT-STARTED
raised: 2026-10-05
milestone_dependencies: 763, 95
decision_dependencies: 16
machine_requirements: none
specific_machine: none
needs_person: no
---
# 765. A destroyed region cannot free the root a running thread walks

Raised by the page-tables-outlive-destroy lane on 2026-10-05 (UTC), while cutting intermediate page
tables out of live walks when their region is destroyed, in milestone 763 (a destroyed region cannot take a
live space's page tables with it). The root is the one table
that cannot be cut, so it needed a different answer, and every answer changes what
`MemoryRegion::DESTROY` does to a thread.

**Decided.** calef ruled option A on 2026-10-05 (UTC), quoted: "A, queue it after #1665"
([comment on #1665](https://github.com/nifeos/nife/pull/1665#issuecomment-5988177177)). The record
is §16 (object revocation)'s amendment of that date. B, C, and A plus C were refused; the reasons
are under the options below, which are kept as the record of what was weighed. Promoted from
`design/roadmap/proposals/` to this milestone by the maintainer at the ruling's merge.

It is sequenced after milestone 95 (an unmap primitive). Both touch `revoke` and `sched`'s reap
path, and 95's lane is in flight (#1678); building this beside it would be two lanes in one file.

## Acceptance

- `reap_region_objects`' finish and refuse phases treat a live thread whose bound space's root lies
  in the destroyed span as a resident: this pass refuses, the thread is marked killed, and the
  retry reclaims the region, root included. No new method, number or refusal on the syscall
  surface.
- A kernel test builds the shape (space rooted in region R, bound to a TCB from a second region),
  destroys R, and proves the thread never runs again on R's root and R's pages come back. It passes
  on aarch64, riscv64 and x86_64.
- A falsification patch under `system_tests/falsifications/` that drops the root test from the
  reap replays red at the test's assertion.
- `revoke::revoke_region`'s `BUGS` entry for the root and milestone 763's matching `BUGS` line are
  removed or rewritten to what remains.

Reuse: none exists to take; this is §16's teardown path in the kernel, where §46 (thin
primitives or whole subsystems) says write it. Each option below reuses an existing mechanism (A the
resident kill, C the `CONFIGURE` refusal shape).

## What §249's build changed underneath this, 2026-10-05 (UTC)

Recorded by milestone 95 (an unmap primitive)'s lane, `lane/space-naming-build`. It built §249 (a
running address space stays nameable) and was asked to confirm or refute the corpse gap that
`notes/naming-a-running-address-space.md` found by reading. Nothing below is built for this
milestone; it is the ground this milestone now builds on.

The registry owns every space now, so `user::reap_address_spaces_in_region` sees bound spaces. It
asks `sched::with_binders` about each bound thread, under one hold of `IPC_TABLES`. It takes an
unbound space whose root is in the span. It takes a bound space whose thread is gone from the
thread table, wherever its root is. And it takes a bound space whose root is in the span and whose
thread is a corpse no core stands on. It leaves every bound space whose thread can still run.

The corpse gap was real, and that ownership closes it. This was driven, not reasoned:
`running_space_tests::a_corpse_does_not_keep_a_space_rooted_in_a_destroyed_region` builds the
shape (space from R, TCB from A, supervised, faulted and left `Dead`) and destroys R. It asserts the
space is gone and the revocation registry no longer names the root. Its falsification puts the old
behavior back and goes red there.

What remains is this milestone's, and narrower than the hole below. The sweep leaves these in
place while R's pages go back:

1. A live thread rooted in R, which is the hole this block was raised for.
2. A bound embryo rooted in R. A later `START` would run it on a freed root, so the
   refuse-and-kill should count embryos too.
3. A corpse still on its core, which still has its root installed. That is `RefuseStanding`'s
   shape.
4. A race the sweep cannot see. A reaper that has taken a corpse's space out of the registry may
   still be dropping it when `DESTROY(R)` runs, and the drop's `forget_root` can then land after R's
   pages are free (`sched::reap_switched_out`'s `BUGS`).

Making `reap_region_objects` refuse while any of the four holds would close them all in the one
place this milestone already changes. The bound mark on each registry entry names the thread to ask
about.

## The hole

`RETYPE_OBJ(ADDRESS_SPACE)` retypes a space's root out of region R. `CONFIGURE` binds that space to
a TCB, and nothing requires the TCB to come from R. `sched::reclaim_region(R)` reaps threads whose
TCB page is in R and unbound spaces whose root is in R (`user::reap_address_spaces_in_region` reads
only `USER_SPACES`). A bound space whose thread lives elsewhere is neither, so `DESTROY` succeeds,
`unpin` clears R's pin, and R's pages, the root included, go back to the allocator while the thread
still runs on that root. The next owner of the page writes that thread's top-level translations,
which is fatal risk 7's confinement claim broken outright.

Is the premise true? Driven once on aarch64 by a scratch test that was not kept: a kernel
thread adopted a space built by `user_address_space_create(R)`, and the test called
`sched::reclaim_region(R)`. It answered `Ok(())` with R no longer resolving and the thread still
on root `0x4001c000`, inside R's old span. The userspace route (`CONFIGURE` with a TCB retyped from a
second region) is reasoned from `configure_thread_control_block`, which checks only that the TCB is
an embryo.

Who builds this shape today? Nobody. Every `ADDRESS_SPACE` retype in the tree takes its TCB from
the same region: `supervision_protocol::build_child_space` (`build_ut` for both),
`fixtures/src/job_mix_task.rs` and `os_primitives_benchmarker.rs` (the child's region for both),
`system_tests/src/user/reap_tests.rs`, and `process_composition_witness`, which binds nothing. So the hole
is reachable only by a program that sets out to build it.

## The options

All three keep the intermediate-table fix as it is; they differ only in the root.

**A. `DESTROY` treats a thread whose bound space is rooted in the region as a resident.** The finish
and refuse phases of `reap_region_objects` test "TCB page in the span"; this adds "or the bound
space's root in the span". Such a thread is killed exactly as a resident is (§16's amendment that
`DESTROY` force-kills a live resident: refuse this pass, kill, the retry reclaims). What changes: a
holder of R can end a thread whose TCB it does not own. It already owns the memory that thread
translates through, so the thread could not have kept running whatever the answer.

**B. `DESTROY` refuses, and kills nothing, while a bound live space is rooted in the region.** The
lender cannot get its memory back until the borrower's thread ends of its own accord. That is the
pre-amendment shape that §24 (interrupting the foreground process)'s `^C` needed removed: a runaway
that never finishes would pin R forever.

**C. `CONFIGURE` refuses a space whose root is not in the TCB's region.** Rung one of the ladder:
the shape that cannot be reaped stops being constructible, and the TCB-based reap already covers
the space. Every binding in the tree today passes. It adds a refusal to `CONFIGURE`, which is a new
answer on the syscall surface, and it forbids a split some future loader might want (a supervisor
holding the TCB budget, a child holding its own space).

## The seven questions

1. Considered and lost. Cutting the root, the answer the intermediate tables got: nothing points
   at a root, so there is no entry to clear. The closest equivalent is switching the thread to the
   reserved root, which leaves a thread that faults on its next instruction and is option A with
   extra steps. Recording the root in the mapping log like a table: the registry already knows
   every root, and what is missing is a reaction, not a record.
2. What the tree does in the analogous case. A thread whose TCB is in the region is killed and
   the destroy retried (§16's force-kill amendment; `reclaim_region`'s `BUGS` calls a refused
   reclaim destructive). Option A extends that to the second page a thread cannot live without.
3. Prior art. From memory, not re-read for this file: in seL4, revoking an untyped deletes
   every object derived from it, including a VSpace root. A TCB whose VSpace capability is
   deleted is left with a null root and faults to its handler on its next run. That is closest to
   option A: the thread survives as an object and cannot run.
4. Premise. True on aarch64 as above; the other two ISAs share the code path
   (`reclaim_region`, `reap_address_spaces_in_region`) and were not run.
5. Cost. A: one more comparison per thread in two scans that are already a walk of the thread
   table, on `DESTROY` only; not measured. B: the same comparison, plus a pinned region for as long
   as the borrower's thread lives. C: one region lookup per `CONFIGURE`, which is spawn-time; not
   measured.
6. Reversibility. All three are §16 semantics, which every future program is written against.
   Nobody has acted on the current behavior, because nothing in the tree builds the shape.
7. Effort. Each is small, and C is the smallest. That is an effort argument, said as one; it is
   not a reason to prefer C.

Options only were given, because this is on the irreversible list. calef chose A. B lost because a
runaway borrower would pin the region forever, which is the shape §16's milestone 28 (a solid terminal: the line discipline as a component) amendment
removed. C lost because it adds a refusal to `CONFIGURE` and forbids a split a loader may want; A
plus C lost because once A exists, C buys no safety and still costs the new refusal.

## Index row

A region's `DESTROY` still hands back a space's root while a thread bound to that space runs on it,
when the thread's TCB came from another region. calef ruled on 2026-10-05 (UTC) that such a thread
dies as a resident: the pass refuses, the thread is killed, the retry reclaims. It is the hole in
fatal risk 7's confinement claim that milestones 762 and 763 left at the top of the page table.
