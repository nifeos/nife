# The method contracts: why each `crates/abi` method reads as it does

`crates/abi` states each syscall and method as a contract: its arguments, what it returns and what
it refuses. The rulings behind them are in `design/decisions/`, and each doc comment cites its
section. This page carries the reasoning, the prior art and the standing measurements that the doc
comments link to rather than repeat. [`notes/abi.md`](../abi.md) carries the convention itself.

## The five calls

`exit`, `exit_thread`, `yield` and `cap_delete` are bare syscalls because a capability is authority
over something else. Nobody needs a grant to stop running or to forget one of their own slots. Every
other operation is a method on a capability reached through `invoke`, so a new operation is a new
method number and a new syscall is a design fork.

`exit_thread(word)` is calef's ruling on pull request #1892, question 3, and §276 (the thread
syscalls as built) records it. The cleared word is Linux's `CLONE_CHILD_CLEARTID`. The prior art for
a thread exit beside a process exit is Linux `exit` beside `exit_group`, Zircon `zx_thread_exit`
beside `zx_process_exit`, and FreeBSD `thr_exit`, whose `state` word is this one.

## Errors

`Gone` and `NoSuchSlot` are the answer of milestone 50 (pipes and redirection) to `SIGPIPE`. A program never granted a stdout
keeps running and prints into the void, which is what every system does to a process whose stdout
is closed. A program whose reader has exited must end. One error for both would allow only one of
those behaviors.

`Error::from_ret` decodes only the values the kernel assigns. An unassigned negative is not decoded
as an error, which is what lets a wire contract layered on top, such as `entropy_contract` or
`credential_contract`, tell "no capability" from "no".

## Rendezvous

A plain `RECEIVE` puts the badge of the sender's capability in `x3`. That is the amendment milestone 613 (a system log service) made to §230 (badged endpoint capabilities), which first delivered the badge on `RECEIVE_CAP` alone. The system log stamps a byte-sink writer from its badge, and a byte-sink writer sends with `SEND`. A death message is still told apart by its first word, and nothing reads `x3` before
checking that word.

Under §246 (a plain `RECEIVE` never takes a capability) a `SEND_CAP` reaching a plain `RECEIVE`
delivers its data word in `x0`, with `0` in `x1` and `x2`, and the sender keeps its own copy of the
capability. A `CALL` delivers its two words in `x0` and `x1`, and the caller gets `Gone`, because
the one-shot Reply has nowhere to go.

### BUGS

- On `RECEIVE`, `x0` is the sender's own `w0`. A negative `w0` equal to an `Error` code is decoded
  as that error by any wrapper that checks the sign, and the receiver cannot tell it from a real
  refusal. The same fact is why the notification tag lives in `x4`. The fix is a register
  convention for `RECEIVE`'s status, and every receiver in the tree is written against the current
  one.
- A §26 (the fault endpoint) death message read with `RECEIVE_CAP` loses the faulting `pc`. The kernel's five words are
  `(event, tid, pc, addr, 0)`, and `RECEIVE_CAP` returns `(event, NO_CAP, tid, addr, 0)`. Every
  supervisor in the tree receives deaths with `RECEIVE`, through `user_mode_runtime::receive_fault`.
- A delegated capability passes the `NO_CAP` guard. Any client with `WRITE` on the endpoint and
  `GRANT` on a capability can `SEND_CAP` it to a `CALL` server. A server that invoked `REPLY` on it
  would run method `0` of whatever object it is. `REPLY_DELIVERED` in `x4` (§245 (a `CALL` server tells a Reply from a delegation)) is the test, and
  `user_mode_runtime::receive_request` reads it and deletes an unasked delegation. A server that
  reads `x1` raw is still exposed. An endpoint that refuses `SEND_CAP` outright is the recorded follow-on of milestone
  706 (a `CALL` server can tell a Reply from a delegation).

### `REAP`

Authorization is the supervision relationship, not a rights bit and not a registry. The kernel
checks that the named thread's recorded fault endpoint is the endpoint invoked, so a tid means
something only relative to the capability it arrived through. A supervisor needs no capability to
the child's memory and gains none. The reclaimed region returns to its owner under §13 (capability revocation and untyped reclamation), which is the
builder, so a supervisor can free a child's memory but cannot spend it. Killing a live child is the
stronger act and stays with `MemoryRegion::DESTROY`, the forcible `^C` of §24 (interrupting the
foreground process).

`NotSupervised` covers another supervisor's child, one already collected and a stale tid. They are
one error so that a supervisor cannot probe the tid space of children it does not supervise.

