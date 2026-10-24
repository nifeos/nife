# Giving up a mapping: `AddressSpace::UNMAP`

Milestone 95 (an unmap primitive, and the mappings init never lets go), built 2026-10-05 (UTC) on
§162 (whether a holder can give up a mapping), which calef decided as option A that day. §162 left
two semantics owed by this milestone; both were built here, argued below, and ruled by calef the
same day as built (§162's amendment of 2026-10-05). The method number (2) is the next free one on
the object, and `forget_mapping_at` is a provisional name.

There was a third finding, and it mattered more than either semantic: **as the object model stood,
`UNMAP` could not reach the window §162 was decided to close.** It is under
[the finding](#the-finding-no-capability-names-a-running-space), kept as it was found. §249 (a
running address space stays nameable) answered it the same day, and the second build of this
milestone, under [the progenitor gives its windows back](#the-progenitor-gives-its-windows-back),
is what that answer made possible.

## What it does

`invoke(aspace, UNMAP, va, 0, 0)` takes the one page the space maps at `va` out of three places, in
one hold of the mapping registry:

1. The space's page tables (`arch::mmu::unmap_user_at`, the function every revoke uses).
2. Every core's TLB. aarch64 issues `tlbi vaae1is` (every ASID, every core in the inner-shareable
   domain). riscv64 runs a local `sfence.vma` and an SBI remote fence to every other online hart.
   x86_64 runs `invlpg` and the NMI shootdown, and with `CR4.PCIDE` off a space not installed on a
   core has nothing cached there.
3. The space's mapping record, so `LIST` stops reporting the page and a later revoke of the frame
   does not reach `va`.

It needs `WRITE` on the address-space capability, which is the right `MAP_INTO` takes. It touches
no capability and no other space.

## EXAMPLES

A builder that mapped the wrong page into a child before starting it:

```rust
// SAFETY: invoke is the syscall; the kernel validates the capability, method and address.
unsafe { invoke(aspace, abi::address_space::MAP_INTO, va, frame, abi::address_space::MAP_RW) };
// ... wrong frame. Give the page up and map the right one at the same address.
let r = unsafe { invoke(aspace, abi::address_space::UNMAP, va, 0, 0) };
assert_eq!(r, 0);
unsafe { invoke(aspace, abi::address_space::MAP_INTO, va, right_frame, abi::address_space::MAP_RW) };
```

Asking twice is an error the second time:

```rust
assert_eq!(unsafe { invoke(aspace, abi::address_space::UNMAP, va, 0, 0) }, 0);
assert_eq!(
    unsafe { invoke(aspace, abi::address_space::UNMAP, va, 0, 0) },
    abi::Error::BadPointer as i64,
);
```

The proof is `system_tests/src/user/unmap_tests.rs`, four tests on all three architectures, each with
a replayable falsification under `system_tests/falsifications/user.unmap_tests.*`.

## Semantic one: `UNMAP` of a `va` with nothing mapped answers `BadPointer`

Provisional behavior. Refused with `BadPointer`, nothing changed. The same answer covers a
misaligned or kernel-half `va`, and a page already given up.

1. What else was considered. Success with nothing done, which is idempotent. It loses on what
   the method is for. A program calls `UNMAP` to close a window. If it passes the wrong address,
   an idempotent answer says the window is shut while it is still open, and nothing tells the
   program otherwise. A new error code (`NotMapped`) was also considered. It loses because adding a
   code to `abi::Error` is a wire change of its own, and `BadPointer` already means "not memory you
   could have touched yourself", which an unmapped page is.
2. What the tree does in the analogous case. `MAP_INTO` of an address already mapped answers
   `BadPointer` rather than succeeding or replacing, and `supervision_protocol::map_scratch` relies
   on that refusal as its free list. `UNMAP` is the same rule read the other way. Inside the
   kernel, `MappingHold::forget_mapping` is silent on a missing record, but it is a rollback, and
   for a rollback "nothing left to undo" is the right answer.
3. Prior art, read on 2026-10-05. It splits. Linux `munmap(2)` succeeds ("It is not an error if
   the indicated range does not contain any mapped pages"). seL4's aarch64
   `performPageInvocationUnmap` unmaps only when the frame capability records a mapping, and returns
   success either way. Zircon `zx_vmar_unmap` answers `ZX_ERR_NOT_FOUND` ("Could not find the
   requested mapping"). Mach's `vm_deallocate` man page answers `KERN_INVALID_ADDRESS` ("The address
   is illegal or specifies a non-allocated region"). seL4's case differs in kind, since its unmap names a capability that
   knows its own mapping, so "is there a mapping" is never ambiguous there. The two systems whose
   unmap names an address, as ours does, split one each way.
4. Is the premise true? Yes. `mmu::unmap_user_at` already returns `None` for an empty address,
   and the record lookup returns `None` for an absent record, so the handler can tell the two cases
   apart at no cost.
5. Cost. One branch. Neither option is cheaper to build.
6. Reversibility. This is the more reversible of the two. A caller written against
   `BadPointer` that treats it as "nothing there" keeps working if this ever becomes success. A
   caller written against idempotent success breaks if it ever becomes an error. Nothing outside
   this branch has called the method.
7. Same cost? Yes. The choice is about what a caller can learn, not effort.

## Semantic two: the frame capability survives the unmap, untouched

Provisional behavior. `UNMAP` reads, consumes and changes no capability. A frame capability
the caller still holds can map the page again; one already deleted was never needed.

1. What else was considered. Consuming the frame capability the page was mapped under. It does
   not work, and the reason is structural: `UNMAP` names an address, not a capability. The mapping
   record stores the run's base address, which every derivative of a capability shares (§132 (what
   `PageFrame::REVOKE` owes an overlapping run)), so there is no single slot to consume. And in the
   case §162 was decided for, there is nothing to consume: `supervision_protocol::fill_and_map` and
   `system_initializer::receive_image` delete each frame capability right after mapping it. A
   second alternative, refusing `UNMAP` unless the caller still holds the capability, loses for the
   same reason. It would make the progenitor's windows impossible to give up.
2. What the tree does in the analogous case. Mapping does not consume either: `MAP_INTO` and
   `PageFrame::MAP` leave the capability in its slot, and the tree's builders delete it themselves.
   `PageFrame::REVOKE` deletes capabilities because taking authority back is what it is for. That
   is §13 (capability revocation and untyped reclamation), and `UNMAP` is not a revoke. It gives up
   one space's use of a page, and the capability is a separate authority.
3. Prior art, read on 2026-10-05. seL4's `performPageInvocationUnmap` clears the mapped address
   and ASID on the capability in its slot and leaves it there, remappable. Zircon's `vmar_unmap`
   page does not mention the VMO handle; that it survives is recalled, not read.
4. Is the premise true? One part of §162's framing is worth checking: "whether the frame
   capability survives" assumes the unmap can find one. It can find a family (the record's object
   word), never a slot, which is why consuming was never really on the table.
