---
status: AMENDED
raised: 2026-09-04
decided: 2026-09-04
ratified_by: calef
---

# 142. What a spawner retains over a child after `START`

Retention is a declared field on `ChildEndowment`, and the kernel does not change (answer R4). What
a spawn path declares by default is that the spawner keeps nothing over a running child (answer R0)
beyond the region it built the child from and the supervision endpoint the child was born under. A
requester may instead hold the region a builder builds its job from, and then the requester, not
the builder, supervises the job and tears it down. Milestone 133's proposal A ships alongside the
field rather than after it.

Every future spawn path is written against this convention, so it sits in the *move fast on what
can be undone; be methodical on what cannot* tenet's irreversible category.

The evidence is in [`0142-spawn-retention/`](0142-spawn-retention/README.md); this page is
complete without it:

- [`thread-capability.md`](0142-spawn-retention/thread-capability.md): the premise check, and
  R2's findings.
- [`precedent.md`](0142-spawn-retention/precedent.md): recorded precedent, and reversibility.
- [`prior-art.md`](0142-spawn-retention/prior-art.md): the prior art, quoted.
- [`sequencing.md`](0142-spawn-retention/sequencing.md): the order against milestone 133 (ending a
  permanently blocked thread), and the readings of the mapping.

## What is being decided

**Does a spawner retain a lifetime handle on its child, and if so, is that handle the supervision
endpoint, the thread capability, or something that does not exist yet?**

`crates/supervision_contract`'s `ChildEndowment` declares what a child *gets*, and it is built to be
read as the whole story: its own doc says "read one of these and you know the complete authority
of the thing about to run". A literal ends `..ChildEndowment::new(..)`, so what is not listed is not
granted. It carries `capabilities`, `placed`, `maps`, `blobs`, `stack_pages`, and
`fault: Option<u64>`, the supervision endpoint of §26 (the fault endpoint).

Retention is the reciprocal half: what the parent keeps. Without it, each call site would decide
that by whether it happens to call `cap_delete` after `START`.

## The rule

`ChildEndowment::retention` takes one of three values, and `ChildEndowment::new` takes it as an
argument, so no default hides the question. That is rung one of AGENTS.md's ladder for the specific
failure that a spawn path forgets to decide.

