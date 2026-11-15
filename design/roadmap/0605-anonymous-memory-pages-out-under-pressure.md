---
status: REFUSED
raised: 2026-09-26
promoted_from: anonymous-memory-pages-out-under-pressure
refused_by: 126
---
# 605. Paging anonymous memory out to storage under pressure

Refused 2026-09-26. *(Number provisional until the merge queue lands it. Renumbered from
600 on 2026-09-26, because a concurrent lane kept 600.)* calef ruled
on the proposal the day it was filed. He answered "Yes" to three things together: refuse it for now;
call it "paging out", not swap, because swap already means live replacement; and `free` and `vmstat`
drop their swap rows rather than printing zeros. Promoted from the proposal
`anonymous-memory-pages-out-under-pressure`, filed 2026-09-26 by the lane `proposal/swap`. calef had
asked for it ("Add a proposal for swap") while ruling how milestone 126 (the `procps` package: who
else is running, and who is allowed to ask) reports memory. The argument below is the proposal's
own, kept because its four syscall-surface questions are where a reopening should start.

## The ruling

- Refused for now. The option table below recommended A, and A is what was taken.
- The name is "paging out", and a process that does it is "a pager". "Swap" stays the word for
  live replacement.
- `free` and `vmstat` drop their swap rows. A row that could only ever print zero is not printed,
  and the absence is a `BUGS` line in each tool pointing here. This part is milestone 126's to
  build.

## The word was taken

In this tree "swap" already means live replacement of a running component: `crates/hot_swap_contract`,
the `swapper` fixture and `kernel/src/user/live_swap_tests.rs`, all from milestone 23 (a
capability-routed component OS with live replacement). `swap_protocol` is a ratified name. So this
file said "paging out" and "a pager" and left the name to calef, who ratified "paging out" on
2026-09-26, rather than put two meanings of one word on one subsystem boundary.

## Is it already on the record

No milestone, proposal, refusal or decision covers it. A grep of `design/`, `notes/` and `briefs/`
for swap space, page-out, demand paging, overcommit, external pager and memory pressure on
2026-09-26 finds only these, which are its edges rather than it:

- `design/open-design-ideas.md`, the `Tcb::SUSPEND` tracker, names "a userspace pager (demand paging
  is fault-message, fix, resume)" as the first trigger for designing a resumable fault.
- §26 (the fault endpoint: thread death becomes a message a supervisor holds), part 4, chose
  dead-until-reaped and reserved a message word "so a fault-reply/resume protocol can arrive
  additively".
- §9 (locking: IrqSafeMutex, plus a discipline): "Kernel memory is never demand-paged."
- `crates/page_tables/src/aarch64.rs`: the Access Flag "exists for page-replacement policy we do not
  do".

## What paging out would mean here