5. Cost. Nothing either way. The surviving behavior is the absence of code.
6. Reversibility. Consuming later would break any caller that remaps after unmapping. Nothing
   does yet.
7. Same cost? Yes.

Neither contradicts §13 or §41 (the endpoint is the broker, and a device is revoked by taking it
back), which §162 required. `UNMAP` touches only the named space's record, so a revoke still finds
every other mapping of the page. A device page the space gave up is no longer in that space, which
is what §41's take-back would have done to it anyway, and the device capability is untouched.

## The finding: no capability names a running space

§162 decided that "this space gives up this window". The window milestone 95 exists to close is the
progenitor's scratch window, which lives in the progenitor's own, running address space. **No
capability names a running address space, the caller's own included**, so `UNMAP` cannot be invoked
on it.

The evidence, each one a lookup:

- `AddressSpace` capabilities are minted in one place, `RETYPE_OBJ(ADDRESS_SPACE)` (`kernel/src/syscall.rs`).
  The kernel grants none at boot, so init holds none for the space it runs in.
- `ThreadControlBlock::CONFIGURE` consumes the builder's capability and calls
  `user::take_user_address_space`, which removes the name from `USER_SPACES`. A copy made earlier
  resolves to nothing afterwards: `user_address_space_root`'s own comment says so for `LIST`.
- The progenitor maps its scratch pages with `PageFrame::MAP`, which acts on the caller's own space
  and needs no address-space capability at all.

