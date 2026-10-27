# Naming a running address space: what seL4 and Zircon do, and what option A must say

Research for the fork pull request #1678 raised, written 2026-10-05 (UTC) by the
`lane/research-space-naming` lane. Milestone 95 (an unmap primitive, and the mappings init never
lets go) found that no capability names a running address space, and proposed three options in
`design/roadmap/0789-a-running-process-can-name-its-own-address-space.md` on its branch. Option
A claimed seL4's shape from memory. This note reads the sources, maps them onto nife's code, and
says what A must specify before it can be ruled. The file name is provisional. Nothing here changes
the kernel.

## The premise, checked

True, at `origin/main` e1f3ddf8c:

- `RETYPE_OBJ(ADDRESS_SPACE)` is the only mint (`user::user_address_space_create`,
  `kernel/src/user/address_space_registry.rs`), and the name lives in that file's `USER_SPACES`.
- `CONFIGURE` deletes the caller's capability (`thread_control_block_configure`,
  `kernel/src/syscall.rs:1366`) after `sched::configure_thread_control_block`
  (`sched/configure.rs`) has called `user::take_user_address_space`. The space
  moves into `Thread::space: Option<AddressSpace>` (`kernel/src/thread.rs:468`), owned outright.
  The comment at `syscall.rs:1363` says why the delete is there: "a second bind must not find it".
- The thread's death frees the space: `reap_switched_out` takes `t.space` (`sched/switch.rs`) and drops
  it. Init's space is kernel-built and adopted the same way (`adopt_address_space`, `sched/thread_control_block.rs`);
  it never enters `USER_SPACES`.
- A region's `DESTROY` runs `reap_region_objects` (`sched/region_reaping.rs`), then
  `user::reap_address_spaces_in_region`, which sees only unbound spaces, then unpins
  (`reclaim_region`, `sched/region_reaping.rs`). `revoke::revoke_region` (`kernel/src/revoke.rs:847`) carries
  the running-root `BUGS` entry that milestone 765 (a destroyed region cannot free the root a running
  thread walks) closes.

One thing about nife's model matters for every comparison below. **A capability never decides an
object's lifetime here.** A space dies with its thread (bound) or its region (unbound), and a
capability is a generational name that goes stale when the object does. Deleting a capability
destroys nothing. seL4 is the opposite on this one point, and that is where its answers stop
transferring.

## seL4, read

Source: `seL4/seL4` at commit `6df0b6ee61f6a9e8a4ee7de8eb8fd698c0211b64` (master, 2026-10-04),
fetched and read 2026-10-05. Manual text from `manual/parts/*.tex` at the same commit.

**1. The root task does get its own VSpace capability.** `libsel4/include/sel4/bootinfo_types.h`
line 18: `seL4_CapInitThreadVSpace = 3, /* initial thread's VSpace cap */`. `create_initial_thread`
in `src/kernel/boot.c` (lines 516 to 520) does `cteInsert(it_pd_cap, <slot seL4_CapInitThreadVSpace>,
<the TCB's tcbVTable slot>)`. So the root task's slot holds the original and the TCB holds a copy,
linked in the capability derivation tree. The recalled claim was right on this.

Anyone holding a VSpace capability can map into it whether or not a thread runs there.
`decodeARMFrameInvocation`, case `ARMPageMap` (`src/arch/arm/64/kernel/vspace.c` around line 1617),
checks only that the VSpace capability is a valid native root with a live ASID; there is no
running-thread check anywhere on that path.

A thread unmaps its own pages through the **frame** capability, not the VSpace one.
`performPageInvocationUnmap` (`vspace.c` line 1320) unmaps using the ASID and address recorded in
the frame capability itself and then clears them. The root task receives a frame capability for
every page of its image (`userImageFrames`, `bootinfo_types.h` line 65). So in seL4 the authority to
give up a page is the frame cap, and the VSpace cap is the authority to *add* one. §162 (whether a
holder can give up a mapping) refused that shape (its option B, `Frame::UNMAP`) and ruled
`AddressSpace::UNMAP(va)`, and nife's builders delete frame capabilities right after mapping. That
is why nife needs the space named where seL4 would not.

