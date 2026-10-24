# What attacking the claims found: risk 7's adversarial pass, 2026-09-21

An appendix to [notes/confinement-claims.md](../confinement-claims.md). It holds the first pass
that assumed a claim false and went looking for the case that makes it so.

## The posture

Every pass before this one read the claims and asked whether each was tested. This one took the
other posture `design/fatal-risks/README.md`'s risk 7 has asked for since it was written: assume a
claim is false, and go looking for the case that makes it so. One claim was false in the tree, on a
path any two cooperating programs can take, and the tree had already written down the rule it
broke. The rest is what was attacked and held, because a pass that reports only its hits is
indistinguishable from one that stopped early.

## A capability revoked while it is in flight was delivered anyway

The attack: ask where a capability can live that is not a capability-table slot, because every
revocation sweep in the kernel walks tables. There is exactly one such place, and it is not obscure.
It is `Thread::outgoing_cap`, the hand-off slot `sched::ipc_send_cap` writes when a `SEND_CAP` finds
no receiver waiting, and `sched::ipc_receive_cap` takes when one arrives.

The claim was false. A sender parks `PageFrame(p, 1)` there and blocks. `PageFrame::REVOKE` then
runs over that frame. `sched::delete_page_frame_caps_where` deletes the capability from every table,
including the sender's own. `revoke::unmap_under_object` unmaps every page the log records. The
hand-off slot is read by neither. The next `RECEIVE_CAP` files the surviving capability in the
receiver's table, and the receiver may `MAP` a page the revoker believes it took back. This was
measured, not argued.
`kernel::user::revocation_in_flight_tests::a_capability_revoked_while_it_is_in_flight_does_not_reach_the_receiver`
went red on the tree as it stood, on aarch64, with its vacuity guard and its premise check both green
first.

`MemoryRegion::DESTROY` is the sharper case. `revoke::revoke_region` sweeps capabilities by overlap
precisely so that "no capability still names a page this allocator is about to hand out" is true
before the region's pages go back. That is the hole the review in milestone 142 (a text display good
enough that people use it instead of a GUI) found and closed. An in-flight capability reopens it: the
use-after-free that DECISIONS §13 (capability revocation and untyped reclamation) exists to prevent,
through the one slot nobody swept.

The tree had written the rule down, once, for a different object. `sched::delete_reply_caps_naming`
sweeps `outgoing_cap` beside the tables. Its doc comment says why, in exactly the words this defect
needed: *"`outgoing_cap` goes too, and it is the half a second copy would forget ... a live `Reply`
in a hand-off slot is the same forgery one step earlier."* The one sweep without the defect is the
one that states the rule. That is the same shape as the row 12 finding of milestone 307 (which
assertion actually fires when a confinement claim is broken). There a comment correctly explained a
trap, and then cited, as its precedent, the one harness still caught in it. The failure is not that
nobody knew. It is that knowing lived in a doc comment on one call site, which is AGENTS.md's ladder
rung four wearing the clothes of a design.

Fixed here, in the three sweeps that lacked it, by dropping the parked capability. They are
`delete_page_frame_caps_where` (which is both `PageFrame` policies),
`delete_device_frame_caps_from_others`, and `x86_64`'s `delete_port_range_caps_impl`. Dropping rather
than failing the send is the behavior `ipc_send_cap` already documents for the other way a hand-off
comes up empty, a receiver whose table is full. The data word still arrives, and the receiver sees
`NO_CAP`. No new error reaches userspace, so nothing about the syscall surface moves. The
falsification is recorded and replayed red.

What it does not settle. Only the `PageFrame` sweep is driven by a test. The other two carry the same
two lines, and nothing exercises them through a parked hand-off, so read those as reasoned from the
code. And the test stops at delivery rather than at exploitation. It proves the receiver holds a
capability naming the revoked run, not that it then read a page somebody else owns.

## A mapping the kernel wires at boot is invisible to revocation

