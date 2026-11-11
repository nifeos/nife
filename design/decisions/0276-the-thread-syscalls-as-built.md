---
status: DECIDED
raised: 2026-10-10
decided: 2026-10-10
ratified_by: calef
---

# 276. The thread syscalls as built: the process object, `BIND`, futexes, the thread pointer and `SYSCALL_EXIT_THREAD`

*Section number provisional until the merge queue lands it; 270 was the highest on `main` and 271
to 275 were in open pull requests when this was written. Recorded on 2026-10-11 (UTC) by
maintainer/thread-syscalls-and-bind-decision, from the code merged by pull request #1892 as
`ddd38f0b0`.*

§10 (process model: capability-based, microkernel) requires every new method's semantics to be
written here and not only in code. Milestone 812 (`std::thread::spawn` runs real threads in one
address space) added one object type, one rights bit, one syscall number and five methods or
method forms. §269 (how threads share a process) holds the seven fork rulings that shaped them;
this section holds calef's four further rulings on #1892 and what the merged kernel does. Every
claim below was read from `kernel/` and `crates/` at `ddd38f0b0`.

## The rulings on #1892

All four on 2026-10-10 (UTC), each relayed by a maintainer session as a comment on the pull request
at the time given.

1. **Where `BIND` lives (17:32 UTC).** calef, after the trade-offs and prior art (Zircon's
   `ZX_RIGHT_MANAGE_THREAD` on a process handle, Windows' `PROCESS_CREATE_THREAD`, Mach's
   `thread_create(task)`, seL4 with no process object, LinuxThreads giving way to NPTL): *"It seems
   like b is where the peers are headed."* `BIND` is a right on a new process capability, which
   answers §269's open consequence. Refused: the right on the address space (the space as the group,
   which fork 5 already refused) and the right on both (two authorities for one act).
2. **`CONFIGURE` has no spare words (17:33 UTC).** calef: *"It seems like there is only one option
   while preserving parity."* The thread pointer rides the sixth argument register, and the kernel
   chooses the thread's current-CPU page and returns its address. Refused: the creator passing the
   address, which needs a seventh word riscv64 does not have (§19 (architectural parity is a tenet)).
3. **Ending one thread (17:38 UTC).** calef: *"I don't like the less obvious part. It looks like
   peers aren't passing flags. If they were the same cost, which would you pick?"* At equal cost the
   maintainer picked a separate call, and calef chose it: a new, capability-free syscall number. This
   is the ruling on the new number that `AGENTS.md` treats as a design fork. Refused: a flag in
   `SYS_EXIT`'s second word, and a method on the thread's own capability (ending yourself needs no
   capability).
4. **The aarch64 thread-pointer register (17:40 UTC).** calef chose `TPIDR_EL0`, the toolchain's
   convention, as built. Refused: `TPIDRRO_EL0`, which needs the `tpidrro-el0` target feature for
   everything built for nife and a second register switched per thread.

Two more rulings on #1892 concern dependencies, not surface, and are listed for completeness. At
20:52 UTC calef answered *"a"*: `rayon` is a dependency of `std_exerciser` only, under §46 (thin
primitives or whole subsystems). At 22:48 UTC he answered *"On rg, option a."*:
`helpers/build-ripgrep.sh` patches `ignore`'s parallel walker until upstream carries the fix.

## The surface, method by method

Registers: the syscall number is in `x8`, `a7` or `rax`; the arguments are `x0` to `x5`, `a0` to
`a5`, or `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`
(`kernel/src/arch/x86_64/exceptions.rs`, `TrapFrame::arg`). An `invoke` puts the capability slot
and the method in the first two and three data words in the next three. A negative result is an
`abi::Error`.

### `RETYPE_OBJECT` of `object_type::PROCESS` (6): making a process

`invoke(region, RETYPE_OBJECT, PROCESS, space_slot, floor)` -> slot, method 2. The ruling of
2026-11-11 (UTC) spelled out its name and the module's, `object_type`. `kernel/src/syscall/retype.rs`, `kernel/src/sched/process.rs`
(`create_process_from`).

- Needs `WRITE` on the region (`NotPermitted`) and `WRITE` on the address-space capability in
  `space_slot` (`NotPermitted`). An empty slot is `NoSuchSlot`; a capability that is not an address
  space is `WrongObject`.
