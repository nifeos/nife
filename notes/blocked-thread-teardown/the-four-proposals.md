# The four proposals in full

*Name: provisional.*

[`notes/blocked-thread-teardown.md`](../blocked-thread-teardown.md) lays the four proposals side by side
in one table and says which were built. This is each one in full, as it was written before calef chose proposal A on 2026-09-03, with proposal C's block
updated for what milestone 254 (a caller stranded by a server that died)
built.

Each states the mechanism, the authority, the cost, what it breaks, and how it fails. They are not
ranked, and none is recommended. They are ordered from smallest surface to largest.

---

## Proposal A: `DESTROY` finishes what it starts

Provisional name for the concept: *the completed reclaim*. Not minted; an architect's call.

Mechanism. In `reap_region_objects`'s refuse phase, a resident thread that is `Blocked` is not
merely marked `killed`. It is unlinked from whatever holds it (`remove_sender`, a new
`remove_receiver`, or nothing at all for `WaitRole::Reply`), every `Object::Reply(tid)` naming it is
swept out of every capability table, and its state is written straight to `Finished` without ever waking it.
The next pass, which the owner's existing retry loop already performs, finds a reapable corpse and
reclaims the region. `RegionReap` grows a fourth verdict; `region_reap_verdict` already exists as a
lifted, testable function precisely so a rule like this can be stated and proved without staging a
race.

Authority: the region capability. No new right, no new method, no new syscall number, no ABI
change. The holder of the `Untyped` has already said this region is going away.

What it costs. Roughly thirty lines plus `remove_receiver`. One reply-cap sweep per victim, O(128
x 16). No user-visible surface change whatsoever, so no program in the tree needs editing and no
documentation outside the kernel changes.

What it breaks.

- A `Blocked` thread dies without running another instruction. It never returns from its syscall,
  never sees `Gone`, never runs a destructor. This is seL4's `ThreadState_Inactive` and Zircon's
  entire objection to thread killing, and the objection lands: locks it holds stay held, and whatever
  it was in the middle of stays in the middle. The mitigating fact particular to this kernel is that
  a `Blocked` thread holds no *kernel* state in flight, and its region is being destroyed anyway, so
  the "process left in a bad state" hazard applies to the region's other threads and to nothing else.