BUGS: `NotPermitted` has a second cause. The reclaim behind `REAP` also refuses while another thread
whose page lies in the corpse's region can still run, and that refusal arms the kill of §16 (object revocation) on it,
as a refused `DESTROY` does. A supervisor that treats the first `NotPermitted` as fatal, as
`terminal_supervisor` and `sub_server_supervisor` do, cannot tell the two causes apart. No test
builds a corpse that shares its region with a live thread, so whether a supervisor can meet this is
unmeasured. The corpse itself still leaving its core is waited out by the kernel and never reaches
the caller.

### `SURVEY`

`record` is a selector (calef, 2026-09-21), as Zircon's `zx_object_get_info` is a selector over
topics. `x0` and `x1` are the frame and only `x2` belongs to the record, so a caller wanting two
facts walks twice and joins on the tid. `record::STATE` is 0 so that callers which passed an unused
0 keep reading the run state.

The scope is the supervision domain. A `ps` launched from a shell sees the shell's children, and an
operator's `ps` sees the machine only because somebody handed it the endpoint that supervises the
machine. Each call is a snapshot of one entry, which is `readdir`'s bargain. Holding the IPC tables
across a whole survey would put a userspace program in charge of how long the scheduler is locked.

`BADGE`'s number was kept free by §148 (a supervisor restarts by asking).

### The survey records

`PLACEMENT` gives userspace the fact `sched::spawn_reporting_placement` gives the in-kernel job-mix
supervisor. It is the placement rather than the current core on purpose. Tracking the current core
means a store in the context switch, the hottest line of the hottest function. The kernel's
`Thread::last_cpu` does exactly that, and it sits behind a soak-build feature gate because it cost
5.7% of `ipc_fastpath`'s footprint on aarch64, over the 5% bound of milestone 132 (the fast path's
footprint, and a gate).

The id is a name and not an index. On the VisionFive 2 slot 0 is an M-mode monitor core with no MMU,
so the online set is `{1, 2, 3}`. A census keyed by the ids it observes needs no online mask, since
those ids are a subset of the online set by construction. A reader that needs the online set itself
has no path to it.

`CPU_TIME` is scheduled on-CPU time. §150 (how does a thread's CPU time reach userspace?) refused two
cheaper answers. Wall-clock age reads the same for a thread that slept five minutes and one that ran
for five, and userspace sampling over `SURVEY` misses every thread that runs between two samples.
Sampling at the tick is §150's sub-choice 1, the `jiffies` bargain Linux also takes.

It tells a holder of `ENUMERATE` more than a run state does. A run state is one of five values at
an instant, while this is a continuous, monotonic signal, and two reads measure the work a thread did
between them. A confined viewer holding a supervision endpoint can watch the shape of another
thread's workload without holding anything that acts on it. §150 accepted that, because `ENUMERATE`
is already the right to learn what exists, and the leak is bounded by the domain the caller was
endowed. §204 (how userspace asks where a thread runs) records that `PLACEMENT` reveals strictly
less.

## Reply

`REPLY_CAPABILITY` has no word for a rights mask, so a server narrows before it answers. The network
stack mints each socket from a `WRITE | GRANT` copy of its own endpoint, and the copy it answers with
has those rights. A refused source consumes nothing, and the Reply can still answer with `REPLY`.

The caller does not opt in. It asked for a reply by calling, and it can delete what it is given; seL4
gates reply transfer on the server's `Grant` alone. §246 refused filling the table of a receiver that
never asked, and a `CALL` caller is not one. Whether a caller should be able to refuse is left open
in §255 (each socket is its own capability).

## Address spaces

`LIST` answers one entry per call because the space's mapping log is held only long enough to read
one row, never for the whole walk. It reuses `survey::DONE`, because "no more entries" means the same
on both objects, and it reuses `MAP_RO`, `MAP_RW` and `MAP_CODE` for `kind`, because read back they
are the same fact about the same page. The listing does not tell device memory from ordinary data.

A cursor the space's own log never minted answers `BadPointer` rather than `DONE`, under the 2026-10-06 addendum to §114 (`ENUMERATE` extends to the
address-space object). A caller that corrupts its own cursor learns that it is broken rather than
seeing a truncated listing.

`LIST` needs `ENUMERATE` and not `WRITE`, the split `SURVEY` drew between looking and acting.
Every address-space capability minted since 2026-08-17 already carries the bit, because a capability
is created with `Rights::ALL`, so this method is what makes that bit live.

`UNMAP` is not idempotent on purpose. The caller is closing a window, and "it was never open" is a
different fact from "it is now shut". A frame capability the caller still holds survives and can map
the page again, and a later revoke of that frame no longer reaches the address. calef ruled both on
2026-10-05 (UTC), as the amendment of that date to §162 (whether a holder can give up a mapping); `notes/unmap.md` has the argument. On a running
space the thread there faults on its next touch of the page.

