# User address spaces in the kernel

A user address space is `user::AddressSpace` (`kernel/src/user/address_space.rs`), and the registry
that names every one of them is `kernel/src/user/address_space_registry.rs`. `kernel/src/user/loader.rs`
builds a space from an ELF. This note carries the design those files' doc comments link to: who
owns a space's memory, what a mapped window costs, the pages every process gets, and how a space
leaves the registry. [The address-space map](address-space-map.md) has where each band of a process
sits, and [naming a running address space](naming-a-running-address-space.md) has §249 (a running address space stays nameable)'s
background.

## One region, and who frees it

Every page a space owns comes from one untyped region (milestone 14 (kernel objects from untyped) phase B.4): the root table, the
intermediate tables and every owned leaf. The region is the record of what the space owns, so
there is no frame list. Teardown is one `memory_region::destroy`, made safe by §13 (capability revocation and untyped reclamation) revocation. A
space dies all at once, so the kernel never unmaps a space page by page to tear it down, and the
leak of a leaf unmapped with its tables left standing cannot happen.

A space is built two ways, and they differ in who frees that region. `AddressSpace::new` carves its
own region out of the frame allocator, so nobody else has a name for it, and its `Drop` is the only
thing that can free it. `user_address_space_create` (the `RETYPE_OBJ(ADDRESS_SPACE)` engine,
milestone 19b (run a real workload)) is lent a region whose `MemoryRegion` capability the caller holds, and that caller
reclaims it with `DESTROY`. A lent region has two names for one run of memory, and only one of them
may free it.

`Backing` is a two-variant enum with the region inside, so a space cannot be built without saying
which it is. That is rung one of AGENTS.md's ladder. A pin on the lent region is not enough on its
own. `retype_object_page` pins and `sched::reclaim_region` unpins, but `reclaim_region` clears the
pin before the reaper's deferred drop can land. `sched::finish_switch` hoists a dead thread's space
out from under `IPC_TABLES`, releases the lock, and only then drops it. Two `destroy` calls for one
region would then both pass the refusal check and both free every page of the run.

Spending a lent region is correct: it is the point of `RETYPE_OBJ(ADDRESS_SPACE)`, which runs the
space on the caller's budget. Only freeing is restricted.

## The TLB tag

Every space takes an ASID at creation (milestone 15 (tagged address spaces), `crates/address_space_identifier`). Every user
mapping is non-global, so its TLB entries carry the tag and a context switch flushes nothing: the
other spaces' entries stop matching. The tag is taken before the space joins the revocation
registry, because the registry records it and a revoke that cuts a table out flushes by it. `Drop`
invalidates every entry wearing the tag on every core, and only then frees the number, so the next
owner cannot hit stale translations.

`Drop` also stops being the live space before its frames go back. Otherwise the `TTBR0` the walker
reads points at memory the allocator has handed to somebody else. That is about the walker, not the
TLB: neither ISA flushes on a root switch, and cached translations are the `flush_asid` at the end.

On RISC-V a process runs on one `satp` that must map both halves, so each new root gets copies of
the kernel root's high-half entries (`mmu::share_kernel_half`, DECISIONS §17 (the second architecture)). On aarch64 the kernel
lives in `TTBR1` and the copy is a no-op.

## What a mapped window costs

`AS_OVERHEAD` is sixteen pages beyond a space's content: the root, an L1 and an L2, a handful of
L3s (one per 2 MiB window touched), and margin. A page mapped with `map_physical` is not free. It
costs a share of an intermediate table and, because the mapping is recorded, a share of a log page,
both out of the space's own region.

The overhead absorbs that for the handful of pages most callers map. It does not absorb it for the
two that map thousands. The progenitor's archive window pays for itself at its own call site. The
installer's copy of the boot file goes through `run`, so `load` takes a `windowed` count and charges
it there. A budget a caller must remember to widen is wrong the first time somebody maps a bigger
window, and its failure is an `OutOfPageFrames` panic three frames away from anything that mentions
memory.

What `load` charges is the cost above `WINDOW_IN_OVERHEAD`: one log page (170 recorded mappings)
and no table pages, since a window that size touches at most one L3 the overhead already has. A
version that charged the full window moved the aarch64 suite's frame ledger from 22,249 kept frames
to 22,317. That is one frame per long-lived process, reserved into a region that never used it, to
fix a window one caller has.

`run_with` counts every window the new process will have: the `Spawn::maps` pages, a device run, and
every `PageFrame` run it is granted, since a process maps a granted run into itself and pays the
log for each page. Both kinds of grant count: the runs still in `spawn.grants`, and the runs the
spawn closure already placed at fixed slots with `grant_at`. Every screen and GPU spawn hands over
its surface the second way, with `grants: &[]`. A 1920x1080 surface is 2,025 records, twelve log
pages. Counting only the first kind runs the log dry partway through, and the framebuffer driver is
refused page 1,029 and dies with `E_SURFACE`.

## Mappings are recorded