- Destroying region A silently removes a waiter from an endpoint in region B. The endpoint's owner
  observes a sender that never arrived, which is indistinguishable from a client that never called.
  The precedent is settled (`remove_sender` on a supervisor's endpoint already does this) but the
  scope widens from corpses to live threads.
- `killed` acquires a second meaning, or, better, loses its old one for blocked threads. Worth
  deciding explicitly rather than letting the two coexist.

How it fails. It does not solve the *stranded caller in another region*. A client of the hung
server, blocked in `CALL`, sits in its own region, and freeing that region is its own owner's act.
Proposal A makes each region individually reclaimable by its own owner, which is enough for the
capacity problem and is not the same as an abort. `wait_on` could go stale: a future block site might
write `state = Blocked` by hand instead of calling `park`, which `thread_wake_handshake`'s `BUGS` says
nothing prevents. Then the unlink targets the wrong queue or none, and the failure is a dangling
pointer in an endpoint queue: the worst failure mode in the set. That argues for a `debug_assert!` pairing
`Blocked` with `Some(wait_on)`, and possibly for the lint-shaped fix that crate declined.

---

## Proposal B: a terminate verb on the `ThreadControlBlock` capability

Provisional names, all unratified: `ThreadControlBlock::TERMINATE`, `ThreadControlBlock::STOP`,
`ThreadControlBlock::CANCEL`. An architect's call, and the vocabulary matters more than usual
because L4 shipped two words for this and cannot tell them apart in its own documentation.

Mechanism. seL4's `suspend()`, transliterated: cancel the IPC (unlink, sweep the reply caps),
dequeue, set a terminal state. A `ThreadControlBlock` capability's methods stop refusing non-`Embryo` threads for
this one verb.

Authority: a `ThreadControlBlock` capability with `WRITE`. Delegable, narrowable, and it names exactly one thread
that the holder was handed. Endpoint-only naming is not breached.

What it costs. A new method number in `abi::tcb`, a `DECISIONS` section for its semantics (§10 (process)'s
rule), and the widening of what a `ThreadControlBlock` cap means. Existing holders gain a power they were not offered;
`crates/system_initializer` and every spawn path would need auditing for who ends up holding one after
`START`.

What it breaks.

- The construction-time invariant. Today a `ThreadControlBlock` cap is inert once the thread runs, and that is a
  clean, checkable property. Afterwards it is a lifetime handle, and "who holds a `ThreadControlBlock` cap on this
  thread" becomes a question with security weight that it does not have now.
- It gives a *narrower* authority than proposal A gives, which sounds like an advantage and is
  also the risk: a holder can end one thread without owning its region, so the thread's memory is not
  thereby reclaimed. Ending a thread and reclaiming its memory become two acts with two authorities,
  and the hung-component note already found that conflating them is what made §32's sentence half
  wrong.

How it fails. If the answer is "suspend, then the region owner destroys", it needs both
authorities to be held by cooperating parties, and a supervisor holding a `ThreadControlBlock` cap but no region
capability can stop a hang without reclaiming anything. That may be exactly right (it separates
policy from memory) or exactly wrong (it produces a stopped thread nobody can free), and which one it
is depends on a spawn-time endowment convention that does not exist yet.

---

## Proposal C: the caller's side, and only the caller's side

Provisional name: *the abortable call*. Unratified.

Mechanism. Leave the hung *server* alone entirely and fix the stranded *client*, which is QNX's
behavior and Zircon's. Two independent pieces, and they can be taken separately:

1. A reply-parked caller is woken when the thing it awaits becomes unreachable, closing the gap
   the hung-component note found: `Error::Gone` reaches endpoint wait queues and not reply parks. The
   trigger would be the destruction of the server's region or of the endpoint the call went to, both
   of which the kernel already witnesses. This makes case (b) work for callers the way it already
   works for waiters, and it needs a way to find reply-parked callers of a given endpoint: their
   `wait_on` records `(ep, WaitRole::Reply)`, so a scan over `MAX_THREADS` answers it.
2. A deadline on `CALL`, which is L4's answer and is milestone 106's fork rather than this one's.

Authority: none new for piece 1. It rides on authorities already exercised (destroying a region,
destroying an endpoint) and changes only their reach. Piece 2's authority is the caller's own.

Piece 1 is built (milestone 254, 2026-09-04), and building it moved one thing this note got
slightly wrong. The trigger is not two events but four, because *the server ceasing to be able to
answer* is three of them. It exits or faults (`depart`). It is forcibly killed (the §16 (object revocation)
conversion at
the top of `schedule`, which never reaches `depart`). Or it is reaped with its region. The fourth is
the rendezvous teardown this note named. All four call one helper, `sched::strand_reply_caller`,
which sweeps every `Object::Reply` naming the caller before aborting and waking it. The scan for
reply-parked callers rescans rather than listing, for the stack reason `reap_region_objects` already
argues at length. See design/roadmap/0254-a-caller-stranded-by-a-dead-server.md.

What it costs. Piece 1 is small and is arguably a bug fix rather than a feature. A caller stranded
by a *dead* server is stranded today. QNX has not permitted that since the 1990s, and nothing in
this tree records it as intended. Piece 2 is a syscall-surface change and a timer, and is 106's.

What it breaks. Piece 1 makes `Gone` reachable at reply parks, which is exactly the path that
opens the stale-reply-capability hazard, so it cannot ship without the reply-cap sweep. Piece 2
inherits the L4 warning the parent quotes, word for word.

How it fails. It does not reclaim the hung component's own region, which is the capacity problem
that motivated the whole question. It halves the leak (one hang costs one region instead of one per
caller in flight) and leaves the other half. It is the only proposal here that is plainly
complementary to the others rather than an alternative to them, and that is worth noticing when
ranking: it can be taken with A, with B, or alone.

---

## Proposal D: accept the leak, bound it, and say so

Argued properly, because it may be right.

Mechanism. No kernel change. Instead:

1. Record the limitation where a reader meets it, which is `reclaim_region`'s `BUGS`, `Holding`'s
   `BUGS`, `notes/frames.md` and `notes/quotas.md`. Today `reap_region_objects`'s comment is the only
   place this fact lives, and a comment inside a kernel function is rung four.
2. Bound it with the quota machinery that already exists. `QuotaToken` is documented as holding a
   spawner's slot precisely for this case: "a child that blocks forever keeps holding it, which is
   correct: it is still consuming a thread, a stack, and an address space." A supervisor that spawns
   from a bounded budget cannot leak unboundedly; it runs out of children and refuses, loudly, at a
   place a human can see.
3. Make the leak visible. `SURVEY` already reports `BLOCKED`; a count of unreclaimable regions is
   a `free`-shaped program (milestone 126 already names `Untyped` + `ENUMERATE` as wanting exactly
   this) rather than a kernel change.
4. Recover the service, not the memory, which is what the hung-component lane demonstrated works
   today with no new authority: build, revoke the device, start the replacement.

Authority: unchanged. Nothing new anywhere.

The argument for it, which is not a straw man.

- §40 (supervisor) is "no reaper of last resort", and Zircon deleted this feature after shipping it. RFC-0007
  is a system with far more users than this one concluding that thread killing "encourages bad
  practices" and removing it. A project that adds it should be able to say why Fuchsia was wrong, or
  why its case differs. The honest difference is that Fuchsia kills the *process* instead, which nife
  can do for cases (a) and (b) and cannot for (c); the honest similarity is that both are arguing
  about the same hazards.
- The failure this prevents is a capacity failure, and a capacity failure is survivable and
  visible. Running out of untyped budget is a refusal at a call site, not corruption. A forcible
  teardown's failure mode is an invariant broken inside a component that was still holding something.
- The customer path may not need it. A backup target's hang budget is set by its supervisor's
  quota. If the supervisor can restart the service (which it can), and the operator can restart the
  machine on a schedule it already has, an unreclaimable region is a monthly reboot rather than an
  outage. That is a real answer, and it should be tested against the actual budget before a kernel
  change is spent on it.
- This tree has refused stronger machinery on principle before, and the refusals are on the record
  rather than being drift.

How it fails. The bound is per-supervisor, not global: N supervisors each with a quota of M leak
N x M regions before anyone refuses, and nothing composes those bounds. It also makes the *first*
hang free and the *last* one fatal with no gradient in between, which is the shape of failure that
looks fine in testing and arrives at 3 a.m. And it leaves this kernel alone in the parent's survey table
in having no way out at all, which is a claim a stranger reading the demonstrator will notice.

---

## Questions the note left open

What I would want to know more about, offered as questions rather than as a ranking.

1. Is the stale-reply hazard reachable today by any path I did not find? I convinced myself it is
   not, by enumerating the exits from a reply park, but it is a proof by exhaustion over code I read
   rather than a checked property. It is Kani-shaped if `ipc_reply`'s guard were lifted into a
   testable function the way `region_reap_verdict` was.
2. What does the customer path actually need? Proposal D's case stands or falls on whether
   milestone 55's supervisor quota bounds the leak below the reboot cadence, and that is a number
   nobody has measured. It is measurable now.
3. Is C separable enough to take on its own merits? It looks less like a fork and more like a
   defect: a caller stranded by a *dead* server is a QNX behavior from the 1990s that this kernel
   lacks, and nothing in the tree records that as intended. Answered yes, by calef on
   2026-09-04, and built as milestone 254 without touching A, B or D.