- One page of the region becomes the process: a header (the space, a member count, whether it has
  ended and how) followed by the shared capability table. The region pays for it and the page is
  reclaimed only when the region is destroyed and no member remains (`forget_empty`).
- The space must have no holder, else `WrongObject`. The process becomes its holder in the address
  space registry, and the space capability is consumed, as `CONFIGURE` consumes one.
- The new capability carries `Rights::ALL`, `BIND` among them, at the lowest free slot at or above
  `floor` (0 means the first free slot). `OutOfMemory` if the region, the process registry
  (`MAX_PROCESSES`, the thread ceiling) or the caller's table is full.

### The `BIND` right (bit 4)

`abi::rights::BIND` and `capability::Rights::BIND`, `1 << 4`; `Rights::ALL` widened to `0b1_1111`.
Only a process capability consults it. Because every object is created with `Rights::ALL`, every
capability minted since `ddd38f0b0` carries the bit, inert on every other type, the same audit
caveat §114 (`ENUMERATE` extends to the address-space object) recorded for `ENUMERATE`. A delegator withholds it by narrowing, as with any right.

### `ThreadControlBlock::CONFIGURE` (0): joining a process

`invoke(tcb, CONFIGURE, entry, user_sp, slot)`, sixth register `thread_pointer` -> the thread's
current-CPU page address. `kernel/src/syscall/thread_control_block.rs`,
`kernel/src/sched/configure.rs`.

Needs `WRITE` on the TCB capability. The object in `slot` chooses the form:

| | an address space (as before 812) | a process |
|---|---|---|
| right needed on it | `WRITE` | `BIND` (not `WRITE`) |
| the capability | consumed; the space's name survives (§249) | kept |
| the thread | is its own process: it holds the space, keeps its own table, and has no process object or capability | becomes a member: it runs in the process's space with the process's table |
| result | `PAGE_VA`, or 0 if no page could be attached | the address of the lowest free slot of 16, counting down from `PAGE_VA` |

Refusals, in the order checked for the process form. Nothing changes on a refusal, and a page slot
taken for a refused join is given back.

1. `NotPermitted`: no `WRITE` on the TCB capability.
2. `NoSuchSlot`: `slot` is empty.
3. `WrongObject`: neither a space nor a process.
4. `NotPermitted`: no `BIND` on the process capability.
5. `BadPointer`: a thread pointer that is neither zero nor a user-half address.
6. `Gone`: the process has ended.
7. `OutOfMemory`: all 16 page slots are taken, the per-process thread limit
   (`current_cpu_protocol::THREAD_PAGE_SLOTS`).
8. `WrongObject`: not an unconfigured embryo.
9. `WrongObject`: a slot the embryo holds is already taken in the process's table.

On a join, whatever the embryo's own table holds moves into the process's table at the same slot
numbers, under `IPC_TABLES`, so a loader endows a thread before it joins exactly as it always has.
`START` then refuses a member whose process has ended (`Gone`). The first member's `START` takes the
reserved `abi::fault::FAULT_RENDEZVOUS_SLOT` (its name spelled out by the ruling of 2026-11-11
(UTC)) out of the shared table, so supervision belongs to that thread and a member started later is
unsupervised (§26 (the fault endpoint)).

The thread pointer and the page address are set in the same critical section as the space and the
entry, so a racing `START` sees all of them or none.

**One holder per space.** Ruling 1 said §249's bound mark retires. What the code retires is the
one-*thread* limit: threads share a space by joining its process. What it keeps is one *holder*
per space, a thread that is its own process or a process, so a second `CONFIGURE` or
`RETYPE_OBJECT` on a held space is still `WrongObject` and two processes cannot share one space
(`kernel/src/user/address_space_registry.rs`, `bind_user_address_space`). §249 is amended below to
say so.

calef confirmed one holder per space on #1925 on 2026-10-11 (UTC), choosing option A. Processes
share memory by mapping the same frames into their own spaces, as Windows sections, Mach memory
objects and Zircon's native model do. Several processes on one space (Linux's `clone(CLONE_VM)`
without `CLONE_THREAD`) was refused because nothing needs it. nife builds a child from outside, so
spawning needs no `vfork`, and §264 (`fork` is declined for good) refuses the rest. Keeping the
refusal also leaves a later lift as a widening, which breaks no program. What would reopen it is a
Linux-compat personality (milestone 19 (the first real workload)), and its precedent is Fuchsia's
`zx_process_create_shared`, which shares one region of a space rather than the whole space.