**2. Deleting or revoking the VSpace capability while a thread runs.** `invokeTCB_ThreadControl`
(`src/object/tcb.c` line 1786) and the MCS `installTCBCap` (line 1703) do not consume the caller's
capability: they `deriveCap` and `cteInsert` a copy into the TCB's `tcbVTable` slot. A delete is
"final" only if no capability to the same object sits beside it in the derivation tree
(`isFinalCapability`, `src/object/cnode.c` line 846), and the TCB's copy counts. So deleting the
builder's copy does nothing to the running thread.

Teardown happens when the last copy goes, including the TCB's. That is a revoke of the original or
of the untyped it came from (`cteRevoke`, `cnode.c` line 527, deletes every following capability
for which `isMDBParentOf` holds, wherever its slot lives), or the TCB itself being destroyed.
`Arch_finaliseCap` (`src/arch/arm/64/object/objecttype.c` line 158) then calls `deleteASID` when
`final` and the capability holds an ASID. `deleteASID` (`vspace.c` line 1174) invalidates the TLB by
that ASID, clears the pool entry so the ASID is free for a later `ASIDPool_Assign`, and calls
`setVMRoot` on the current thread. `setVMRoot` (`vspace.c` line 870) reads the TCB's `tcbVTable`
slot. If it is not a valid root, or its ASID no longer maps to that root, it installs the global
empty user VSpace under the reserved ASID. RISC-V does the same (`src/arch/riscv/kernel/vspace.c`,
`deleteASID` line 494, `setVMRoot` line 578, falling back to the kernel-only root).

So the thread is not killed. It stays a TCB object, runs on an empty space, and its next user access
is a VM fault delivered to its fault handler (manual, `vspace.tex` §"Page Faults"). A handler that
holds the TCB could give it a new VSpace with `TCB_SetSpace`. The memory itself comes back only
when the untyped is revoked: "After this invocation, no references remain to any object within the
untyped region, and the region may be safely retyped again" (manual, `objects.tex`, the paragraph
on `seL4_CNode_Revoke`).

**3. Several TCBs on one VSpace.** Each TCB holds its own copy in `tcbVTable`. There is no reference
count. Lifetime is "the last capability in the derivation tree", found by looking at the
neighboring entries (`isFinalCapability`), and memory reuse needs the untyped revoked, which
deletes every copy first. A thread whose space dies under it faults, as above.

**4. A creator's authority after start.** Kept by default: `ThreadControl` copied the capability, so
the builder still holds its own and can map into, or delete its copy of, the running child's space.
Systems built on seL4 decide what children get:

- Microkit: a protection domain gets no VSpace capability unless the system description says so.
  `docs/manual.md` (seL4/microkit at `04a259ea28ac`), section `cspace`: "`cap_vspace`: A capability
  to a protection domain's VSpace", with a `pd` attribute so one domain can be given another's. The
  manual calls the `cspace` element "an advanced feature".
- CAmkES: a component gets its own VSpace capability only when it is configured `simple`.
  `camkes/templates/component.simple.c` (seL4/camkes-tool at `e6fa51fd12d1`) line 13 opens
  `if configuration[me.name].get('simple')`, and line 31 inside it allocates `my_pd_cap` over
  `my_pd`. The template's own header calls it "highly experimental and unsupported".

Both are explicit, opt-in grants decided by whoever builds the system. That is option A's shape,
and neither is option B's.

## Zircon and Fuchsia, read except where marked

- Self. `zx_process_create` returns the process handle and "a handle to the root of its
  address space" to the *caller* (fuchsia.dev, `zx_process_create`). The child gets one only if the
  launcher sends it: `PA_VMAR_ROOT 0x04u`, "Handle to the root of our address space"
  (`zircon/system/public/zircon/processargs.h` line 83). Fuchsia's Rust `process_builder`
  (`src/lib/process_builder/src/process_builder.rs`, main, read 2026-10-05) duplicates its own root
  VMAR handle into the bootstrap message (`add_root_vmar`, line 756). `zx_vmar_root_self()` is the
  C runtime's accessor for that handle (recalled; the header that declares it was not fetched).
