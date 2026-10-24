# The syscall boundary: five calls, one decoder, and where each method's code sits

*Name: provisional, minted 2026-10-23 (UTC) by comments/929-kernel-syscall-selftest.*

`kernel/src/syscall.rs` and its children under `kernel/src/syscall/` are the whole of what a user
program can ask the kernel for. This note carries the design those files' doc comments link to. The
semantics of each method are recorded in `design/decisions/`, and the wire contract is in
`crates/abi`.

## The surface

```text
  exit(code)                          authority over yourself
  exit_thread(word)                   likewise, one thread of yourself
  yield()                             likewise
  cap_delete(slot)                    likewise: your own capability table is your own
  invoke(cap, method, a0, a1, a2)     EVERYTHING ELSE
```

All but one are authority over the caller itself, and the last is everything else. That property
matters more than the count. There is no `open`, `read`, `write` or `fork`: a process can only act
on things it was handed. DECISIONS §4 (kernel shape: monolithic, deferred, with two cheap rules)
keeps the surface narrow, "a boundary, not a habit". §8 (process model / syscall ABI: deferred to a
hard decision point) deferred the choice, and §10 (process model: capability-based, microkernel)
made it.

No pointer crosses the boundary. Every argument is a scalar in a register: a capability slot, a
method, an address, a word. So the kernel follows no user pointer and there is no confused deputy.
`mmu::user_can_read` is kept for the first syscall that takes a user pointer.

`invoke` looks the slot up in the calling thread's own table, which lives in kernel memory. That
lookup is the security mechanism. An empty slot answers `NoSuchSlot`, meaning there is nothing
there, which is different from a refusal.

## Where each method's code sits, and why

`script/fastpath-footprint` bounds the bytes an IPC round trip fetches. Its `syscall_entry` half sums
a flat list of symbols, `syscall::dispatch` among them, with no call closure. So the placement of
every method body is a measurement decision, and there are three placements.

- `dispatch` is `#[inline(never)]`, so the gate can name it. An inlining flip moves bytes into
  or out of a flat sum while the code a syscall fetches stays identical. Milestone 220 (this kernel
  drives no clock or reset controller, and the first real device will need one) added a dependency.
  LLVM then folded the dispatcher's 1,160 bytes into the aarch64 exception handler, and the gate
  read a 35% shrink. Milestone 368 (`script/fastpath-footprint`'s entry set is flat, so an inlining flip
  can move 12% into it) records the pattern. The attribute also stands on its own: the dispatcher is
  a large match run once per syscall, not worth copying into a handler that also serves faults.
- The round trip's arms are `#[inline(always)]`: `SEND`, `RECEIVE`, `SEND_CAP`, `RECEIVE_CAP`,
  `CALL` and `SURVEY` in `rendezvous.rs`. The release profile
  builds the kernel in several codegen units without LTO. A child module can land in another unit
  than its parent, where a plain private function is not inlined at all. The round trip would pay a
  call, and its bytes would leave `syscall_entry` under a symbol the gate does not count. `SURVEY`
  is not on the round trip, and stays inline because it was inline before the arms moved out.
- The `MemoryRegion` decoder arm is `#[inline(always)]` too, for a different reason. It is decoder
  code, and its rights checks are counted inside `syscall_entry`. Out of line, it would leave the
  measured set and read as a shrink nothing on the syscall path earned. Its method bodies are out
  of line like the rest.
- Every other method is `#[inline(never)]`, out of line, the rule of milestone 156
  (`syscall_entry`'s measured size is every method combined; extract the rest and ratchet both
  ways). None of them is a step of the round trip, so each costs the entry one call. `PortRange`'s
  `REVOKE` is also `#[cold]`, because the bound counts the non-cold calls reachable from the
  dispatcher, and cold keeps it and `revoke_port_range_from_others` out of that closure.

What folding one administrative method into the decoder cost, measured:

| method | what it did to `syscall_entry` |
|---|---|
| `MemoryRegion::USAGE` | 5.6% larger on riscv64 and 8.8% on `x86_64`, over the 5% bound |
| `Reply::REPLY_CAPABILITY` | 144 bytes (8.6%) more on `x86_64`, for a method only the network stack uses |
| `ThreadControlBlock::SET_THREAD_POINTER` | aarch64 6% over the 5% bound |
| the `AddressSpace` futex arm | aarch64 over the 5% bound |
| `MAP_INTO`'s rollback and tail check | riscv64 over the 5% bound |

