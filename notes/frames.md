# PageFrame capabilities: shared memory a process owns

*Renamed 2026-08-24 (DECISIONS §113, milestone 158): the kernel object this file describes was
called `Frame` until this rename. It is now `PageFrame`, the standard OS term for a physical page
in a virtual memory system, chosen because bare `Frame` collides with `crates/compositor`'s own use
of "frame" for a rendered screen update, an unrelated concept. The appendix on a stack-frame fault
is historical and keeps the word as it read before the rename. `frame` there always meant a CPU call
frame (compiler stack-size accounting), never this object. Renaming it would create the exact
collision this rename exists to remove.*

The evidence behind this note is in [frames/](frames/README.md): the page frame budget, the BUGS
list in full, the record of the run-of-pages fork, and the stack-frame fault.

DECISIONS §10 (process model) has a one-line rule for the data path: IPC carries control, shared memory carries
data. The endpoint moves the small stuff (a length, a request code) and the bulk bytes live in a
page both parties can see, so the kernel never copies them. For a long time nife honored that
rule only by accident of setup. The kernel allocated the shared page and mapped it into both the
console client and server at spawn, and both sides found it at a fixed virtual address they had
agreed on in advance. The sharing was real but frozen. Two processes could share memory only if the
kernel decided, at the moment it created them, that they should.

A `PageFrame` capability makes shared memory a thing processes *do* instead of a thing the kernel
*pre-arranges*. This note is that object.

## What a PageFrame is

A capability whose object is a single physical page. Its address is its identity:
`Object::PageFrame(pa)` names the page at `pa`. A process can never forge one. The only ways to hold
a `PageFrame` are to retype it out of your own untyped or be handed it by someone who has it, and
both keep the object intact. Its rights say what you may do with the page: `READ` to map
it read-only, `WRITE` to map it read/write, `GRANT` to pass it on.

## Retype, then map: two operations, not one

seL4 splits "get a page" from "put it in your address space," and so do we, because the split is what
makes a page a first-class, delegatable object rather than something that only exists mapped.

- `MemoryRegion::RETYPE` carves one page out of the caller's untyped and mints a `PageFrame` capability for
  it, full rights, into the caller's capability table. Nothing is mapped. The caller now *holds a page* and
  can map it, or delegate it, or delegate it and never map it.
- `PageFrame::MAP(va, writable, untyped_slot)` maps the page at `va`. A read/write mapping needs `WRITE`
  on the page frame; a read-only one needs `READ`. The page tables to reach `va` are drawn from the
  untyped named by `untyped_slot`, so like everything a process spends, mapping a page frame comes out of
  its own budget and the kernel allocates nothing.

Contrast `MemoryRegion::MAP`, which does both at once (retype a page and map it writable). That is the
convenient path for a process's private memory. `RETYPE` + `MAP` is the path when the page is going
to be shared, because between the two steps is where the delegation happens.

## Sharing is delegation applied to memory

Because a `PageFrame` is an ordinary capability, it travels over a rendezvous with `SEND_CAP`, and the
rights narrow on the way exactly as they do for any delegation (see [delegation.md](delegation.md)).
So the whole sharing protocol is:

1. Producer `RETYPE`s a page frame, `MAP`s it read/write, writes into it.
2. Producer `SEND_CAP`s the page frame to the consumer, narrowed to `READ` (dropping `WRITE` and `GRANT`).
3. Consumer `RECEIVE_CAP`s it, `MAP`s the *same physical page* read-only, and reads what the producer
   wrote.

The kernel copied nothing and was never told these two processes would share memory. They built the
sharing themselves out of a capability, and the read-only narrowing means the consumer can look and
not touch. A peer handed `READ` alone gets `NotPermitted` if it asks to map the page writable, which
the test checks by trying.

## The lifetime question, and why there is no double-free

A page shared into two address spaces cannot be owned by either, or the first one to die frees memory
the other is still using. nife sidesteps this cleanly because of how teardown already works: an
`AddressSpace` frees only the page frames it recorded at spawn (`self.frames`) plus its page tables and
root. A page mapped at *runtime*, by `MemoryRegion::MAP` or `PageFrame::MAP`, is never in that list, so
teardown does not free it. A page frame's page (and the page tables that map it) belong to the untyped
region they came from, and are reclaimed only when that region is destroyed, wholesale, the way
untyped memory always is. So when the producer exits, its mapping of the shared page goes away
with its address space; the physical page persists, and the consumer's mapping is still good. No
refcount, no double-free, because address spaces borrow page frames and never own them.

The honest limit: individual page frames are not reclaimed on their own, only with their whole untyped
region. That is the same bounded, deliberate gap untyped memory already has, and closing it is the
same parked problem: capability revocation.