On Linux, swap answers "the machine is out of RAM". On nife no process draws on the machine's
free frames directly; they are only where root regions are carved. Milestone 11 (untyped memory,
and the number that proves the kernel stops allocating) and milestone 14 (kernel objects from
untyped: remove the kernel heap) made every page a process maps into a frame. Each is retyped from a
`MemoryRegion` it holds, and the kernel allocates nothing. There is no overcommit: a
virtual page exists only with a frame behind it. Running out means a region's budget is spent, and
§119 (splitting `OutOfMemory`'s three causes is declined for want of a customer) already records
that this is a fact about the caller.

So paging out here would mean one thing: a region may back more virtual pages than it has
frames, with the rest in storage. That settles most of the ownership question by construction.

- The holder of the region still owns every page's contents, resident or not.
- The backing store is paid for the same way frames are: a capability to a storage extent, held by
  whoever pages the region, and carved from someone's grant exactly as a region is split from a
  parent's. No holder can make another holder's pages leave RAM.
- A resident frame stays retyped for life. A pager reuses a frame by mapping it somewhere else,
  never by returning it to the region. That keeps the invariant §13 (capability revocation and
  untyped reclamation (frames)) rests on, that a retyped frame is spend-only, and it means
  `crates/memory_regions`' bump watermark and its Kani proofs are untouched.

That last point decides kernel versus userspace, before any prior art is read. A kernel-resident
pager would have to choose which page to evict, own the frames it reuses, and issue block I/O from
a fault. The kernel owns no frames (milestone 14). It holds no block driver (milestone 261 (the NVMe
driver leaves the kernel, on the machine that can finally confine it) moved the last one out on
2026-09-17), and it takes no fault it could block in (§9). A pager in this tree is a process.

## Prior art

Read on 2026-09-26 by a research pass for this lane, from the sources named. Every capability
kernel in the L4 line leaves paging to userspace, and the two that keep something in the kernel keep
it for anonymous memory specifically.

| system | where the pager lives | how a fault resumes | who pays |
|---|---|---|---|
| seL4 | userspace; the manual never mentions swap | fault IPC on the thread's fault endpoint; "Replying to the fault IPC will restart the thread" | whoever retyped the untyped |
| L4 X.2, L4Re | userspace (the region map is "an application's default pager") | fault IPC to the thread's pager, answered with a map or grant item | the dataspace manager; frames descend from σ0 |
| Genode | a userspace server behind a managed dataspace | core signals the server, which attaches real backing store | the server that attaches it |
| Mach | backing-store I/O in userspace, replacement policy in the kernel, anonymous memory to a trusted default pager | `memory_object_data_request`, answered by `data_supply` | the kernel's page cache holds the frames |
| Zircon | userspace for pager-backed VMOs; anonymous memory is compressed in the kernel and never goes to storage | the thread waits on a page request until `zx_pager_supply_pages` | the pager fills its own VMO and the pages move |

Three details matter here. L4 X.2's unmap is recursive across every address space the page was
mapped into, and returns the referenced and written bits, which is how a userspace pager learns
what to evict without reading page tables it does not own. Zircon's RFC-0219 (anonymous page
compression, accepted 2023-06-03) is explicit. Swap for anonymous memory "would absolutely need to be
implemented in user space via a user pager mechanism". Compression went in the kernel because
"needing to down-call to a user space process to decompress a 4KiB page will add significant
latency". The same RFC gives latency-sensitive VMOs a way to opt out. And the Linux `mlock(2)`
manual page: "paging is one major cause of unexpected program execution delays".

Sources: `sel4.systems/Info/Docs/seL4-manual-latest.pdf`, section 6.2.7; `l4ka.org/l4ka/l4-x2-r7.pdf` section 7.3
and the Unmap section; `l4re.org/doc/l4re_concepts_ds_rm.html`; `genode.org`, Genode Foundations
20.05, the core chapter; `gnu.org/software/hurd/gnumach-doc/Default-Memory-Manager.html`;
`fuchsia.dev`, the `zx_pager_create_vmo` and `zx_pager_supply_pages` references and RFC-0219;
`man7.org/linux/man-pages/man2/mlock.2.html`. Mach's pageout deadlock is folklore here: the manual
does not discuss it, and only Hurd IRC logs mention a "concern about deadlocks".

## What it touches, measured on `main` at `256815e56`

**Faults.** Every user fault is death, on all three architectures. `user_fault` in
`kernel/src/arch/{aarch64,riscv64,x86_64}/exceptions.rs` returns `!` in each port and ends in
`sched::fault`, which calls `depart` and never resumes the thread. No faulted thread ever reruns its faulting instruction. That is three handlers and the scheduler's
departure path.

**Giving up a mapping.** There is no unmap. `crates/abi`'s `address_space` has `MAP_INTO` and
`LIST`; `page_frame` has `MAP` and `REVOKE`. `REVOKE` on an ordinary frame unmaps it everywhere and
deletes every capability to it, the caller's own included, so a pager that used it to evict would
lose the frame it meant to reuse. On a `DeviceFrame` the same method keeps the caller's own
capability and takes the page back from everyone else (milestone 23). That take-back is exactly the
eviction a pager needs, on the wrong object type.