- Creator after start. `BuiltProcess` holds `root_vmar` and documents using it "to manipulate
  the process or its address space before starting it, such as when creating a process in a
  debugger". `start(self)` consumes the struct, so the builder's copy is dropped unless the caller
  kept it. Explicit again.
- Lifetime. Handles do not set it. "When the last handle to a process is closed, the process is
  destroyed" (`zx_process_create`). For child VMARs, "if all handles to the child VMAR are closed,
  the child and its descendants will remain active in the address space" (kernel objects,
  `vm_address_region`). `zx_vmar_destroy` on a VMAR leaves outstanding handles valid, "but all VMAR
  operations on them will fail", and refuses the root VMAR (`ZX_ERR_NOT_SUPPORTED`). That a process's
  VMAR handles also fail once it is dead is recalled, not read.
- Shared spaces. Every thread of a Zircon process shares the process's space, and it lives as
  long as the process (the process object, not a count of handles).

Zircon's lifetime model is nife's: the object belongs to an owner and handles are names that can go
dead. seL4's is the derivation tree. Option A should borrow seL4's *authority* shape and Zircon's
*lifetime* shape, and it can, because nife already has the second.

## What changed from the recalled claim

- Right: seL4 hands the root task its own VSpace capability (slot 3), and allows mapping into a
  running space.
- Incomplete: in seL4 the VSpace capability is not how a thread gives a page up; the frame
  capability is. A rests on §162's ruling, not on seL4 precedent, for `UNMAP` itself.
- Different: seL4's `ThreadControl` copies rather than consumes, and the running thread's copy is
  what keeps the space alive. A's "consume, but keep the name" is not seL4's mechanism; it reaches
  the same authority with nife's ownership model.
- Different in the 765 case: seL4 leaves the thread alive on an empty space, to fault. nife kills it.
  That divergence is right for nife, because `CONFIGURE` refuses anything but an embryo
  (the embryo check in `configure_thread_control_block`, `sched/configure.rs`), so a nife thread that lost its space could never be given another.

## What option A must specify