## The synchronization edge is the IPC rendezvous

On ARM's weak memory model, the producer's write is not automatically visible to the consumer just
because it happened first in time. What makes it visible is that the delegation is a *rendezvous*:
the producer's `SEND_CAP` releases the scheduler lock and the consumer's `RECEIVE_CAP` acquires it, and
that release/acquire pair is the happens-before edge. The write lands before the send, the send
synchronizes with the receive, the read comes after. So the same IPC that carries the capability also
orders the memory, which is a tidy demonstration of why "control travels by IPC" and "data travels by
shared memory" fit together rather than being two unrelated rules.

## What the test proves

`a_page_frame_capability_shares_a_page_and_a_read_only_view_cannot_write_it` runs the protocol above
with two user processes and checks two things. The consumer reads the producer's sentinel through its
own mapping (the page is genuinely shared). And a writable mapping of the read-only view is refused
(the rights confine it). The sharing half is self-verifying in a nice way: `RETYPE` hands back a
*zeroed* page, so if the consumer had somehow mapped a different page instead of the shared one, it
would read zero, not the sentinel. Reading the sentinel can only mean it mapped the producer's page.
And verified it can fail: stub the `WRITE` check in `PageFrame::MAP` and the read-only view becomes
writable, so the confinement assertion trips.

## Three ways a page gets into an address space, and one of them is invisible

This is the finding milestone 108 (the drivers move onto frame capabilities) turned up, and it is the reason that milestone existed. There
were, and are, three routes:

| Route | Who calls it | In the mapping database? |
|---|---|---|
| `PageFrame::MAP` | a process, for a page frame it holds | **yes** |
| `AddressSpace::MAP_INTO` | a userspace loader, into a space it is building | **yes** (`user_address_space_map`) |
| `Spawn::maps` | the kernel, before the process's first instruction | **no** |

The first two go through `revoke::record_mapping`, and an unrecordable mapping is refused rather
than made, at the mapper's own expense. The third is `AddressSpace::map_physical`, which maps and
returns; there is nothing to record it against, because the process does not exist yet.

So a page delivered by `Spawn::maps` cannot be revoked. `PageFrame::REVOKE` deletes every
capability naming the page (there is none) and unmaps it from every space that recorded it (this one
did not), and the holder's mapping survives untouched. That is not a bug in `revoke`; it is the
honest consequence of a mapping that no capability ever stood behind. A spawn-time mapping is
permanent by construction.

It cannot be narrowed by anyone downstream, because the kernel picked the permissions at spawn
and there is no object to attenuate, and it cannot be handed on, because there is nothing to hand.

## The migration (milestone 108)

The disk and display paths now hold their pages as `PageFrame` capabilities and map them themselves.
Each migrated program gained two things in its capability table: the page frames, and an untyped to draw the
page tables from, because `PageFrame::MAP` retypes intermediate tables out of a region the caller names
and the kernel allocates nothing.