`map_physical` maps a page the space does not own: a device's registers, or a page shared with
another space. `Drop` leaves such a page alone, since freeing MMIO is meaningless and freeing a
shared page when one holder dies would hand live memory to the allocator. The tables reaching it
are the space's own.

The mapping itself is recorded in the revocation log, because a mapping revocation cannot see is
the DECISIONS §13 use-after-free. Every unmap sweep in `crate::revoke` is driven by that log. An
unrecorded page would survive `PageFrame::REVOKE` of a frame shared through `Spawn::maps`, such as
the file server's channel, which `boot_progenitor` also grants the progenitor.
`user::spawn_mapping_revocation_tests` is the falsification.

`under` names which capability's authority made the mapping, and it is a required argument for
`crate::revoke::PageMapSource`'s own reason. Every kernel-wiring caller passes `NoCapability`
truthfully. A `Spawn::maps` entry, a `DeviceRun`, the initial ramdisk and the timebase page are
endowments the process holds no capability for, so a single-page revoke of the page finds the
record. The `MAP_INTO` syscall is the caller that passes a capability, because it has one.
`map_physical_held` maps under a `MappingHold` the caller already has, so a capability read under
that hold and the mapping made from it are one critical section.

## The pages every process gets

### The current-CPU page

A thread reads its own core from a per-thread page rather than through a syscall (calef, 2026-09-21),
because the consumer is a memory allocator asking on every allocation. `crates/current_cpu_contract`
holds the layout and the ordering argument. The context switch publishes the core on the core doing
the switch, which is the core the thread will run on.

`AddressSpace::new` attaches the page unconditionally, so every kernel-built space has one and no
new builder can forget it. A space userspace builds gets its page when a TCB binds it, which is
when "which CPU am I on" starts to have an answer. `attach_current_cpu_page` is idempotent, and every
failure is silent: a thread with no page reads an unmapped address, and the reader answers `None`.
A load that failed because one frame was short would turn a diagnostic into a reason a program does
not start.

The frame is zeroed, because the whole frame is mapped into the process, and then stamped. The
magic tells a prepared page from a frame nobody wrote, and a sentinel makes "never run" a state
rather than CPU 0. The page is read-only to the process. The kernel writes a frame it owns and lends
a view, which keeps this out of Tock's necessarily-unsafe category ([the trusted base](trusted-base.md)).

The frame comes from the global frame allocator, not the space's region. A lent region is sized by
whoever lent it, and spending one more page of it unconditionally breaks hand-sized regions. So
`Drop` frees this frame by hand. A process's other threads get pages of their own in slots 1 and up
(milestone 812 (`std::thread::spawn` runs real threads), §269 (how threads share a process) fork
6). A slot's page is kept once made and reused by the next thread to take the slot. The pages
are stored as bare physical addresses with a bitmap, because an `Option` per slot was 384 bytes in
every `AddressSpace` and pushed `load`'s frame past the 4 KiB guard page on riscv64.

### The timebase page

`user_mode_runtime::now()` needs the counter's rate. aarch64 states it in `CNTFRQ_EL0`, which the
kernel opens to EL0. `x86_64` measures it and riscv64 reads it from the device tree, which a process
cannot reach, so on those two the kernel writes one frame and maps it read-only into every process.
The rate does not change while the machine runs, so one frame is correct. If the rate has not
resolved, the frame stays zeroed and reads as unknown, which keeps every process's layout the same
regardless of boot order.

`load` maps it for the processes it builds. Several kernel paths build a process's space by hand
instead, because each wants a narrower world: `spawn_elf_child_builder`, and the harnesses that
hand a narrowed archive to one named program (`timetable_tests::spawn_timetable`, `authority_tests`'
`root_supervisor` spawn, `c_seam_tests::spawn_confiner`, `login_service`, `live_swap_tests`' `swapper`
spawn). `load`'s mapping reaches none of them, and each would fault the first time it read the
clock. They all call `map_timebase_page`.

`user_address_space_create` maps no timebase page. That syscall serves every purpose that wants a
bare address-space object, most of which build nothing like an ELF process, and one test's
hand-sized region has no table budget for it. The userspace loader
(`supervision_protocol::build_child_space`) writes the page itself from the rate the parent holds,
so a parent that knows nothing hands down nothing rather than a plausible constant.

## The registry