`REPLY_CAPABILITY` looks its Reply up again rather than take it from `invoke`, so the arm in `invoke`
is a jump; a cold method pays one table read for that.

## The rights each method asks for

| object | method | right |
|---|---|---|
| `Rendezvous` | `SEND`, `SEND_CAP`, `CALL` | `WRITE` on the endpoint (`SEND_CAP` also `GRANT` on what it sends) |
| `Rendezvous` | `RECEIVE`, `RECEIVE_CAP`, `REAP` | `READ` |
| `Rendezvous` | `SURVEY` | `ENUMERATE` |
| `Rendezvous` | `BADGE` | `GRANT` |
| `Reply` | `REPLY`, `REPLY_CAPABILITY` | `WRITE` (minted `WRITE`-only, without `GRANT`) |
| `AddressSpace` | `MAP_INTO`, `UNMAP` | `WRITE`, and the shape's right on the frame |
| `AddressSpace` | `LIST` | `ENUMERATE` |
| `AddressSpace` | `WAIT`, `WAKE` | `READ`, on the caller's own space |
| `MemoryRegion` | `MAP`, `RETYPE`, `RETYPE_OBJ`, `SPLIT`, `DESTROY` | `WRITE` |
| `MemoryRegion` | `USAGE` | `ENUMERATE` |
| `PageFrame` | `MAP` | `WRITE` for read/write, `READ` for read-only |
| `PageFrame`, `DeviceFrame`, `PortRange` | `REVOKE`; `PageFrame::SLICE` | `GRANT` |
| `ThreadControlBlock` | every method | `WRITE` |
| `Timer` | `ARM`, `CANCEL` | `WRITE` (`ARM` also `WRITE` on its notification) |
| `Process` | `DESTROY` | `WRITE` |
| `Irq` | `WAIT`, `ACK` | `READ` |
| `Virtio` | every method | `WRITE` |
| `Reboot` | `REBOOT` | none: holding it is the authority |

`REAP` takes `READ` because the right to collect a death is the right to receive deaths there
(§32 (a supervisor may collect a corpse without being able to build one)). Reaping through
`MemoryRegion::DESTROY` needed `WRITE` on the region, which is also the right to build. The tid
`REAP` names is authorized relative to the endpoint, so it is a name inside a relationship.

`SURVEY` takes `ENUMERATE`, not `READ`, because `READ` also unlocks `RECEIVE` and `REAP`. A
viewer granted it could reap a child. A holder without `ENUMERATE` is refused rather than shown an
empty domain, because a monitor that reports nothing when it could not look is the worst failure
it has. `LIST` and `USAGE` follow the same rule one object over.

`DeviceFrame::REVOKE` spares the invoker's own capability and mapping, unlike a frame revoke. The
kernel mints a `DeviceFrame` once, at boot, so a symmetric revoke would make the device unreachable
for the rest of the machine's life (§41 (the endpoint is the broker, and a device is revoked by
taking it back)).

## What an aborted send answers

A `SEND`, `SEND_CAP` or `CALL` that did not happen answers one of two errors. `Gone` means the
endpoint was revoked or reclaimed, and it is not `NoSuchSlot`: the slot holds a real capability to
a destroyed object. A pipe writer branches on that difference (milestone 50 (pipes and redirection:
one sink protocol)), which is why the ABI grew a variant rather than the sink protocol a heartbeat.
`NotPermitted` means the endpoint carries an interrupt, whose driver reads `w0 = 1` as "the device
fired", so no program may deposit there (§101 (notification objects), its amendment "an endpoint bound to an interrupt refuses every send"). The
refusal is asked for only on the abort branch, so a send that went through pays nothing for it.

## Mapping a run of pages

`PageFrame::MAP` and `AddressSpace::MAP_INTO` map a whole run in one call. Both apply one guard and
one rollback.

- Both ends are checked before anything is mapped. `run_end_va` computes the last page with the
  multiply checked as well as the add. An unchecked multiply wraps for a `count` near a multiple of
  2^52, gives a `last_va` a few pages above `va`, and admits a run spanning the address space.
- The pages between need no check, because the user half is a contiguous prefix of the address
  space. On aarch64 `is_user_page_va` is `va & 0xfff == 0 && va >> 48 == 0`, so an aligned address
  between two user addresses is itself one. A widening of `Half::Low` would break this silently.
- All or nothing. A failure partway unmaps and forgets what the call had mapped, so a caller
  told `OutOfMemory` or `BadPointer` never has to ask how much landed. A half-mapped shared surface
  is a peer reading pixels that are not there.

