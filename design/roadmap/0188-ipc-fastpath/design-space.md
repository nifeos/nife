# The design space: what a fastpath may skip, how it stays proved, and the alternatives priced

An appendix of [milestone 188 (the IPC fastpath)](../0188-ipc-fastpath.md), moved out of it on
2026-10-12 (UTC) under §212 (a prose budget). The main page is complete without it and keeps each
section here as a summary. This file keeps the full reasoning, written during scoping (2026-08-28 to
2026-09-04) for whoever takes phase 4. The per-symbol figures it cites are in
[scoping.md](scoping.md).

## What a fastpath would skip, and what it cannot skip

Skippable. The first item is the only one with a measured size; the rest are read off the source
and a phase-3 lane should price them rather than assume them.

- The general scheduler re-selection, 2,872 bytes. A rendezvous that finds a waiting partner
  knows which thread runs next. seL4's fastpath switches directly to it. Ours calls `schedule()`,
  which re-runs policy, and then `finish_switch`, which carries the corpse-reaping branch that
  milestone 132 already had to classify as cold to keep the number honest.
- `trace::record`, called on every serve and every block in `ipc_call` and `ipc_reply`. It has
  no symbol of its own in the closure, so its bytes are inlined into the callers and are part of
  their 1,692 and 480 rather than a line of their own.
- `kmem::recycle`, 280 bytes, if a successful rendezvous never reaches it. The gate's closure
  includes it because `finish_switch` calls it. Whether a switch that came from a completed IPC can
  reach it is a question phase 3 should answer. A wrong answer here is the same silent
  mis-classification the cold list already risks. (Phase 3 answered it: it cannot, and it left the
  closure. See [phases-1-to-3.md](phases-1-to-3.md).)
- The error-return plumbing. `take_ipc_aborted()` is called after every rendezvous method in
  `system_call::invoke` and is a second load and branch on a path that has already succeeded.

Not skippable, and this is the half that makes it a design milestone.

- The capability lookup. `sched::current_cap(slot)` at 372 bytes is the bounds check that *is*
  the security mechanism; `system_call::invoke`'s own doc comment says so. A fastpath that caches or
  elides it is not a fastpath, it is a hole.
- The rights check. `SEND` needs `WRITE`, `RECEIVE` needs `READ`, `SEND_CAP` needs `GRANT` on the
  delegated capability and a subset check on the narrowing. `CALL` needs `WRITE`.
- The one-shot Reply mint and its consume-on-use. DECISIONS §12 (a one-shot reply capability) is
  the whole reason a CALL is answerable exactly once. The fastpath still has to mint the capability,
  insert it into the server's sixteen-slot table, and handle the table being full.
- The `WaitRole::Reply` guard. `ipc_reply` refuses any thread not parked as a reply waiter, and
  its comment records why: a stale reply landing on an ordinary receiver would clobber a mailbox and
  double-enqueue the one intrusive link. That guard is on the fast path by construction.
- Generational thread-id revalidation, which notes/benchmarks.md names as one of the three
  things ours does that seL4's fastpath does not.

## What correctness must not be lost, and how it would be proved

The proofs are on the general structure, which is exactly the structure a fastpath exists to
bypass. `crates/inter_process_communication` carries six `#[kani::proof]` harnesses over
`Rendezvous`. They prove:

- that send and receive and signal each preserve the one-queue invariant;
- that a send rendezvouses iff a receiver waited, and with exactly that receiver;
- that a pending signal is drained before a queued sender;
- that a collected sender is forgotten by the rendezvous, which is the rendezvous half of the
  one-shot Reply guarantee.

`crates/capability_table` carries twelve more. `script/verify` runs them.

None of them prove anything about `kernel/src/sched.rs`. They prove the pure decision core that
`sched` calls into. A hand-written fastpath has two possible relationships to that, and choosing
between them is most of phase 4's design:

1. The fastpath calls `Rendezvous::send`/`receive` too. The proofs continue to cover it for free.
   It keeps the part of the cost that lives in the queue manipulation, so the win shrinks to
   the scheduler bypass and the skipped error plumbing. This is the option that should be measured
   first, because it may be most of the win for none of the verification cost.