### `ThreadControlBlock::SET_THREAD_POINTER` (3)

`invoke(tcb, SET_THREAD_POINTER, value, _, _)` -> 0. Needs `WRITE`. seL4's `SetTLSBase`.

- The target is an embryo, which takes the value at its first switch in, or the caller itself, whose
  field and register change under one hold of `IPC_TABLES` before the call returns. Any other
  started thread is `WrongObject`.
- `value` is zero or a user-half address, else `BadPointer` before anything changes. The same rule
  on all three, because `x86_64`'s `wrmsr` to `IA32_FS_BASE` faults in the kernel on a
  non-canonical value.

Per architecture (`kernel/src/arch/*/thread_pointer.rs`):

| | register | at a switch | can user mode write it? |
|---|---|---|---|
| aarch64 | `TPIDR_EL0` | saves the outgoing value, installs the incoming one; no barrier, since `eret` is context-synchronizing | yes; it persists for that thread only |
| riscv64 | `tp` | nothing: `trap.s` saves and restores it in the trap frame; the first entry builds the frame with it | yes; it persists for that thread only |
| `x86_64` | `FS` base | writes `IA32_FS_BASE` only when the incoming value differs | no: `CR4.FSGSBASE` stays off (§269 fork 4) |

A user write is not part of the contract. `notes/thread-pointer.md` is the scope note.

### `AddressSpace::WAIT` (3) and `WAKE` (4): futexes

`invoke(space, WAIT, va, flags, expected)` -> `0` woken, `1` the word did not hold `expected`.
`invoke(space, WAKE, va, flags, count)` -> how many were woken. `kernel/src/syscall.rs`
(`address_space_futex`), `kernel/src/sched/futex.rs`.