`user::AddressSpace::map_physical` did not call `revoke::record_mapping`, and every unmap sweep in
`crate::revoke` is driven by that log. So a page placed by kernel wiring survived
`DeviceFrame::REVOKE`, `PageFrame::REVOKE` and `MemoryRegion::DESTROY`. Such pages are the serial
driver's UART registers, a `Spawn::maps` entry, a `DeviceRun`, the initrd, and the `x86_64` timebase
page. The capability went and the mapping stayed, which is a take-back doing half its job.

This was first recorded here as latent, and that was wrong. The paragraph this replaces said nothing
reached it. Its grounds were that every `map_physical` call site is boot wiring, and that a driver a
userspace supervisor builds takes `MAP_INTO`, which records. Both halves of that are true, and the
conclusion does not follow. It asks only where the *mapping* comes from, and never asks whether
anything holds a *capability* to the same page. Something does, in an ordinary boot. The two halves
are wired by different modules, which is why nobody had put them side by side.
`user::fs_service::spawn_fs_server` puts the file channel's shared pages into the FS server through
`Spawn::maps`. `user::boot_progenitor` hands the progenitor `PageFrame(file_shared, 1)` with `GRANT`
over the first of exactly those pages. `PageFrame::REVOKE` on that slot is a syscall the progenitor
may make. It deleted every capability naming the run and unmapped every mapping recorded under it.
The FS server kept its writable mapping of a page the progenitor had just un-shared.

Fixed, by making the record a required argument rather than a call to remember. `map_physical` now
takes `revoke::PageMapSource` and records. `record_mapping` has required that answer since §132
(what `PageFrame::REVOKE` owes an overlapping run); `map_physical` was the one mapping site in the
kernel that never had to answer, because it never recorded. Every kernel-wiring caller passes
`NoCapability`, truthfully. So the page stands as its own object, and a single-page revoke of it
finds the record. `user_address_space_map` was the one caller that remembered to record, and is now
the one caller with nothing extra to remember. Nothing about the syscall surface moves.
`kernel::user::spawn_mapping_revocation_tests::a_page_the_kernel_wired_is_unmapped_when_its_frame_is_revoked`
is the falsification, replayed red on aarch64 with its vacuity guard and premise check green first.

What it costs, since a record is not free and these are boot paths. It is one `LogEntry` per
mapping, 170 to a page, paid out of the mapped space's own backing region. Every caller but one maps
a handful of pages. The exception is the initrd read-only window: 2837 pages on aarch64 today, which
is 17 log pages per space that maps it. The suite was green before those budgets were widened. That
means the existing slack absorbed it with roughly 8% to spare. That is the reason the term is now
explicit (`revoke::log_pages_for`), rather than a reason it did not need to be. The archive grows
every milestone, and the first budget to blow would have done it on an unrelated change.

What it does not settle. The test drives the `PageFrame` sweep. `revoke_device_from_others` and
`revoke_region` read the same log and are fixed by the same record. But nothing drives a wired
mapping through either, so read those as reasoned from the code. `revoke_region`'s limb is the
narrower one, and worth stating as a genuine "narrower than recorded". No `map_physical` call site in
the tree maps a page that came from a `MemoryRegion`; they come from `memory::alloc`, from MMIO, or
from the initrd. So `MemoryRegion::DESTROY` never had a wired mapping to walk past. And
`AddressSpace::map_new` still records nothing, deliberately. Its frames are retyped from the space's
own region and freed with it, so no capability names them.

## What was attacked and held

One attack per line, because the misses are what make the hit worth believing.