`kernel/src/syscall/proofs.rs` proves both halves of the guard. The second harness models the two
call sites' control flow, which is a caller's code rather than a function, so no harness can call
it instead. Milestone 213 (a harness that re-implements the code instead of calling it proves
nothing about the code) checked the model against both call sites on 2026-09-02. Nothing re-checks
it: a call site that drops one end would leave the harness green.

The frame is read under the `revoke::MappingHold` that maps and records it, so a revocation sweep
cannot fall between the read and the record. `MAP_INTO` and `UNMAP` take the address-space registry
first, because it ranks above the mapping registry. An `UNMAP` and a `MAP_INTO` of one `va` are
therefore ordered whole, and a revoke's unmap pass finds the record and the page together or
neither. `MemoryRegion::MAP` holds the same hold across its retype, so a `DESTROY` either refuses
the retype or finds the record.

`UNMAP`'s TLB obligation is `mmu::unmap_user_at`'s, the function every revoke unmaps with:
`tlbi vaae1is` on aarch64, a local `sfence.vma` and an SBI remote fence on riscv64, `invlpg` and the
NMI shootdown on `x86_64`. A running space resolves since §249 (a running address space stays
nameable), so that flush is what reaches its thread's core.

An executable mapping made from userspace calls `sync_icache` (`fence.i` on riscv64, a data-cache
clean and instruction-cache invalidate on aarch64). The kernel's own loader does the same, and a
fast spawn-and-reap loop (`bench::spawn_el0`) is what stresses the userspace path.

`MemoryRegion::MAP` refuses a bad address before it retypes a page, or each refusal would spend a
page of the caller's region. No capability names a page it maps (§132 (what `PageFrame::REVOKE`
owes an overlapping run)), and reclamation finds the record anyway, because `revoke_region`'s
unmap sweep is blind to which object made a mapping.

## What a spending method mints

`RETYPE`, `RETYPE_OBJ` and `SPLIT` mint through `Cap::mint_child`, which gives the new capability
the budget's rights and no others (milestone 824 (a retype mints no right its budget lacks)).
`split_never_widens_rights` in `crates/capability` proves the mint never widens. The root budget is
`Rights::ALL`, so the progenitor gets every right on what it makes, and a right added later reaches
it without a hand-kept list. `GRANT` is delegated down the budget tree and never minted at a leaf.

`SPLIT` with a full capability table gives the child back. The child is unpinned, childless and at
the top of the parent's watermark, so `destroy` returns its pages and the parent is as it was.

## SURVEY's selector and what a placement leaks

`SURVEY`'s `record` argument selects which per-thread fact comes back in x2, so x0 and x1 are the
same for every record (calef, 2026-09-21). Linux reached 52 fields through a pseudo-file and Zircon
41 topics through a selector; nobody grows a register row. `record::STATE` is 0, so a caller written
before the selector keeps working, and `abi`'s own tests assert it.
[process-view/survey-design.md](process-view/survey-design.md) has the placement record's authority
argument against §150 (how does a thread's CPU time reach userspace?).

One cost that argument leaves out is accepted: a placement leaks one noisy bit per spawn about
other domains. `pick_spawn_target` samples two cores and takes the shorter run queue, which counts
every domain's threads. So a placement says which of two random cores was lighter at that instant.
Finding 9 of [the 2026-09-24 audit](../design/audit-reports/2026-09-24-new-trust-boundaries.md)
records it, accepted on the reasoning that accepted the counting channel of 2026-08-17.

`AddressSpace::LIST` answers a foreign cursor `BadPointer` (§114 (`pmap` gets its listing:
`ENUMERATE` extends to the address-space object), its 2026-10-06 addendum), and a dead space's stale
cursor `DONE`. It skips an entry whose address no longer translates, which a revoke racing it can
leave, rather than report a kind it made up. The audit that made `ENUMERATE` live on every
address-space capability is in [process-view.md](process-view.md).

## BUGS

- `RETYPE` against a full capability table leaves the run retyped, named by no capability, so
  each such call spends `count` pages until the region is destroyed. `SPLIT` gives its child back in
  the same position; a run has no inverse. `RETYPE_OBJ`'s arms have the same shape, an object
  nobody can name. Each is bounded by the caller's own region.
- A failed multi-page map keeps the page tables its prefix retyped. A region is spend-only, so
  the mapping is undone and the budget is not. [frames.md](frames.md) records it.
- The second run proof's model of its call sites can go stale, as the section above says.
