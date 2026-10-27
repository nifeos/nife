---
status: DECIDED
raised: 2026-09-04
decided: 2026-09-04
ratified_by: calef
---

# 142. What a spawner retains over a child after `START`

Answered 2026-09-03 by calef, after working the options through in
conversation: R4 declaring R0. Retention becomes a declared field on `ChildEndowment`, and what it
declares today is what the tree already does, which is retain nothing. The reasoning, including why
this unblocks milestone 133 rather than deferring it, is in "The decision" at the end.

No recommendation, deliberately. This is upstream of a syscall-surface question and it is itself
a convention every future spawn path is written against, so it sits in the *move fast on what can be
undone; be methodical on what cannot* tenet's irreversible category. Each answer below says what it
would make true and what it would cost. Picking is calef's.

The evidence is in [`0142-spawn-retention/`](0142-spawn-retention/README.md); this page is
complete without it:

- [`thread-capability.md`](0142-spawn-retention/thread-capability.md): the premise check, and
  R2's findings.
- [`precedent.md`](0142-spawn-retention/precedent.md): recorded precedent, and reversibility.
- [`prior-art.md`](0142-spawn-retention/prior-art.md): the prior art, quoted, and the survey table.
- [`sequencing.md`](0142-spawn-retention/sequencing.md): the order against milestone 133 (ending a
  permanently blocked thread), and the readings of the mapping.

## What is being decided

**Does a spawner retain a lifetime handle on its child, and if so, is that handle the supervision
endpoint, the thread capability, or something that does not exist yet?**

Half of the convention is already written and is written well. `crates/supervision_proto`'s
`ChildEndowment` declares what a child *gets*, and it is built to be read as the whole story: its own
doc says reading one "is meant to tell you the complete authority of the thing about to run", with
`..ChildEndowment::new()` meaning that what is not listed is not granted. It carries `caps`, `blobs`,
`maps`, `placed`, `stack_pages`, and `fault: Option<u64>`, the supervision endpoint of
§26 (the fault endpoint).

**The reciprocal half has never been written.** What the parent keeps is decided today one call site
at a time, by whether that call site happens to call `cap_delete` after `START`.

## Prior art, fetched rather than recalled

### L4Re: the retained thing is the capability, and destruction is deleting it

