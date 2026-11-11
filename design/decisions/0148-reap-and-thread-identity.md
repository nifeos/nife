---
status: AMENDED
raised: 2026-09-05
decided: 2026-09-05
ratified_by: calef
---

# 148. Milestone 105's two forks: a supervisor restarts by asking, and resolves by asking the kernel

calef, 2026-09-05, on milestone 105 (the two forks named and left)'s two forks, taken one at a time.

Milestone 22 recorded both in `notes/trusted-init.md` as *"calef's call, not a thing to slip in"*,
and milestone 105 stated them precisely and picked neither on purpose. This is the ruling.

## Fork one: no reap-only right. Tier one adopts tier two's pattern

Decided: neither a new rights bit nor an `Untyped::REAP` method. A tier-one server that should
be restartable gets a spawner, and the root asks it, which is what tier two already does.

### Why, and the check that moved it

The fork was put as *"are reclamation and construction separable rights"*, and the answer is that the
tree does not need them to be, because it already restarts a child without construction
authority. `components/src/sub_server_supervisor.rs`:

> restart policy, in userspace, holding nothing... cannot make an endpoint, cannot allocate a
> page. Its entire power is to ask the spawner for a rebuild of the one program the spawner can
> build. A compromised supervisor is a restart loop, not a foothold.

And the pattern generalizes, which was checked rather than assumed, because it was the one
objection that would have sunk it. `components/src/spawner.rs` holds one untyped budget (`WRITE` only, so
it may spend but never lend), a request channel, and one program image copied in by
`root_supervisor`: *"the only program it can name is the one it was handed."* The image is handed
in by the root, so the root already has the machinery to do this for a tier-one server. The memory
model already survives restart loops: each instance is built in its own region split off the budget,
and a LIFO reap returns the pages, *"so a restart loop is not a leak."*

The authority is bounded more tightly than a right would bound it. A bit says *may reap*. A
one-program spawner says *may only ever produce this*.

### The prior art, read rather than recalled, because the two relatives disagree