So `UNMAP` today reaches a space under construction, which serves a builder that mapped a wrong
page, and nothing more. The negative control §162 and the roadmap block name (the progenitor writes
to a boot server's page and faults) cannot be built until a running space can be named. This is a
design fork on the syscall surface, so it is an architect's call and this lane did not build an
answer. The proposal is
`design/roadmap/0789-a-running-process-can-name-its-own-address-space.md`, with the options and
a recommendation.

This bears on the running-root fix
(`design/roadmap/proposals/a-destroyed-region-cannot-free-a-running-root.md`). Suppose the answer
keeps a bound space in `USER_SPACES` rather than moving it into the thread. Then
`reap_address_spaces_in_region` sees bound spaces for the first time, and that fix's options A and B
change shape. The two should be decided together, or the second written knowing the first.

## The progenitor gives its windows back

Built 2026-10-05 (UTC) on §249 (a running address space stays nameable), by the lane
`lane/space-naming-build`.

- **The registry owns every space**, bound or not (`kernel/src/user.rs`, `USER_SPACES`). `CONFIGURE`
  consumes the capability it is passed and binds the space in place, so a copy made first keeps
  naming it. A thread keeps a copy of its root, its tag and its current-CPU page
  (`user::BoundSpace`), because the context switch runs under `IPC_TABLES` and must not take the
  registry's lock. A space leaves the registry when its thread is reaped or the region sweep takes
  it, and `Table::remove` hands it to one of the two.
- **A second bind is refused** with `WrongObject`, from a bound mark on the entry.
- **The kernel grants the progenitor its own space** at slot 28, `WRITE` alone
  (`system_initializer::BootEndowment::own_space`). It hands the slot to
  `supervision_protocol::give_up_scratch_through` before it builds anything, and the loader then
  `UNMAP`s each scratch page the moment the page is in the child (`give_up_own_page`). The same call
  closes the three other windows it opened onto memory it hands on: the shell's output page after
  the last boot line, and the virtio-rng and NIC DMA pages once their physical base is read.

The proof is `system_tests/src/user/running_space_tests.rs`, six tests on all three
architectures, each with a replayable falsification:

- `UNMAP` through a copy made before `CONFIGURE` faults the running thread.
- A reader spinning on one core faults when another core unmaps its page.
- A second bind is refused.
- A space dies with its thread and not with its capabilities.
- A corpse does not keep a space its region gave back.
- Milestone 95's negative control: a builder holding its own space faults on the page it filled
  for a child, and one without it writes the same page.

## What it costs

`UNMAP` is spawn-time and teardown work, never a step of the IPC round trip, and its body is
`#[inline(never)]` outside `invoke` for the reason `memory_region_map` gives. The dispatch arm adds
one rights check to `invoke`. No `icount` row exists for unmap itself, so nothing here says what one
call costs. The record lookup is a linear scan of the space's log, the same shape and order as
`forget_mapping`, at most `LOG_ENTRIES` (170) entries a page.

What §249's build costs, measured with `script/bench` on aarch64 (TCG and `icount`, single hart),
the branch against its base `c8b5fd09e`, 2026-10-05:

| row | base | branch | change |
|---|---|---|---|
| `spawn_el0` | 1,429,607 | 1,452,643 | +23,036 (+1.6%), 230 ticks a spawn |
| `spawn_reap` | 229,728 | 230,613 | +885 (+0.4%) |
| `map_el0` | 355,903 | 356,621 | +718 (+0.2%) |
| `ctx_switch` | 3,166,067 | 3,156,704 | -9,363 (-0.3%) |
| `ipc_rtt` | 1,088,038 | 1,064,781 | -23,257 (-2.1%) |
| `yield_switch`, `null_syscall` | | | identical |

The spawn rows pay for `CONFIGURE` taking `IPC_TABLES` under the registry's lock and for the reaper's
registry take, which is the cost the ruling buys. The switch rows did not get slower: the switch
reads three words from the thread instead of following a pointer into an `AddressSpace`. The IPC
improvement is not claimed; `icount` counts move with code layout (notes/benchmarks.md), and no IPC
path changed. The registry itself is 21,904 bytes of `.bss` at 288 slots.

## What the running-space tests leave open

- The multicore test's placement is retried, not pinned, because the kernel has no affinity
  primitive. The unmapper stands down when it finds the reader on its own core, and the test picks
  a fresh target, up to 16 times. Sixteen co-placements in a row, or a migration between that check
  and the `UNMAP`, still fail a correct kernel on the guard's "proves nothing about another core's
  TLB" assertion. The guard fired once on a correct kernel, on the cpu-matrix `rv64` leg of CI run
  37405717281 (2026-10-06 UTC), on a branch that touched neither the test nor the scheduler.
- The negative control proves the loader, not the boot. It drives the `supervision_protocol` code
  the progenitor builds boot servers with, in a fixture granted its own space the way the kernel
  grants the progenitor's. A boot that wrote into a boot server would kill the progenitor, so the
  boot itself is proven only by building every server through that code (`script/swish-check`)
  without faulting.
- The negative control keeps 32 frames the first time it runs in a boot, and none after. Measured
  on aarch64, 2026-10-05, by running the control, the real run and a third witness in one test: 32
  frames went after the first witness, and the count held across the next two. It is one-time
  growth the first spawned program of a boot pays, not a frame per process. `scratch_window_tests`
  records the same 32 for its own first builder, unexplained. Run alone, the test is that first
  spawn; in the whole suite something earlier is.
- Only the fault's address is asserted, not its kind. riscv64 reports a load or store page fault
  without saying whether a translation existed, so "it faulted at this page" is the claim all three
  ISAs can state alike.

## The scratch window test

`system_tests::user::scratch_window_tests` proves milestone 604 (the builder's scratch cursor is
bounded), the loader's wrapping window, which needs no `UNMAP`. What it leaves open:

- CI does not run it. The whole suite skips it, and it runs only when named (`script/test --test
  scratch_window`). It was run that way on all three architectures on 2026-09-26, and milestone
  604's block has the times. A gate that only runs by hand is weaker than one CI runs; the cost
  below is the kernel's, and the default suite would pay it on every leg.
- It takes five seconds on aarch64 alone and 104 s in CI's whole aarch64 suite (run 36279440107),
  and on `x86_64` it ran past the leg's timeout. `crate::revoke::revoke_region` finds each page to
  unmap by scanning every live address space's mapping log from the start, once per page. A
  region's destroy costs its mapped pages times every record on the machine, and forty 668-page
  reaps behind a suite's worth of live spaces pay that forty times. The progenitor reaping a job
  pays it too. Milestone 658 (a region reap scans every mapping on the machine per page) is the fix.
- It keeps 32 frames. Measured by the suite's frame ledger on aarch64, run alone, 2026-09-26: both
  regions reclaim without error and the exerciser exits, yet 32 frames do not come back. 32 is also
  `supervision_protocol::SCRATCH_TABLE_PAGES`, which points at the tables the builder's own region
  paid for. That is a guess from one number, not a trace.
- It proves the loader, not the progenitor. The progenitor's own numbers (its 128-page table
  budget, its job pool against the window) are compile-time assertions in
  `crates/system_initializer`, not a boot that spawns `rg` a hundred times. `rg` is in no CI
  archive, for the reasons of milestone 121 (`ripgrep` on nife).

## BUGS

- **A builder can give its windows back only if its builder granted it its own space.** The
  progenitor is the only process in the tree that holds one. The shell's job-frame window and
  `login`'s connect window still never shrink (each file's own comment says so); §249 leaves that
  grant to whoever builds them, and nothing has asked for it.
- **A page a job's image arrived in is given up after the copy, but the job's own pages are not**,
  because they are the job's region's to take back when the job is reaped, as they always were.
- One page per call. A run `MAP_INTO` laid down in one call takes one `UNMAP` per page. §162
  ruled `UNMAP(va)`, and a count argument would be a second semantic to argue. Unmeasured.
- Intermediate page tables stay linked, and their `TABLE` records stay in the log. They come
  back when the space or the region that paid for them dies, as before.
- **The multicore test's falsification is aarch64's alone.** It narrows `tlbi vaae1is` to the
  local core; the riscv64 and x86_64 shootdowns are different code with no recorded patch, so the
  test proves the other two flushes reach the reader. No replay shows it would notice if they did
  not.