- `flags` must be exactly `abi::futex::PRIVATE | SIZE_U32` (futex2's bit layout). A shared futex or
  another size is `BadMethod`, the forms §269 fork 2 reserves.
- Refusals in this order: `BadMethod` (flags), `NotPermitted` (no `READ` on the space capability),
  `BadPointer` (`va` not 4-aligned or not in the user half), `WrongObject` (not the caller's own
  space). `WAIT` adds `BadPointer` when `va` is not mapped user-accessible; `WAKE` reads no memory,
  so an unmapped `va` has no waiters and answers 0.
- `expected` is the low 32 bits of the third word. `count` is converted to `usize`; `u64::MAX`
  wakes all. Waiters are keyed by (space name, `va`) in 64 buckets of FIFOs
  (`crates/inter_process_communication/src/futex.rs`) and woken oldest first, onto the waker's core.
- `WAIT` returns `Gone` if it was ended without a wake (the thread's region was destroyed under
  it). A `0` may be spurious; the caller re-checks its word.

**Ordering.** `WAIT` reads the word and parks under one hold of `IPC_TABLES`, and `WAKE` takes the
same lock, so a waker that stores and then wakes cannot fall between the check and the park. The
kernel's load is `Acquire`, so what the waker wrote before its store is visible to the woken thread.
It behaves the same on all three architectures; the only per-architecture part is the page-table
walk (`arch::mmu::translate_user`). No timeout yet, and the walk does not hold the space's own lock;
both are in `notes/futex.md`'s `BUGS`.

### `SYSCALL_EXIT_THREAD` (4): ending one thread

`exit_thread(word)`, a bare syscall that needs no capability and never returns; the ruling of
2026-11-11 (UTC) spelled out the names of syscalls 0 and 4. `crates/abi`,
`kernel/src/syscall.rs`, `kernel/src/sched.rs` (`exit_thread`).

- Ends the calling thread only. `SYSCALL_EXIT` (0) keeps meaning "end my process"; no flag was added
  to it. A thread that is its own process ends that process either way.
- If `word` is nonzero, the kernel stores zero to the 32-bit word at `word` in the caller's space
  with `Release` ordering and wakes every futex waiter on it (Linux's `CLONE_CHILD_CLEARTID`,
  FreeBSD's `thr_exit` state word). This happens after the thread's last return from user mode and
  before it departs, on its kernel stack, so a joiner that wakes and reads zero may free the user
  stack. A word that is misaligned, in the kernel half, unmapped or not writable is skipped; nothing
  stops a thread from exiting.
- If no other member of its process is `Ready`, `Running` or `Blocked`, the process ends with it
  (`process::depart`). Embryo members do not keep a process alive, and once it has ended they cannot
  start.

### `Process::DESTROY` (0)

`invoke(process, DESTROY, _, _, _)` -> 0. Needs `WRITE` (`NotPermitted`); `Gone` if the name no longer
resolves. The supervisor's kill of §269 fork 5. A caller that is itself a member ends with the
process by `SYSCALL_EXIT` and does not return. The space is released at once if no member remains; the
page stays the region's until the region is destroyed, and an ended process answers `Gone` to a join.

## How a process ends, and how its blocked members are reaped

A process ends on a member's `SYSCALL_EXIT` or fault, its last running member's `SYSCALL_EXIT_THREAD`,
`DESTROY`, its region's destruction, or its last member leaving the thread table
(`kernel/src/sched/process.rs`, `end`, `depart`, `leave`, `reap_region`). The last keeps revocation
sound: the sweeps reach tables through threads, so a table with no member is never joined again.

Ending it ends every other member (`end`):

- A member `Blocked` and off its core is finished in place, unlinked from whatever it waits on (a
  futex, an endpoint), and never runs again.
- A `Ready` or `Running` member is marked killed and dies at its next preemption, the §16 (object
  revocation) amendment's mechanism. A killed member that parks before then is finished by
  `schedule()` instead of sleeping (`finish_killed_current`).
- A supervised member reports the process's end, `[event, tid, pc, addr, 0]`, from whichever path
  finished it, so `REAP` still names the thread the supervisor built. Exit codes are not carried.

**Reaping.** A member that ends itself is reaped at its own switch-out. One finished in place is
never switched out again, so `process::reap_corpses` reaps it: after the departing member's reap
(`reap_switched_out`) and after `DESTROY`. It also finishes a killed member still parked, and waits
for one still on its core mid-switch-out, a bounded spin of 10,000 rounds with the lock released.
A supervised member is `Dead`, not `Finished`, and waits for its supervisor's `REAP`. The space is
released when the process has ended and its last member is reaped.

This was not in §269. `rayon`'s idle workers sleep on a futex when `main` returns, and without
`reap_corpses` such a program kept its space until its region died. CI's `thead-c906` leg found the
mid-switch-out case. Both are proved on all three architectures by `process_tests` and
`std_threads_tests`, with replayable falsifications.

**Not atomic in time.** §269 fork 5 says destroying a process ends every thread atomically. That
holds for state; a killed member runs on until its next preemption (`notes/processes.md`, `BUGS`).

## The current-CPU page and the `std` slots

`PROCESS_PAGES` is the 64 KiB below `0x8000_0000`, 16 pages, and `PAGE_VA` is its top page. Slot `k`
is at `PAGE_VA - k * 4096`
(`current_cpu_protocol::thread_page_va`). Each page is read-only to user mode and carries the
magic, the current core, and an allowance word: the online core count when the page was built
(§269 fork 7). A freed slot is reused with a rebuilt page.

The `std` contract (`crates/std_runtime_contract`) fixes three slots after the resolver at 9:
`PROCESS_SLOT` 10 (the program's process, `BIND`), `THREAD_SLOT` 11 (its first thread, `WRITE`, for
`SET_THREAD_POINTER` on itself), and `SPACE_SLOT` 12 (its space, `READ`, for futexes). Without slot
10 a spawn answers `Unsupported`; without 12 the locks yield instead of sleeping.

## What this changes in other sections

- §249 (a running address space stays nameable, and a capability never decides when it dies) is
  amended, in its own file. Amendment (b)'s refusal now applies per holder, not per thread. A
  member's space dies when its process has ended and its last member is reaped.
- §269's open consequence is answered by ruling 1 above.
- `SYS_EXIT`'s doc comment in `crates/abi` said "terminate the calling thread", which stopped being
  true at `ddd38f0b0`; corrected beside this section.