Zircon has 25 rights and splits both of these: `ZX_RIGHT_ENUMERATE` (*"Allows enumerating child
objects"*) and `ZX_RIGHT_DESTROY` (*"Allows termination of task objects via `zx_task_kill()`"*), read
2026-09-05 at `fuchsia.dev/fuchsia-src/concepts/kernel/rights`. The page states no design principle
for the granularity, so it is an existence proof rather than an argument, and 25 is where the
add-a-bit path ends.

seL4 has four (Read, Write, Grant, GrantReply) and solves this exact problem structurally, with no
right at all. Its manual: *"The revoke method removes all capabilities (in all CSpaces) that were
derived from a selected capability... [used] by managers of untyped memory to destroy the objects in
that memory so it can be retyped."* Authority comes from having derived the capability.

seL4's answer is not available here and that is on purpose. §13 (capability revocation and untyped reclamation) declined the capability-derivation tree as *"a
considered terminal design, not a way-station"*. It revokes all derivatives rather than a subtree,
because *"the full tree buys subtree granularity,
which nothing on the roadmap needs."* Taking seL4's route would mean reopening §13, which is larger
than a rights bit and was not what the fork asked.

### What was refused, and the reason each lost

- A `REAP` rights bit. [`ENUMERATE`](../../crates/capability/src/lib.rs)'s own precedent is real
  (milestone 126 split it out of `READ`, and its doc comment records that `READ` wrongly authorized
  `REAP` on a rendezvous), and there are 28 unused bits, so the structural cost is nil. It lost on a
  check: `REAP` alone buys cleanup, not restart. Rebuilding needs `RETYPE`, which needs `WRITE`,
  so a reap-only root turns *"report and stop"* into *"reap and stop"* and does not gain the
  supervision property the milestone was reaching for.
- An `Untyped::REAP` method needing less than `WRITE`. Same defect, and it makes the authority
  invisible to anything that inspects capabilities, including `caps`.
- Closing the fork with the floor as correct. `root_supervisor`'s own comment says *"a root that
  can restart is a root that can build, and the fail-closed floor is the more valuable of the two"*,
  which is true and is now beside the point: the root does not have to be the builder.

### The constraint this ruling carries, and it is not optional

A supervision endpoint and a spawner land together, per server, or neither does.
`notes/trusted-init.md` is explicit that half of it is worse than none:

> The boot servers are not supervised. Endowing them a supervision endpoint with nobody to restart
> them would make their corpses persist forever instead of being reaped by the kernel, which is
> strictly worse.

The cost accepted. One spawner per restartable tier-one server, each holding `WRITE` on its own
budget, so total construction authority is spread across more processes rather than concentrated in
one root. That is a real trade and calef took it knowingly: each spawner is about as narrow as a
builder can be, and the count is bounded by how many tier-one servers are worth restarting.

## Fork two: `ThreadControlBlock::RESOLVE`, and not deferred

Decided: the kernel owes a supervisor the ability to resolve the identity it already sends. Not
deferred, and not left to a userspace protocol between builder and supervisor.

*Amended 2026-10-04 (calef): the owed resolution holds, and the `RESOLVE` method does not. The
supervisor learns which thread died from the death message itself; see "Amended 2026-10-04" below.
The ruling above is kept as it was made.*

### Why not deferred, which was the live alternative

The multi-child supervisor already exists. `components/src/root_supervisor.rs` builds two children
with `fault: Some(rootfault)` on the same endpoint and then sits in `recv(rootfault)` receiving
`(event, tid, _pc)`. Milestone 105 says the problem *"does not generalize"* to a supervisor with
several children; it does not generalize to the supervisor the tree already ships.

And deferral would have been a decision for the workaround, made by inaction. The workaround
already exists: milestone 105 records `sub_server_supervisor` naming instances *"by a handle the
spawner issues instead"*. Milestone 23 would have baked that in.

This tree has that failure on record from the same day. Milestone 106 (a wait that ends on either the interrupt or the deadline)'s fork stood from
2026-08-04. In the meantime `net_stack` shipped `wait_for_nic` yielding and re-polling, which
burns a core through every retransmit backoff and is now the thing in production. A fork routed
around rather than settled gets expensive, and both 106's block and milestone 51's warn in those
words against settling one by accident.

The usual reason to be careful with syscall surface is weak here. Milestone 105's own argument:
the method *"discloses nothing new: the tid is already in the fault message, so turning it into a
handle reveals no fact the supervisor did not receive."*

### The name, and what it is not

`ThreadControlBlock::RESOLVE`, in `abi::thread_control_block`. `Tcb` became
`ThreadControlBlock` under [§113](0113-kernel-object-plain-names.md) with the reason *"acronym spelled out"*, which
is the rule calef set on the same day this was decided, applied before it was written down.

`resolve` is the tree's own verb for this relation rather than a coinage: `abi` says *"a stale
tid whose generational name no longer resolves"*, and `capability` says *"`None` if the thread
does not resolve at all, or resolves and is unsupervised."*

- `NAME` refused, which was milestone 105's provisional. It is ambiguous between *give this
  thread a name* and *tell me this thread's name*, and calef caught it.
- `BADGE` refused for a collision: seL4 uses it for capability badges and
  [§101](0101-notification-objects.md) already names badged capabilities as a later fork here.
- `LABEL` refused for now, because it presumes the design below.
- `IDENTIFY` refused as a synonym that spends vocabulary the tree already has.

## What a lane must not decide by accident

Whether `RESOLVE` returns a capability or an identifier. Milestone 105 says only *"something a
builder holds"*. A capability is actionable and would be a larger power than the
discloses-nothing-new argument covers; a label the builder set at spawn discloses nothing.
Milestone 126's lesson applies (*enumeration is itself authority*), and the reading should be
argued rather than defaulted to.

`RESOLVE` inherits `REAP`'s anti-probe design and must not undo it. `abi`'s `REAP` collapses
every failure into one error on purpose:

> The two are deliberately one error, so a supervisor cannot probe the tid space of children it
> does not supervise.

A `RESOLVE` returning distinct errors for "not yours", "already collected" and "stale" would quietly
retire that. It is `REAP`'s query sibling: same tid, same supervision check, same single refusal.

## Amended 2026-10-04: the death message names the thread, and `RESOLVE` is not built

calef, 2026-10-04 (UTC), on the sub-question above: *"R3, unless the IPC benchmarks move."*

### Decided: R3

The supervisor learns which thread died from the death message, with no separate
call. Today the kernel drops the fault endpoint capability's badge at spawn (`kernel/src/sched.rs`,
`Object::Rendezvous(ep, _)` where the reserved fault slot is consumed) and sends
`[event, current, pc, addr, 0]` (`depart`). It will keep that badge and deliver it, the label the
builder set, with the death message. This amends fork two's naming of a
`ThreadControlBlock::RESOLVE` method: the obligation stands, met by the message instead of a method.
It also keeps `REAP`'s anti-probe property for free, because a supervisor receives a label only for
a child it supervises and there is no query to probe with.