- `Retention::Nothing`. `process_builder::start_child` deletes the `ThreadControlBlock` capability
  at `START`. The spawner's authority over the running child is then `MemoryRegion::DESTROY` on the
  region the child was built from, plus `REAP` on the supervision endpoint once the child is a
  corpse. Ending a child means ending its region, which is §40 (a supervisor's death is its
  subtree's death)'s ownership argument taken as far as it goes.
- `Retention::ThreadControlBlock { reason }`. The spawner keeps the capability past `START`. Kept,
  it authorizes only `SUSPEND` and `RESUME` (§300 (the paused state, a TCB verb, silent)). The
  reason is part of the variant, so a spawn path cannot keep the capability without saying why.
- `Retention::Requester { reason }`. The builder hands the capability to whoever asked for the
  child and keeps no copy (§309 (a supervised job's `ThreadControlBlock` goes to the shell that
  asked for it)).

The declaration says nothing about the region (§303 (the retention declaration says nothing about
the region)). Who holds the region decides who may end the child.

### A requester may hold a job's region

A requester may hold the region the builder builds its job from, and tell the builder to use it.
Milestone 933 (every job is built the way a supervised job is) builds this for the shell. Then:

- The requester supervises the job on an endpoint it holds, reaps it, and tears it down with
  `DESTROY` on the region. The builder retains nothing over such a job.
- The builder learns that the job's file-service window is free from a message the requester sends,
  not from the undertaker's.
- The requester's memory budget grows by what the builder's pool shrinks.

The wire:

- `Wiring::held` is a request bit (`HELD_BIT`). Three capabilities follow the interrupt pair: the
  region (every right), the requester's endpoint (`READ`, `WRITE`, `GRANT` and `ENUMERATE`), and,
  for a program that declares a share view, the requester's budget narrowed to `ENUMERATE`.
- The builder badges the endpoint with a label it mints, places it as the job's supervision
  capability, and answers once with `(SPAWN_OK, label, 0)`. Retention is `Requester`, so the job's
  `ThreadControlBlock` follows as the one message after a good answer.
- A foreground job's output goes through a second copy of the labeled endpoint. A background job's
  goes to the terminal's sink (`SCREEN_BIT` with `HELD_BIT`).
- The kernel stamps the label on the sixth word of the job's death message. The requester tells a
  death from the job's own output by that word.
- The requester sends `release(label)` after its `DESTROY` has returned the region. The builder
  frees the job's window, resolver grants and installer reply, as it does for the undertaker's
  message, and answers nothing.

The builder refuses a held request whose program it takes from the image pool (an installer
holder, or a file run by its path). The pool and the image pool do not shrink until the shell holds
those jobs as well. Milestone 933 states the numbers and what remains.

### Why the field releases milestone 133 instead of deferring it

Proposal A needs no new authority, and that is most of its appeal. Taken alone, it would settle
retention *by accident*, in R0's direction, after which every spawn path grows around it and R1 or
R2 get harder. A declared field removes the accident: a later change is a change to what the field
may say, not to thirty call sites and an unwritten habit. So the two ship together:

- Proposal A makes `MemoryRegion::DESTROY` finish what it starts, so a hung component's region
  becomes reclaimable. That is the capacity problem this whole area exists for.
- Milestone 254 (a caller stranded by a server that died is stranded forever) frees the stranded
  callers: they wake with `abi::Error::Gone`, so a server that merely died does not strand them.
- Together they close the compounding: one hang stops costing one region per caller in flight, and
  stops costing the one.

Cost: zero capability-table slots, no new syscall, no ABI change, and nothing overturned. Not §32
(a supervisor may collect a corpse)'s line at the corpse, not milestone 126's rule that a domain
names its members and does not act on them, not §26.2's deferral of reattaching supervision. Both A
and 254 need the reply-capability sweep, and it is the same sweep.

## Prior art

| System | What the spawner retains | Granularity | Ends a running child? |
|---|---|---|---|
| seL4 | whatever caps it did not delete, the TCB cap among them | thread | yes, `Suspend` |
| Genode | the child's PD-session capability, for life | component | yes, destruct |
| Zircon | a process or job handle with `ZX_RIGHT_DESTROY` | process/job, **never thread** | yes, and refuses at the thread |
| L4Re | the thread/task capability, plus the delete right | thread or task | yes, `ex_regs` or delete |
| QNX | nothing; the right is ambient and the name is a pid | process | yes, by pid |
| **nife** | the region, the supervision endpoint, and a TCB cap only where `Retention` declares one | region, or corpse | **no**; a kept TCB cap suspends and resumes |

L4Re's thread capability is a lifetime handle in the fullest sense: it permits
`l4_thread_ex_regs()`, which exchanges the instruction and stack pointers and cancels ongoing IPC
on a *running* thread ([Thread, L4Re](https://l4re.org/doc/group__l4__thread__api.html), fetched
2026-09-03). QNX is the counterexample: the authority to signal is a property of the *caller*, and
the target is a global pid, the model §82 (ambient authority is the problem) refuses. Everybody
else's answer is a held object, and the only disagreement is which object. nife is not unusual in
refusing a kill verb; Zircon does too. It is unusual in having no per-child object that outlives
construction, and the region capability stands in for one.

## The five answers, and what each costs

Ordered by how much new machinery they need, not by preference. Two measured facts apply to all of
them.

Retention costs a capability-table slot per retained child, and the budget is nearly spent.
`CAPABILITY_TABLE_SLOTS` is 24. `CAPABILITY_TABLE_PEAK_MEASURED` is 21, in init, during
`build_child` for `credentialer`, instrumented over four boots by milestone 230 and kept honest by
`report_peak` and `script/swish-check` since milestone 231 (nothing counts how many capability slots
a boot actually uses). Three slots of headroom. `crates/system_initializer`'s `boot` starts twelve
children, all long-lived. Any answer that gives the spawner one more permanently held capability per
child asks init for twelve slots against three, so it is not an increment. It forces a raise of
`CAPABILITY_TABLE_SLOTS`, which that constant's own doc prices at 32 bytes a slot across
`MAX_THREADS` and whose history is five days of a silently halting boot. An answer whose handle is
one per supervisor rather than one per child costs zero slots, which is precisely the shape §26.5
already chose for the supervision endpoint and for a different reason.

**A new `Object` variant is cheap and is not the expensive part.** `Object` has nine variants and
`const _: () = assert!(core::mem::size_of::<Object>() == 24)` holds; a variant carrying a `ThreadId`
adds no size, since `Reply` and `ThreadControlBlock` already carry one. Sixteen `Object::` sites in
`kernel/src/syscall.rs`, fifteen in `cap.rs`, thirteen in `sched.rs`. The cost of a new capability is
the *slot* it occupies in the holder's table and the meaning it acquires, not the enum.

### R0. Nothing retained: the default declaration

Cost: zero. No code, no ABI, no slot; 25 of 30 `START` sites already deleted the capability when
counted (`thread-capability.md`). It forecloses ending one thread without ending its region: a
supervisor that stops a misbehaving child takes its memory with it, and a child sharing a region
with a sibling cannot be singled out. It is exposed to §40's own recorded caveat: ownership is the
whole answer only while every child is built from its supervisor's own region, and nothing enforces
that yet.

### R1. The supervision endpoint is the handle: the likely successor, not taken

The retained thing would be the `Rendezvous` capability the spawner already passes as
`ChildEndowment::fault`. §32's `REAP` already authorizes an action on a thread off that endpoint by
the kernel matching `fault_ep`, and one endpoint per supervisor costs no slots (§26.5). It fits this
system's own model: the supervisor already receives the fault, reaps the corpse and rebuilds the
child, and *notice hang, end, rebuild* is the same job. Its price is overturning three recorded
decisions: §32 draws its line at the corpse, milestone 126 rules that a domain names its members and
does not act on them, and §26.2 deferred even reattaching supervision. It also concentrates
authority over every child that ever named the endpoint behind a tid the kernel stamped. That
should be bought with a customer, so it is not bought here.

### R2. The thread capability becomes a lifetime handle: refused

seL4's and L4Re's answer. Refused on model grounds as well as cost (`thread-capability.md`, R2's
findings). seL4's lifetime handle earns its keep only alongside a repair-and-resume recovery model,
which §26 declined. The widening would also change the offer under every existing holder from "you
may assemble this thread" to "you may end it, forever". Twenty-five sites would keep a capability,
twelve of them in init against three spare slots. A rights bit could separate construction from
lifetime, but `Rights::ALL` is `0b1111`, so a fifth bit is an ABI change to `abi::rights`.

### R3. A new capability, minted at `START`: refused

Construction and lifetime would become two objects. Refused: the handle is *additional*, so it
costs init twelve slots against three spare, and `START` has no spare argument register (all three
carry the child's `x0`, `x1`, `x2`) to let a caller decline it. It adds a third noun where a
newcomer already finds the area hardest. §139 (who may read the cycle counter) declined a method
whose successor was already visible, and milestone 147 (a profiler) wants cross-thread authority
whose shape nobody has priced.

### R4. Retention becomes a declared field: taken

A field and one branch, no kernel change, no ABI, no slots beyond what a declaration asks for. It
cannot grant an authority the kernel does not offer, so on its own it declares R0 and makes it
checkable. **It is not an alternative to R1, R2 or R3; it is the shape any of them would want
anyway.** Milestone 244 (the largest crate in the tree is proved by nothing a mutation can reach) saw
the same subject from the other side. Its proposed follow-up wanted `boot`'s wiring to be a table a
host test can assert, since thirty-four of `boot`'s mutants deleted a field from a `ChildEndowment`
literal with nothing noticing.

## The mapping: what each answer does to milestone 133

Milestone 133's proposals, as `notes/blocked-thread-teardown.md` states them: A finishes what
`DESTROY` starts, on the region capability. B puts a terminate verb on the thread capability. C
frees the stranded caller, and its first piece is milestone 254, out of scope here. D accepts the
leak, bounds it, and records it.

| | A: completed reclaim | B: terminate verb | D: accept and bound |
|---|---|---|---|
| **R0** nothing retained | **enables**, and is A's own premise | **forecloses in practice**: B's authority is a capability nobody holds after `START` | **enables** |
| **R1** supervision endpoint | enables, unchanged | **replaces** B with a variant on a different object, and owes §32, §40 and milestone 126 an answer | enables |
| **R2** thread capability | enables, unchanged | **enables**, and is the convention B's own failure analysis says it is missing | enables |
| **R3** new capability | enables, unchanged | enables, on the new object rather than on `ThreadControlBlock` | enables |
| **R4** declared retention | enables | **neutral**: declares whichever of the above is chosen | enables, and is the honest way to record it |

## What is not decided

A supervisor still cannot end one thread without taking its region. That is proposal B's job, and
B stays open behind a customer that does not exist yet: a supervisor that must stop one child while
a sibling shares the region, or a policy separating stopping from freeing. R1 is the answer that
customer would buy.