| # | The attack | Result |
|---|---|---|
| 1 | Is there any other authority-bearing field on `Thread` that a table sweep cannot see? | **Held.** `port_range_grant` (milestone 313's find, fixed) and `cycle_counter_grant` are the only two, both are cleared where they must be, and `mailbox` carries scalars. `outgoing_cap` was the third and is the finding above. |
| 2 | Does a recycled TCB slot inherit a dead thread's authority? | Held, structurally. `Thread` has no `Default` and all four constructors (`boot`, `adopt_current`, `spawn_into`, `embryo`) write every field by name, so a new authority-bearing field cannot be added without four decisions. Rung one. |
| 3 | Is the cycle-counter grant enforced on every architecture? | **Held, as a stated exception.** `x86_64`'s `set_cycle_counter_grant` is an empty function: `rdtsc` is ambient in ring 3 and DECISIONS 139 part 3 kept it that way, because `user_mode_runtime`'s `now()` *is* `rdtsc` there and closing it would take out `Instant`, `sleep`, the seed and the benchmark harness at once. It says so in its own doc, at length, and `notes/x86-port.md` carries it. Not a gap. |
| 4 | Can a thread read another thread's FP/SIMD registers? | Held. `fp::hand_over` scrubs to `FpState::INITIAL` on the live-to-not-live switch, `sched::schedule` is the only switch site in the kernel, and the module header names `LazyFP` (CVE-2018-3665) as the reason it is eager rather than lazy. |
| 5 | Can a name escape a directory capability's subtree? | Held. `fs_subtree_caretaker` performs no checks by design, so the whole claim rests on `redoxfs_server::check_component`, which refuses `.`, `..`, the attribute store's directory, and any name containing `/`, `\`, `:` or NUL. All twelve name-taking server verbs call it, `open_dir` and `make_dir` through `resolve_child_dir`. The cross-directory `rename` checks both names and both handles. |
| 6 | Is `record_mapping`'s failure ignored anywhere, so that a mapping is unrecorded? | Held. Three call sites, all three check the return and unmap what they just mapped. The unrecorded mappings are the ones that never call it, which is the section above. |
| 7 | Does a revocation reach the IOMMU's device domain? | **No, and it is a documented limit rather than a live defect.** `iommu::confine` has no inverse and its own doc says it runs once per device per boot. Only kernel drivers call it and the regions it maps are kernel-owned, so there is nothing a userspace revoke is failing to undo. It goes live the day a driver leaves the kernel and programs its own device, which is the question DECISIONS §86 (whether an NVMe driver can leave the kernel, and what
capability would let it) answers. |
| 8 | Can an ELF segment whose `p_vaddr + p_memsz` overflows be laid over the kernel? | **Held.** `map_segments` maps page by page through `AddressSpace::map_new`, whose `Mapper` is built `Half::Low`, so the refusal is per page rather than per segment and a run that walks out of the low half is refused where it walks out. This is the defect class milestone 142's review found in the two syscall map paths, which check the run's last page as well as its first. |

## What this pass could not reach

- The falsification for the first finding was aarch64's until milestone 323 (the falsification
  record is incomplete in five ways) replayed it on riscv64 and `x86_64` on 2026-10-03 (UTC), red on
  both. The record now names all three.
- The claims whose enforcement is a userspace program rather than the kernel. The caretakers were
  read, not attacked from a hostile client. A hostile client is a fixture and a boot, and it is the
  shape `design/fatal-risks/README.md` says wants outside eyes anyway.
- Anything needing hardware this project does not own. MSI confinement stays exactly where milestone
  317 (the interrupt-remapping flags, and where MSI confinement actually lives) left it.

## The in-flight test, and what it leaves untested

`kernel::user::revocation_in_flight_tests` holds the first finding. The claim it guards is row 4 of
[notes/confinement-claims.md](../confinement-claims.md), DECISIONS §12's "a consumed capability
cannot be used again", and the region sweep's promise that no capability names a page the allocator
is about to hand out.

The `PortRange` sweep has a second limb that no test reaches. `ipc_receive_cap` files a delivered
capability with `CapabilityTable::insert` directly, not through
`sched::thread_control_block_insert_cap`. A receiver handed a `PortRange` capability that way holds
it with `Thread::port_range_grant` never set. That direction fails closed.

`kernel::user::spawn_mapping_revocation_tests` holds the second finding, and proves the mapping is
gone, not that the process faults on it. Walking the page tables is what `crate::revoke`'s own suite
asserts and what the sweep controls. Demonstrating the fault needs a live process, which
`user::live_swap_tests` does for the device case, at a whole boot's cost.