### The condition

If delivering the label moves the IPC benchmarks, meaning `ipc_send_receive` or
`ipc_call_reply` lands outside its band in `script/bench`'s symmetric gate, the fallback is R2. Whoever
builds R3 measures before landing.

### Refused

- R1, `RESOLVE` returns a capability. It grants a power, and the "discloses nothing new" argument
  that justified fork two covers disclosure only. Milestone 126 (the `procps` package)'s *enumeration is itself authority*
  is the same objection.
- R2, `RESOLVE` returns a word the builder set at spawn. Not refused outright: it is the fallback
  if R3 moves the benchmarks. It costs a syscall method and a round trip per death that R3 does not.

### Where the label travels, read from the tree 2026-10-04

The unverified lead was that a badged receive already delivers the badge in its own register,
making a sixth word unnecessary. It does, in word 3: `wide` in `kernel/src/sched.rs` puts the badge
there, and `RECEIVE` returns it in `x3`/`a3`/`r10`. But the death message uses word 3 for the
faulting address. Word 4 is reserved for the resume protocol in section 4 of §26 (the fault
endpoint). So the lead does not by itself free a word.

A sixth register is free on all three ISAs. `INVOKE` reads only argument registers 0 to 4
(`kernel/src/syscall.rs`, the `SYSCALL_INVOKE` arm, its name spelled out by the ruling of 2026-11-11
(UTC)). Argument register 5 (`x5`, `a5`, `r9`) is in every
trap frame and restored on return, including on x86_64, whose `syscall` entry exits through
`isr_restore`. Using it costs two things. The user-side stubs (`invoke5` in
`crates/user_mode_runtime`) must declare it as an output on every ISA, or a `RECEIVE` silently
clobbers a register the compiler thinks is preserved. And if the mailbox widens from five words to
six (`Thread::mailbox`, `fault_msg`, `wide`), every IPC delivery stores one more word, which is
what the benchmark condition watches. Carrying the label beside the mailbox on the death path
alone avoids the second cost; that is the builder's call.

### Where the label travels, as built 2026-10-05

Built by milestone 105 (the two forks)'s lane, 2026-10-05 (UTC), on all three ISAs. The builder
labels a child by minting a badged copy of its supervision endpoint with `rendezvous::BADGE`,
inserting that copy in the reserved fault slot, and deleting its own. `START` keeps the badge as
`Thread::fault_label` while it consumes the slot, so the child never holds a capability carrying it.

The label travels beside the mailbox, not in it. The kernel writes it into argument register 5
(`x5`, `a5`, `r9`) of the supervisor's saved user frame, and only when it delivers a death to a
plain `RECEIVE`. Both routes do it: at the rendezvous, when a supervisor is already waiting
(`deliver_death`), and when a supervisor later collects a parked corpse
(`collected_without_serving`). `Thread::mailbox` and `fault_msg` stay five words. No other path
writes the register, so a receiver zeroes it on entry, as `user_mode_runtime::receive_fault` does,
and label 0 means the capability was unbadged or the message was not a kernel-stamped death. A
sender's badge still arrives in word 3, so a child can neither learn nor forge its label.
`RECEIVE_CAP` and a kernel thread's in-kernel receive do not get it.

The benchmark condition held, so R3 stands. Measured against the base commit and again against
main after merging: `script/fastpath-footprint`'s `ipc_send_receive` and `ipc_call_reply` were
byte-identical on all three ISAs. In `script/bench`, every icount move was under 2% and inside its
band, and none was on the kernel IPC path. Milestone 105's block has the numbers.

## BUGS

- Fork one is a ruling about a pattern, not a built thing. No tier-one server has a spawner
  today, and the note's warning means the first one to get a supervision endpoint must get a spawner
  in the same change.
- Fork two's method is named and unspecified. What it returns is the design question above, and
  this section deliberately does not answer it. *Amended 2026-10-04: answered by R3, no method; see
  the amendment.* Not built.
- The Zircon and seL4 readings are one page each, taken 2026-09-05. Neither project's full
  rights model was studied, and the Zircon page's silence on rationale is reported rather than
  interpreted.