## Threads and processes

`CONFIGURE` consumes the address-space capability it is given but not the space's name (§249 (a running address space stays nameable)). A
copy made first goes on naming the space while the thread runs, which is how a builder that keeps one,
or the progenitor with its own, can `UNMAP` a page of a running space. The space dies when its thread
is reaped or its root's region is destroyed, and every capability still naming it then fails;
deleting a capability never frees it. A space that already has a holder, a thread that is its own
process or a process, is refused. Threads share a space by joining its process.

The sixth argument register of `CONFIGURE` is the thread's first thread pointer (§269 (how threads share a process) fork 4,
Linux's `CLONE_SETTLS`): `TPIDR_EL0`, `tp` or the `FS` base, kept per thread. `CONFIGURE` is the
only method that reads that register, and `user_mode_runtime::invoke6` zeroes it, so a caller that
sends nothing sends "none". calef ruled the encoding on pull request #1892, question 2. The result is
the page the kernel publishes the thread's core and allowance on (§269 fork 6). A thread configured
with a space reads `current_cpu_contract::PAGE_VA`, and one that joins a process reads its own slot
below it, `current_cpu_contract::thread_page_va`.

A thread blocked in IPC when it is suspended stays in that IPC. Its rendezvous may still complete,
and a `RESUME` lets it return what it was given.

A process's supervision slot is the process's. The first member's `START` takes the reserved
`FAULT_EP_SLOT` out of the shared table, and the process's death is reported there whichever thread
ends it; a member started later finds the slot empty. A process ends when a member calls `exit` or
faults, when its last running member calls `exit_thread`, when `Process::DESTROY` is invoked or when
its region is destroyed. Every other member ends with it, and no thread may join or start in it
afterwards. §276 has the whole contract.

## The fault message

`RECEIVE` returns a death's `w0` in the result register and `w1` to `w4` in the next four argument
registers. The label in argument register 5 is §148's ruling R3 as amended on 2026-10-04. The kernel
keeps the badge when `START` consumes the fault slot and writes it only on a death delivered to a
plain `RECEIVE`, so ordinary IPC stores no extra word. A receiver that wants `0` to mean "not a
kernel-stamped death" zeroes the register before the `RECEIVE`, as `user_mode_runtime::receive_fault`
does. The child never holds a capability carrying its label, and a sender's badge arrives in `w3`, so
a child can neither learn nor forge the label that names it.

## Notifications and timers

Two signals sent before anyone waits arrive as one wake carrying both sets of bits. A signal that
finds the bound thread anywhere but a receive is kept in the word and delivered the next time that
thread receives, waits or polls. A second bind of either the notification or the thread is
`NotPermitted`.

A timer has no wait of its own. §147 (a timer a userspace service cannot hold) put option 1 as a
timer object with a blocking wait, which has one wake source; this timer has none and borrows the
notification's. Arming a timer is signaling its notification later, so it takes `WRITE` on that
notification, the right `SIGNAL` needs. calef confirmed the surface on 2026-09-27 (UTC) with no slack
argument: *"Keep it confirmed. We will build a slack if we need it."* Zircon's `zx_timer_set` carries
one, and a tickless kernel would be the reason to add it.

## Memory regions and frames

`RETYPE_OBJ` makes one object per page, so the whole object family follows one memory rule; packing
is a later placement optimization. A `std` program passes `std_runtime_contract::SLOTS` as the floor,
so nothing it makes lands in a fixed slot it was not given.

`USAGE` is a selector of `SURVEY`'s shape, so a new figure is a new value and an arm in the kernel.
The frame and object records count the whole subtree because a budget's pages are mostly carved into
child regions, and the objects live in those.

BUGS: every subtree record walks the whole region table under the region lock, with interrupts
masked. The table (`kernel/src/memory_region.rs`, `MAX_REGIONS`, 256) is machine-wide, so one call
costs every region's parent chain. The 2026-10-03 security audit measured it on the host: 8 ns for a
root with eight children, 42 µs for a root over the deepest chain the table can hold, and 62 µs for
that chain's leaf. Any holder of `ENUMERATE` on a region can repeat it, so it is a bounded
interrupt-latency cost of the same shape as the reclaim scans the 2026-09-29 audit accepted. It is
accepted: nothing multi-domain splits that deep, and the bound is `MAX_REGIONS`, not the caller's.

`PageFrame::SLICE` is the method §102 (a Frame names a run of pages) anticipated with two capabilities,
one for the first 401 pages of a run and one for the 74 after them. A pool's holder slices window `w`,
maps it into the client, and deletes the slice. A slice never names pages outside its source's run
and carries the source's rights, which the holder narrows further when it delegates.

*Name: provisional. calef names files.*