Migrated: `disk_surveyor` (the block-shared page and the roster), the roster probe, `disk_partitioner`,
`mkfs`, the virtio-gpu driver (its whole DMA region), `painter` (the surface), and `display_terminal`
in its whole-screen mode (the surface and the application's output page).

The stack is the floor. It is still a `Spawn::maps` entry and has to be: a program cannot map its
own stack, because it needs a stack before it can make the syscall that would map one. Everything
else a process touches can be a capability; that one page cannot.

### EXAMPLES

The wiring side, from `kernel/src/user/disk_service.rs`. The roster goes in with `READ` and nothing
else, and the program is handed a budget to reach it with:

```rust
crate::sched::grant_at(SURVEY_SLOT_BUDGET, untyped_cap(budget))?;
crate::sched::grant_at(SURVEY_SLOT_ROSTER, page_frame_cap(roster_phys, Rights::READ))?;
run(surveyor_image, Spawn { arg0: ROLE_SURVEY, grants: &[], maps: &stack, .. })
```

The program side, from `components/src/disk_surveyor.rs`. It picks its own address, because it owns the
page now and the kernel has no opinion:

```rust
if !user_mode_runtime::map_page_frame(ROSTER_PAGE_FRAME, ROSTER_VA, /* writable */ false, BUDGET) {
    user_mode_runtime::exit()
}
```

And the negative control, which gained a rung it could not have had before. Under the old wiring the
only boundary was the page permission, so the only thing the probe could do wrong was write. Now it
cannot even obtain a writable window:

```rust
// Rung one: refused by the rights on the capability, before a page-table entry is written.
let rw_refused = !user_mode_runtime::map_page_frame(ROSTER_PAGE_FRAME, ROSTER_VA, true, BUDGET);
// Rung two: the read-only mapping we are entitled to, and a write through it. This faults.
user_mode_runtime::map_page_frame(ROSTER_PAGE_FRAME, ROSTER_VA, false, BUDGET);
unsafe { core::ptr::write_volatile(ROSTER_VA as *mut u64, 0) };
```

### What the migration proves that the object alone did not

`disk_tests::the_roster_can_be_revoked_out_from_under_its_holder`. A program maps the roster page
frame, reads its first word and reports it, and parks. The kernel checks that word against its own
read of the same physical page through the direct map (so the mapping was real, and was of *that*
page), revokes the page frame, and lets the program go. The second read faults, at the address it
faults at.

Verified it can fail, which is the point of writing it: put the roster back as a `Spawn::maps`
entry and the test trips its own assertion, "a program read a page frame that had been revoked out
from under it, at 0x50010000, and was NOT stopped: the mapping outlived the capability". That was the
state of the world for every driver in the tree the day before.

## The page frame budget: what a test boot spends, and what it gets back

The aarch64 test boot used to fail as `Unmappable(OutOfPageFrames)` about one run in three
(measured 2026-08-16). It failed in whichever test spawned last, never the test that spent the
memory. The cause was contiguity rather than memory. Loading a program calls `alloc_contiguous`, and
the boot ended with 216 free page frames and no free run longer than 117.

`kernel/src/testing.rs` now keeps a page frame ledger. It reads free page frames once per test, so
each test is charged the drop to the next reading. Two ceilings fail the run:
`SUITE_PAGE_FRAME_BUDGET` catches the total residue growing, and `SUITE_MIN_FREE_RUN` requires a
free run of at least 1024 page frames at the end. The second is the real gate.

The fix was wiring, not mechanism. `user::holding::Holding` (name provisional) gives back what the
kernel handed a service, and reclaiming a region now sweeps its endpoints before it refuses, so a
server parked in `RECEIVE` can be reclaimed. The boot now ends with a free run of 14080. What is
still held at the end is accounted for item by item: boot services wired once, `root_supervisor`'s
split trees, and the DMA pages a dead driver's NIC may still write.

A kill must not land on a kernel thread that is still building a process. `user::run_with` holds
the space it loads as a local until `sched::adopt_address_space` hands it to the registry, and a
thread `schedule` converts is never resumed, so a kill carried out in between loses that space for
the boot. `sched::defer_kill` and `sched::allow_kill` bracket the window: an armed kill waits and is
carried out once the space is the thread's. `sched::tests::a_kill_armed_while_a_space_is_built_waits_for_it_to_be_adopted`
proves it.

Read a large per-test charge as "the first test to need that service", not as an accusation. The
ledger attributes by the running test. The tables, the fix in full, the itemized residue and the
ledger's own BUGS are in [the page frame budget](frames/page-frame-budget.md).

## BUGS

Each entry is in full, with its measurements and history, in [frames/limitations.md](frames/limitations.md).

- A capability naming a run used to survive reclamation of the pages it named. Fixed 2026-08-27:
  reclamation sweeps capabilities by overlap over the whole range.
- A `REVOKE` on one run used to unmap mappings it did not authorize. Fixed 2026-08-27: each mapping
  record carries the base of the object it was made under
  ([design/decisions/0132-overlapping-page-frame-runs.md](../design/decisions/0132-overlapping-page-frame-runs.md)).
- An overlapping capability still survives a `REVOKE`. That is the decision, not a gap.
- A run and a longer run sharing a base are one object to the revocation log. The over-broad unmap
  stays inside the revoked run.
- Revocation says nothing to a device. A DMA window is a byte range, not a capability, and whether
  `REVOKE` should narrow it is still an architect's call.
- A failed multi-page `MAP` undoes the mapping but does not return the page tables it spent.
- A run capability names up to one page more than its object. Every consumer bounds-checks against
  `SURFACE_BYTES`.
- Resolved 2026-08-26: a `PageFrame` names a run of pages
  ([design/decisions/0102-frame-names-a-run.md](../design/decisions/0102-frame-names-a-run.md)). The
  appendix keeps how the fork was found, priced and decided.
- Not everything migrated. The console is deliberately last, the compositor path is out of scope,
  and `date`'s test wiring still uses `Spawn::maps`.
- Each migrated program costs one more untyped region.
- The mapping records cost the process, not the kernel.

## A note about stack frames

A thread-stack fault that surfaced during milestone 108's migration was not this milestone's. Its
cause was one 6816-byte frame in `sched::reap_region_objects` on `main`, and then a stack freed under
its owner. The account, which uses "frame" for a CPU call frame and predates the `PageFrame` rename,
is in [frames/thread-stack-fault.md](frames/thread-stack-fault.md).