Since §249 the registry owns every space, bound to a thread or not, so a running process holds an
entry for as long as it lives. A capability is a name, and it stops resolving once the space is
gone. Deleting one removes nothing (§249's amendment (a)).

`MAX_USER_SPACES` is `crate::revoke::MAX_SPACES`. Every space registers with the revocation registry
when it is built and leaves only when it drops, and holds one entry here, so this table cannot fill
while a space that wants an entry exists. That is why `register_bound_address_space` panics on a
full table rather than handing every kernel spawn path an error it could never act on. At 288
slots the table is 21,904 bytes of `.bss` on all three architectures (`registry_footprint`): 72 bytes
an entry and a 4-byte generation each. Nothing walks the empty slots, because
`generational_table::Table` bounds every sweep by its highest live slot.

### Holders, and the copy the switch reads

A space has at most one holder: a thread configured with it directly, or a process whose members run
in it (milestone 812). Threads share a space by joining its process (`BIND`), and a second holder is
refused. calef ruled this on pull request #1892, retiring §249's amendment (b) and the §105 (`std::thread::spawn` stays declined) refusal
of `std::thread::spawn` that rested on it.

A bound thread keeps a `BoundSpace`: the registry's name, the root, the composed root register and
the current-CPU page's address. The context switch holds `IPC_TABLES`, and the registry's lock
ranks above it, so the switch reads the copy instead of the registry. All three values are fixed for
the space's life. The copy is safe because of who may drop the space: a bound space is dropped only
once its thread can never be switched in again.

`bind_user_address_space` is `CONFIGURE`'s engine. It calls `bind` under the registry lock, and
`bind` takes `IPC_TABLES` (60, below the registry's 61). That makes the bind one critical section
across both tables, so no region sweep can see a space marked bound to a thread that does not have
it. The current-CPU page is attached first, because the copy carries its address. A space whose bind
is then refused keeps the page, which is harmless.

### How a space leaves

There are two removers, and a removal is take-once. The reaper removes a space by the name in its
thread's `BoundSpace`, and the region sweep `reap_address_spaces_in_region` takes the rest.
`Table::remove` hands the space out once and kills the name in the same step, so whichever comes
second finds nothing. The space drops outside the registry lock, because its `Drop` takes the
revocation, region and ASID locks.

When a region is destroyed, the sweep takes three kinds of entry:

1. An unbound space whose root is in the region, built from it and never bound.
2. A bound space whose thread is gone from the thread table, wherever its root lives. That is a
   thread `reap_region_objects` just removed because its TCB was in the region. Removing a thread
   drops nothing of its space, so the space is collected here, before the region is unpinned.
3. A bound space whose root is in the region and whose thread is a corpse no core stands on (`Dead`
   or `Finished`, off its stack). It can never be switched in again. Taking it closes the gap where
   a corpse kept a root the region had given back, and its later drop forgot the revocation records
   of whoever got that page next.

A bound space whose thread can still run is left alone, root in the region or not. Milestone 765 (a
destroyed region cannot free the root a running thread walks) closes that hole. It makes
`reap_region_objects` refuse and kill such a thread first; until then the hole is recorded at
`revoke::revoke_region`. A corpse still standing on its stack counts as runnable,
because a core still has its root installed.

## Asking the hardware about a space

`AddressSpace::while_installed` runs a closure with the space installed as the current user half,
so a kernel thread can ask the hardware's tables a question rather than read back the kernel's
record of them. It is a closure rather than a guard whose `Drop` uninstalls, because `mem::forget`
is safe, and a forgotten guard would leave the space installed under a root register after the
space dropped.

Interrupts are masked from the install to the uninstall. The scheduler does not know about this
install: the calling kernel thread has no space of its own, so a switch back to it puts the reserved
root in place. A timer tick inside the closure that switched away and back would uninstall the space
under the question, and `translate_user` would answer for the kernel's empty low half. Masking also
keeps the thread on its core. `address_space_tests::an_installed_space_stays_installed_across_a_timer_tick`
is the witness. Tests that read through the user translation itself, or install a space on another
core, carry their own arguments in `system_tests::user::tests`.

## Handing a test service's memory back

A test boot has one pool of frames, and the aarch64 suite builds a service, proves something with
it, and moves on. `user::holding::Holding` gives back what the kernel handed such a service
([frames](frames.md) has the ledger that found the leak). It is bookkeeping over DECISIONS §16 (object revocation)'s
existing verbs, in the order that works:

1. Arm the kill on every thread first, so a thread the reclaim wakes is already doomed and cannot
   get work done with capabilities about to go stale.
2. Reclaim each region, retrying. The first pass is what arms the kill on a live resident, and only a
   later pass finds it converted. Regions go in the order they were added, so a region `SPLIT` from
   another goes first; a parent with live children refuses.
3. Wait for each thread's name to stop resolving, because its address space and the region behind it
   are freed by the `Drop` chain at reap, and the ledger reads right after.
4. Only then reclaim the regions a live thread was standing on, such as a process's stack.

The deadline is two seconds of wall clock rather than a yield count, for the reason in DECISIONS §28
(SMP placement). A killed thread is converted by its own core's next tick, and a hundred yields here
can finish inside one tick there.

What a holding cannot end is a service blocked for good. The armed kill is spent by `schedule()`,
which a blocked thread never reaches. Reclaiming a region removes the endpoints inside it and wakes
their waiters with an aborted IPC, so a service blocked on an endpoint its own budget paid for does
die. One blocked on an endpoint from somewhere else does not, and `release` returns `false` rather
than pretending (milestone 133 (ending a permanently blocked thread)'s question). A caller parked in `CALL` is a different case: it is
freed the moment its server can no longer answer, wherever the caller lives (milestone 254 (a caller stranded by a server that died)).