2. The fastpath replicates the decision. Then it needs its own harnesses, plus something the
   tree does not have today: an equivalence proof. The shape is tractable precisely because
   `crates/inter_process_communication` is a pure-logic crate the solver already handles. The
   harness seeds a nondeterministic `Rendezvous` with `kani::any()`, runs the fastpath's predicate
   and the general `send` over the same state, and asserts the same outcome and the same resulting
   queue state. That harness, not a test, is what would make a second path safe to keep.

The invariants both paths must hold, stated so a phase-4 lane can turn each into an assertion.
A message is delivered to exactly one receiver or to none. A capability is never widened. A Reply is
consumed on first use. A blocked thread appears on at most one queue. A thread's mailbox is written
only while it is parked in a role that expects that write. The first four already have harnesses in
`crates/inter_process_communication` or `crates/capability_table`. The fifth is `ipc_reply`'s
`WaitRole::Reply` guard and is currently held by a comment and a runtime check.

## How the two paths stay in agreement

This is the honest reason not to do phase 4 lightly, and it is a cost that never ends. seL4's
fastpath is hand-maintained, and hand-maintained means every future change to the IPC surface must
be made twice or must be proved not to reach the fast path.

A differential test that drives every operation through both paths and compares is the obvious
answer. It is not sufficient on its own, for the reason this tree already records about the
`fastpath-footprint` cold list: a wrong entry is silent. A fastpath's whole design is a predicate
that says "this case is simple enough". A test can only exercise the cases somebody wrote down, and
the failure mode is a case that takes the fast path and should not have. What is sufficient is the
pair: the equivalence proof above for the decision, plus a differential test for the plumbing the
proof does not model (the capability table, the mailbox, the trap frame).

Three mechanisms are available and they are the AGENTS.md ladder in order, strongest first:

1. Make divergence unrepresentable. If the fastpath and the slow path both reach one function
   for the check they share, there is no second copy to drift. This is what option 1 above buys.
2. A gate that fails loudly. The equivalence harness in `script/verify`, plus a differential
   test in the kernel's own suite.
3. A written record at the thing itself. A comment on every rights check saying which path also
   performs it. This is the floor and should not be the plan.

## The alternatives, priced

Per AGENTS.md's six questions, and refusals with reasons, because the refusals are the useful half.

### A. Extract cold arms with `#[inline(never)]`

Already built; measured. Milestone 156 (extract the rest and ratchet both ways) did this to `invoke`'s administrative arms and records its
own before and after. On aarch64 `system_call_entry` went 4,168 to 3,480, and on riscv64 2,914 to
2,210: a 16.5% and a 24% cut. Further work since has taken those to 3,304 and 1,870, so the entry
figure is down 20.7% and 35.8% from the pre-156 baseline in total. Over the same period
`ipc_fastpath` went from 5,780 to 5,788, which is eight bytes. The cheap method has delivered its win
on the entry figure and has not been tried at all on the fastpath closure, where 88% of the overrun
now lives. That is phase 3.

### B. Reorder the dispatch decode so common opcodes are found first

Refused, and it is worth saying why loudly. It would not move this number by a byte.
`script/fastpath-footprint` sums whole symbol sizes; reordering a match changes which instructions
execute, not which symbol they live in. It is already effectively done: `SYS_INVOKE` is one of four
system call numbers, and `Object::Rendezvous` is the first arm of `invoke`'s object match.
`SEND`/`RECEIVE`/`SEND_CAP`/`RECEIVE_CAP`/`CALL` are method numbers 0 through 4. A lane that reached
for this would spend a day and report a flat number.

### C. Shrink argument validation on the hot path

Refused. The validation on this path is the capability lookup and the rights checks, which the
section above establishes cannot be skipped without turning the fastpath into a vulnerability. There
is nothing else to shrink. DECISIONS §10 (process model: capability-based, microkernel) means no
pointer crosses the boundary, so there is no user-memory validation on this path to begin with.

### D. Split dispatch into a hot decoder that tail-calls a cold one

Subsumed by A, which is the same idea with the extraction done per method rather than per decoder,
and which is already built.

### E. Fix what is measured before optimizing it

This is the recommendation, and it is phases 1 and 2. It shrinks nothing and it is a precondition
for every other option being judgeable.

### F. A hand-written fastpath, seL4's shape

Phase 4. Deferred, not refused.