L4Re's thread capability is a lifetime handle in the fullest sense. The group description says a
capability to a thread permits `l4_thread_ex_regs()`, which exchanges the instruction and stack
pointers and cancels ongoing IPC on a *running* thread
([Thread, L4Re](https://l4re.org/doc/group__l4__thread__api.html), fetched 2026-09-03). Retention
and construction are one capability, as in seL4, and the delete right is what turns holding into
ending.

### QNX: the counterexample, and it is the ambient one

The authority is a property of the *caller*, not of anything the caller was handed, and the target
is named by a global pid. A parent retains nothing in particular; it is simply one of the processes
that may signal. This is the model §82 (ambient authority is the problem) and this whole tree exist
to refuse. It is included because it is the one that makes the shape of the others visible:
everybody else's answer is a held object, and the only disagreement is which object.

## The five answers, and what each costs

Ordered by how much new machinery they need, not by preference.

Two facts apply to all of them and are measured, so they are stated once here rather than five times.

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

---

### R0. Nothing. The region capability is the whole answer

What it makes true. The spawner's authority over a running child is exactly what it has today:
`MemoryRegion::DESTROY` on the region the child was built from, plus `REAP` on the supervision
endpoint once the child is a corpse. The TCB capability stays construction scrap and is deleted at
`START`. Ending a child means ending its region, which is §40's ownership argument taken as far as it
goes.

Cost: zero. No code, no ABI, no slot. It is the tree's current behavior in 25 of 30 sites; what
it costs is a paragraph, and the paragraph is the point, because right now the convention is
inferable only by grepping for `cap_delete`.

What it forecloses. Ending one thread without ending its region. The granularity of every
authority in the system stays the region, so a supervisor that wants to stop a misbehaving child
takes its memory with it, and a child sharing a region with a sibling cannot be singled out. It also
leaves milestone 105's fork two answered by default in option 3's direction: a tid is userspace
bookkeeping.

What it is exposed to. §40's own recorded caveat. Ownership is the whole answer only while every
child is built from its supervisor's own region, and nothing enforces that yet.

---

### R1. The supervision endpoint is the handle

What it makes true. The retained thing is the `Rendezvous` capability the spawner already holds
and already passes as `ChildEndowment::fault`. §32's `REAP` is already authorized off it by the
kernel matching `fault_ep`, so the mechanism for "this endpoint may act on this thread" exists and is
proven. A verb that acts on a *live* child would use the same check.

Cost. No new capability, and no slots at all: one endpoint per supervisor covers every child
(§26.5). A new method on the `Rendezvous` arm, and a `DECISIONS` section for its semantics under §10
(process model: capability-based, microkernel).

**What it costs that is not code.** Three recorded decisions push against it, and the note that
surveyed this said so first. §32 (a supervisor may collect a corpse) draws its line at the corpse in
as many words. Milestone 126 (the `procps` package) ruled that a domain names its members and does
not act on them. And §26.2 deferred even *reattaching* supervision as a new decision. It also
concentrates: the supervisor's one endpoint becomes authority over every child that ever named it.
The identity separating them is a tid the kernel stamped, which is fine for `REAP` on a corpse and
is a wider blast radius on a live thread.

The asymmetry worth noticing. This is the answer a reader expects, because the supervisor is the
party that notices a hang. It is also the one that overturns the most.

---

### R2. The thread capability becomes a lifetime handle

What it makes true. seL4's answer and L4Re's: the object you build a thread with is the object
you hold it with. `ThreadControlBlock` stops meaning "a thread under construction" and starts meaning
"this thread, for its life". It is delegable, narrowable, names exactly one thread, and does not
breach endpoint-only naming, because a capability handed over is a name handed over.

Cost, measured. Twenty-five call sites stop calling `cap_delete` after `START`, which is where
the slot arithmetic bites: twelve retained capabilities in init against three spare, so it forces
`CAPABILITY_TABLE_SLOTS` up. Then a method number in `abi::thread_control_block`, a scheduler
function whose embryo check differs from its three neighbors', and a §-section.

**What it costs that is not code.** The clean property measured above stops being true: a
capability's authority becomes a function of the state of the object it names. That is the
state-dependent rule AGENTS.md's ladder says to design out, and the note on blocked-thread teardown
independently flagged the same shape in the `killed` flag.

And the widening is a real offer changing under existing holders, even though the audit found few
of them. A builder that hands out a `ThreadControlBlock` capability today is offering "you may
assemble this thread". Afterwards it offers "you may end this thread whenever you like, forever". The
five `hello.rs` sites that keep theirs would acquire that power without anyone deciding it, which is
an argument for the retention convention being written down in the same change rather than after it.

The mitigation available. Rights. A capability narrowed at `CAP_INSERT` time already exists as
machinery, so "construction-only" and "construction plus lifetime" could be two rights on one object
rather than two objects. That reuses `Rights` instead of teaching a new noun, and it costs a bit;
`Rights::ALL` is `0b1111` today, so a fifth bit is an ABI change to `abi::rights`.

---

### R3. A new capability, minted at `START`

What it makes true. Construction and lifetime become two objects, so nothing about the existing
one changes meaning. `START` would return the slot the new handle landed in, which is expressible:
`CAP_INSERT` already returns a child slot the same way, and `START` returns 0 today.

Cost. One `Object` variant (nine to ten, `size_of` unchanged), a new arm in the dispatcher, and
the semantics section. Then the slot arithmetic again, and worse than R2's, because this handle is
*additional* rather than a re-use: twelve in init against three spare. The caller cannot decline it
without a flag. That means either `START` grows an argument (it has none spare; all three
registers are the child's `x0`, `x1`, `x2`) or the mint is unconditional and every existing caller
pays a slot it did not ask for.

**What it costs that is not code.** A third noun for a reader to learn in the area a newcomer
already finds hardest, and the one place this tree has explicitly warned against spending an
irreversible number. §139 (who may read the cycle counter) declined a method whose successor was
already visible on the roadmap. The same question applies here, since milestone 147 (a
profiler)'s profiler wants cross-thread authority with a named target and nobody has priced its
shape.

---

### R4. Retention becomes a declared field, and the kernel does not change

What it makes true. `ChildEndowment` grows its reciprocal, so one struct literal states both what
the child gets and what the parent keeps, and `build_child` acts on it (deleting the TCB capability
unless the endowment says to keep it). The convention stops being inferable by grepping for
`cap_delete` and becomes the thing a reader already reads.

Cost. A field and one branch in `crates/supervision_proto`, no kernel change, no ABI, no slots
beyond whatever the declaration actually asks for. It is rung one of the ladder for the specific
failure that a spawn path forgets to decide: the field has no default that hides the question.

**What it does not do.** It cannot grant an authority the kernel does not offer, so on its own it
declares R0 and makes it checkable. It is **not an alternative to R1, R2 or R3; it is the shape any
of them would want anyway**. It composes with 244's proposed follow-up (the largest crate in the
tree is proved by nothing a mutation can reach), which wants `boot`'s wiring to be a table a host
test can assert. That proposal is worth citing rather than absorbing: it is this subject from the
other side, and thirty-four of `boot`'s mutants delete a field from a `ChildEndowment` literal with
nothing noticing.

## The mapping: what each answer does to milestone 133

This is the table this decision exists to produce. Milestone 133's proposals, as
`notes/blocked-thread-teardown.md` states them: A finishes what `DESTROY` starts, on the region
capability. B puts a terminate verb on the thread capability. C frees the stranded caller.
C's piece 1 is being split out as a milestone of its own on the branch
`maintainer/mint-254-stranded-caller`, so it is out of scope here. D accepts the leak, bounds it,
and records it.

| | A: completed reclaim | B: terminate verb | D: accept and bound |
|---|---|---|---|
| **R0** nothing retained | **enables**, and is A's own premise | **forecloses in practice**: B's authority is a capability nobody holds after `START` | **enables** |
| **R1** supervision endpoint | enables, unchanged | **replaces** B with a variant on a different object, and owes §32, §40 and milestone 126 an answer | enables |
| **R2** thread capability | enables, unchanged | **enables**, and is the convention B's own failure analysis says it is missing | enables |
| **R3** new capability | enables, unchanged | enables, on the new object rather than on `ThreadControlBlock` | enables |
| **R4** declared retention | enables | **neutral**: declares whichever of the above is chosen | enables, and is the honest way to record it |

## What is blocked until this is answered

- Milestone 133's A/B/D choice. Not blocked in the sense of unable to proceed; blocked in the
  sense that proceeding decides this file's question without reading it.
- Milestone 105's fork two (`Tcb::NAME`, a tid that becomes a handle), which is the same question
  asked from the naming side and has been open since 2026-08-04.
- §26.2's deferred `SET_FAULT_EP`, which is the R1 row in miniature and was already deferred once
  with the note that reattachment "is a new decision when it does" arrive.


## The decision, 2026-09-03

R4 declaring R0, and milestone 133's proposal A ships alongside it rather than after it.

### What is decided

Retention becomes a declared field on `ChildEndowment`, so one struct literal states both what a
child is given and what its spawner keeps. `build_child` acts on it: the `ThreadControlBlock`
capability is deleted at `START` unless the endowment says otherwise. The field has no default that
hides the question, which is rung one of AGENTS.md's ladder for the specific failure that a spawn
path forgets to decide.

What it declares today is R0, which is what 25 of the 30 `START` sites already do. The five in
`fixtures/src/hello.rs` that keep their capability with no recorded reason become visible rather than
inferable, and each has to say why or stop.

### Why this releases milestone 133 instead of deferring it

**The objection to proposal A was silence, not the region capability.** A needs no new authority,
and that is most of its appeal. The risk recorded above is that taking it would settle retention *by
accident*, in R0's direction, after which every spawn path grows around it and R2 or R1 get harder.

A declared field removes the accident. With retention written down, A cannot settle anything
silently, because the convention is explicit and a later change is a change to what the field may
say rather than to thirty call sites and an unwritten habit. So the two are not sequential:

- Proposal A makes `MemoryRegion::DESTROY` finish what it starts, so a hung component's region
  becomes reclaimable. That is the capacity problem this whole area exists for.
- Milestone 254 (a caller stranded by a server that died is stranded forever) frees the stranded
  callers: they wake with `abi::Error::Gone`, so a server that merely died does not strand them.
- Together they close the compounding: one hang stops costing one region per caller in flight, and
  stops costing the one.

Cost: zero capability-table slots, no new syscall, no ABI change, and nothing overturned. Not
§32's line at the corpse, not milestone 126's rule that a domain names its members and does not act
on them, not §26.2's deferral of reattaching supervision. Both A and 254 need the reply-capability
sweep, and it is the same sweep.

### What is deliberately not decided

A supervisor still cannot end one thread without taking its region. That is proposal B's job and
B stays open, behind a customer that does not exist yet: a supervisor that must stop one child while
a sibling shares the region, or a policy separating stopping from freeing.

R1 is named here as the likely successor, because it is the answer that fits this system's own model
rather than seL4's. The supervisor already receives the fault, already reaps the corpse and already
rebuilds the child. Extending it from *notice death, reap, rebuild* to *notice hang, end, rebuild*
is the same job, and it costs no slots (§26.5, one endpoint per supervisor). §32's `REAP` already
proves the kernel can authorize an action on a thread off that endpoint. Its price is overturning
three recorded decisions, and that should be bought with a customer rather than speculatively, which
is the whole reason it is not being bought here.

**R2 and R3 are disfavored on model grounds and not merely on cost**, per the two findings recorded
in R2's entry above. This tree has no state-dependent authority for R2 to remove, and seL4's
lifetime handle earns its keep only alongside a repair-and-resume recovery model that §26 declined
in as many words.