**Knowing what to evict.** All three page-table encoders set the accessed bit eagerly
(`crates/page_tables`: aarch64's AF, Sv39's A, and x86_64 likewise), so nothing records which pages are
in use. Replacement policy needs that bit cleared and read back. From memory, not re-read: x86_64
sets A and D in hardware; aarch64 does so only with FEAT_HAFDBS and otherwise faults on a clear AF;
RISC-V may either update A and D (Svadu) or fault (Svade). What radon's U74 cores and argon's A57
cores do was not measured. That is a parity question with a different answer per ISA.

**Storage.** The block servers are the virtio-blk driver (`components/src/block_driver.rs`) and the
EL0 NVMe driver (`components/src/non_volatile_memory_express.rs`), both speaking
`filesystem_protocol::blk`. Both run on QEMU on all three architectures. On the boards:

| board | block device a pager could use today |
|---|---|
| xenon (x86_64) | NVMe, confined; not yet run at real speed (fatal risk 6 is open) |
| radon (riscv64) | none: its PLDA root complex is not driven, so `find_nvme_device` finds nothing |
| argon (aarch64) | none: a Jetson TX1 stores to eMMC, SD or SATA (recalled, not re-read), and the tree drives none of them |

**Latency, the part the benchmarks can price.** From `bench/baseline-aarch64.txt` (2026-09-26,
icount): `ipc_rtt_el0` is 10,994,714 instructions over 5,000 round trips, about 2,200 each, and
`map_el0` is 390,819 over 500, about 780 per map. A page-in through a userspace pager takes at least five steps: a fault message to the pager, the pager's call to a block server, the device read, a `MAP_INTO` and a resume. That is two IPC round trips and a map, around 5,200 instructions of software before the device
term. The device term dominates and has no number here. The one block measurement on record is
milestone 138 (close the read gap: a 4 KiB request must stop moving 128 KiB)'s filesystem
throughput on QEMU, which prices bandwidth, not a single cold read.

The one memory-pressure failure on record is not a shortage of RAM. The proposal
`a-typed-prompt-outruns-the-undertaker` traces riscv64 spawn failures to 40-page holes left in a
240-page pool when a job's region is reclaimed below the watermark. Paging out cannot fill a hole
in a watermark allocator, so it would not have helped.

## The syscall surface: what building it would ask

Four questions, each calef's, and none of them ruled: the refusal made them moot for now, not
answered. They are kept as the record of what a reopening owes a ruling on.

1. How a fault reaches a pager and the thread resumes. (a) Extend §26: the supervisor's
   rendezvous gets the fault, and a reply on the reserved word resumes the thread, seL4's shape.
   (b) A pager endpoint per thread, set at spawn like the fault endpoint, L4's shape. (c) A pager
   bound to a region rather than to a thread, so every thread that touches a paged region reaches
   the same pager, which is the shape of Zircon's pager-backed VMOs and Mach's memory objects.
   (a) adds no object; (c) is the only one where the authority to page follows the memory rather
   than the thread.
2. How a pager evicts. Either the unmap §162 (whether a holder can give up a mapping, and what
   gives it up) asks about, once ruled, or `page_frame::REVOKE`'s device
   take-back extended to ordinary frames. The second is one method's meaning widened, not a new
   method, but it changes what `REVOKE` promises every holder that already calls it.
3. How a page comes back. (a) The pager holds the client's `AddressSpace` with `WRITE` and
   calls `MAP_INTO`. That exists today, and it means the pager can map anything into the client.
   (b) A supply method on a pager-backed object, Zircon's `zx_pager_supply_pages` shape, which
   confines the pager to the pages of the object it serves at the cost of a new object type.
4. How it is counted. A figure per region for resident and paged-out pages. That is milestone
   126's option 1 with one more field, and it is covered under `free` and `vmstat` below.

## The case for and against

For it. It is the only way a workload larger than its region's frames runs at all. Programs ported
from Linux assume memory is soft, and `free` and `vmstat` expect a swap row. And the region model would make it cleaner than Linux's. Pages leave RAM only from a region whose holder arranged a
pager for it, and the cost lands on that holder rather than on whoever is running when the global
reclaimer wakes.

Against it, in the order that decides:

- No workload needs it. Principle 1 ranks by the customer path, and milestone 530 (name a
  customer, or admit the ranking function has nothing to rank) ruled on 2026-09-21 that the path
  stays vacant. The one customer on record left over family backups, which never needed more RAM.
  The QEMU runners boot with 256 MiB and the boards have 4 GB (argon) and 17 GB (xenon), and no
  record in the tree shows a workload running a machine short of RAM.
- It spends the latency story. Milestone 148 (a noise bound, not a noise measurement) claims
  something no general-purpose kernel can: every kernel-side event that can preempt a running
  thread, enumerated and bounded. A page-in is an unbounded wait on storage inside a memory access.
  A pager bound to a region (question 1c) keeps the bound for every region without one, which is an
  argument for 1c if this is ever built, but a program that touches paged memory has no bound.
- Its prerequisites are worth more than it is. A resumable fault and an unmap each have other
  consumers: the SUSPEND tracker names a debugger and job control, and milestone 95 (an unmap
  primitive, and the mappings init never lets go) is a confinement hole today. Neither should be
  shaped by a pager nobody needs.
- Parity costs a driver per board. Two of three boards have nothing to page to. An in-RAM
  compressing pager (option C below) avoids that, and has no customer either.

## Whether to build it

| option | what it is | why it loses, or wins |
|---|---|---|
| A. Refuse it now, with the condition that reverses it | a refusal milestone at promotion | taken, 2026-09-26 |
| B. A userspace pager to block storage | the four surface questions, a pager process, a storage extent capability | the whole prerequisite chain for no workload, and a board driver short on two ISAs |
| C. A userspace pager that compresses into its own region | as B, minus storage | parity is free; Zircon measured the down-call as too slow and moved compression into its kernel, which this kernel cannot do; and RAM is not short |
| D. Paging in the kernel | eviction and block I/O on the fault path | refused on its face: the kernel owns no frames, holds no block driver and cannot block in a fault |

Recommendation: A, because refusing is a record and costs nothing to reverse. The refusal should
name what would reverse it: a workload on the customer path whose working set exceeds what its
region can be given, on a board with a block device nife drives. Until then the surface questions
above stay unasked, and a resumable fault and an unmap get designed for their own consumers.

The equal-cost test: if B were free, the customer argument goes away and the latency argument does
not. So at equal cost B is worth having only as a pager bound to a region, which the Revisit
condition below carries forward.

## What it changes for `free` and `vmstat`

Milestone 126's open fork (the lane's appendix, `notes/process-view/remaining-procps-programs.md` on the branch
`milestone/126-procps`, pull request #1349) is whether memory is reported per region under a
capability or as a machine-wide figure. Paging out follows whichever wins. Under option 1 a
region's figure grows a paged-out count beside its committed count, read with the same right, and
there is no machine-wide swap total anyone holds a capability to. Under option 2 a machine-wide
swap figure is one more ambient fact.

With A, the columns have no subject. Linux with no swap configured prints `Swap: 0 0 0` (recalled,
not re-read), and those zeros would be true here. calef ruled against printing them: `free` and
`vmstat` drop the rows, and each tool's `BUGS` section says why and points here.

## Parity

Anything built from B or C is three pieces of work per ISA, and none is shared: a resume path in
each `exceptions.rs`, an accessed-bit story that differs per architecture, and a block device on
each board. Under DECISIONS §19 (architectural parity is a tenet) it ships on all three or records
the gap.

## Exit criterion, if it is ever reopened

On all three architectures, a process maps more pages than its region has frames, writes every
page, and reads each back byte-exact. A second process whose region has no pager takes zero
page-in faults across the same run, so milestone 148's bound still holds for it.

## Revisit

- **Condition.** A workload on the customer path whose working set is bigger than the region it
  can be given, on a board with a block device nife drives. Both halves: a workload with no device
  to page to reopens the device question first, and a device with no workload is the case refused
  here. Whoever reopens it starts from the four surface questions above, and at equal cost the
  constraint that survives is a pager bound to a region, so milestone 148's bound holds everywhere
  else.

## Index row

Refused: paging anonymous memory out to storage, until a customer's working set exceeds its region
on a board with a block device nife drives. Named "paging out", since swap means live replacement;
`free` and `vmstat` drop their swap rows.