**(a) Deleting a capability, or destroying a region, while a thread runs on the space.** Deleting
any capability to a bound space does nothing to the space. There is no final-capability rule, as
there is none anywhere else in nife. A bound space dies when its thread is reaped or when the region
its root came from is destroyed (765, below). When it dies the name is retired, and every capability
still naming it, in any process, goes stale and answers as `LIST` does today for a consumed name
(`user_address_space_root`'s comment, in `kernel/src/user/address_space_registry.rs`). No
capability-level revoke of a space is added. That is Zircon's "handle valid, operations fail".

Where the `AddressSpace` lives decides most of the code. The cleaner answer is that the registry
owns every space, bound or not, and a thread caches its immutable root and tag for the context
switch. The switch reads the root every time (`schedule`, `sched/switch.rs`) and must not take the
`ADDRESS_SPACES` lock.
Then one owner, two removers (the thread's reap and the region sweep), and `remove` on a generational
table is take-once, so a double free is not representable. The alternative leaves the space in the
`Thread` and makes the registry an alias. That makes every `MAP_INTO` and `UNMAP` on a bound space
resolve through `IPC_TABLES`, below `MAPPINGS` in the rank order. Not built or measured, either.

**(b) Several threads on one space.** Today a space has exactly one thread, by construction: the
consumed capability is the only thing stopping a second `CONFIGURE`, and §105 (`std::thread::spawn`
stays declined) rests on that. Under A the name survives, so a second capability could reach a second
`CONFIGURE`. **A must refuse a second bind explicitly** (a bound flag on the registry entry, refused
with `WrongObject` as an already-started TCB is), or it quietly reverses §105. Sharing, when a
customer asks, then becomes a counted binder on that entry: the space dies with its last thread or
its region. That is seL4's rule written as a count, since nife has no derivation tree to inspect.

**(c) Init's capability.** "The kernel grants init one at boot" is a slot convention, which is a
wire format. seL4 fixes it at slot 3. nife's precedent is `abi::fault::FAULT_EP_SLOT`
(`crates/abi/src/lib.rs:967`), or a grant through `Spawn::grants` in `boot_progenitor`
at a slot the progenitor is told. That needs naming and an architect's ruling; it
is B's mechanism, applied to one program.

## How A composes with 765

765 stands as ruled. Its semantics (a live thread whose bound root is in the destroyed span is a
resident: refuse, kill, retry) are what A needs, and A adds nothing to the syscall surface there.
What A changes is the bookkeeping. With registry ownership,
`reap_address_spaces_in_region` sees bound spaces for the first time. It must not drop one whose
thread can still run, and the existing order already guarantees that: `reclaim_region` calls
`reap_region_objects` first, and 765 makes it refuse while such a thread lives. On the retry the
sweep frees the root in one place, which is simpler than freeing it through the corpse.

One gap, reasoned from the code and not driven: a thread already `Dead` or `Finished` but not yet
reaped still holds its space today, and `region_reap_verdict` (`sched/region_reaping.rs`) says `Reap` for it.
If its TCB is outside R and its root inside, R can come back while the corpse still owns that root.
The corpse's later `Drop` then calls `revoke::forget_root` on a page that may by then be someone
else's root. Under A with registry ownership the sweep takes the space from under the corpse
first. Without A, 765's builder must count such a corpse as holding the root. That is handed to
milestone 765's lane as a question, below.

## The seven questions, A against B and C

1. Considered. A (keep the name, explicit grants). B (kernel grants every thread its own space at
   `START`): loses, because seL4, Microkit, CAmkES and Fuchsia all make the grant explicit and
   optional. None gives every process authority over its own space by default, and a default grant
   cannot be taken back program by program. C (leave it): loses milestone 95's negative control and
   leaves §162's ruling closing no hole the roadmap named. A frame-capability unmap (seL4's real
   self-unmap path) was not reopened: §162 refused it.
2. The tree's analogous case. Every other authority a child holds is a `CAP_INSERT` grant chosen
   by its builder. `FAULT_EP_SLOT` is the one kernel-placed slot, and it is placed by the
   supervisor, not by default.
3. Prior art. Read above. seL4 for authority, Zircon for lifetime; both explicit.
4. Premise. True, as checked. The proposal's framing that seL4's VSpace cap is how a task gives
   pages up was not true.
5. Cost. None measured. A: one registry entry kept per bound thread, which `MAX_USER_SPACES`
   (32, in `kernel/src/user/address_space_registry.rs`) does not hold today, since every process
   would now occupy one; that bound must grow or bound spaces live elsewhere. A cached root and tag
   per thread. B: a slot per thread and a capability minted at every `START`. C: nothing.
6. Reversibility. All three touch the `CONFIGURE` contract or a slot convention, so all are on the
   irreversible list. Who has acted on today's "consumed, dies with it" wording
   (`crates/abi/src/lib.rs:848`): §105 and `notes/thread-spawn-fork.md`, which A keeps true only with
   (b)'s explicit refusal. No program depends on a stale name staying stale.
7. Equal cost. Yes, A. B is likely less work, which is an effort argument and the only one for it.

## Recommendation

A. The evidence strengthens it rather than changing it. It needs three amendments the
proposal does not yet carry: (a)'s retire-on-reap rule with no final-capability semantics, (b)'s
explicit refusal of a second bind, and (c)'s slot for init named as its own decision. Rule it
together with the registry-ownership choice, because that is what lets 765's retry free the root in
one place.

## BUGS

- Nothing here was run. The seL4 behavior is read from source and manual, not exercised.
- The Zircon `zx_vmar_root_self` declaration and the dead-process VMAR behavior are recalled.
- The corpse gap under 765 is closed by registry ownership, per §249 (a running address space stays
  nameable), and driven rather than reasoned:
  `system_tests`' `running_space_tests::a_corpse_does_not_keep_a_space_rooted_in_a_destroyed_region`
  builds the shape. Its falsification, the sweep leaving a corpse's space alone, goes red at
  exactly this gap: the space outlives its region and the root stays registered. What remains is
  narrower and is in milestone 765's block.
