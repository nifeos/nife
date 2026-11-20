---
status: BUILT
raised: 2026-09-04
built: 2026-09-04
---
# 254. A caller stranded by a server that died is stranded forever, and nothing records that as intended

Built. This is the half of milestone 133 (ending a permanently blocked thread, and deciding who may)
that is a defect rather than a fork, which is why calef took it first.

In brief. `abi::Error::Gone` does not reach a reply-parked caller. The abort machinery walks an
endpoint's wait queues, but a caller whose request was *taken* was popped off at the rendezvous; it is
woken by `sched::ipc_reply` and by nothing else. So a caller stranded by a server that merely died
stays blocked for the life of the machine.

Nothing in this tree records that as intended, which is what makes it a defect rather than a
design. QNX Neutrino has not permitted it since the 1990s:

> If the server thread fails, exits, or disappears, the client thread becomes READY, with MsgSend()
> indicating an error

Zircon reaches the same outcome by a different route (closing the channel, rather than touching the
thread). Both are in notes/blocked-thread-teardown.md's survey, and nobody in that survey blocks
forever with no way out.

## Why this is separable from milestone 133, and why it goes first

Milestone 133 is a genuine fork: *which held capability expresses the right to end a thread*, with
four proposals conceding different things, and it touches the system call surface under one of them. This
is proposal C's first piece and it is the only part of that milestone that **asks for no authority
anybody does not already hold.** It rides on destroying a region or destroying an endpoint, both of
which the kernel already witnesses, and changes only their *reach*.

The note says so in as many words:

> It is the only proposal here that is plainly complementary to the others rather than an alternative
> to them

So it can be taken with A, with B, or alone, and taking it first costs milestone 133 nothing.

**It does not solve 133's capacity problem and must not be quoted as if it did.** A hung component's
own region stays unreclaimable. What this halves is the compounding: today one hang strands every
caller in flight and each stranded caller is itself an unreclaimable region, so a service with many
callers costs one region per caller. After this, one hang costs one region.

## The hazard this cannot ship without solving

Waking a reply-parked caller is exactly the path that opens the stale reply capability, and
milestone 133's block states it as a hazard for every proposal:

`cap::reply_cap` mints `Object::Reply(tid)`, whose payload is a generational thread name with no
call identity. `ipc_reply`'s guard checks the `WaitRole` and discards the endpoint, and that is
sound only because nothing today can leave a reply park and enter a second `CALL` while an
unconsumed `Reply` names it. This milestone creates that path. A hung server's stale reply capability
would then forge an answer to a later, unrelated call, and L4Re documents the identical hazard as a
consequence of its own finite receive timeouts.

Two known fixes, both seL4's, and this milestone must take one and say which: delete the
outstanding reply capability at abort (a cspace sweep, the pattern `sched::delete_frame_caps` already
establishes), or do not wake the victim at all. The second is not available here, because waking the
victim is the entire point.

**The sweep is not optional and is the larger half of the work.** A block that shipped the wake
without it would trade a permanent block for a forgeable reply, which is a worse defect in a
capability system than the one it fixes.

## What the mechanism already provides

Milestone 133's research established that the kernel can do this and only the authority was open.
For this half the authority question does not arise, so what remains is mechanical:

- Finding the reply-parked callers of an endpoint. Their `wait_on` records
  `(ep, WaitRole::Reply)`, so a scan over `MAX_THREADS` answers it.
- Waking them. `set_ipc_aborted` plus `wake` is the existing abort-and-resume pair, already used.
- The trigger. The destruction of the server's region or of the endpoint the call went to, both
  already witnessed.

## What was built

One helper that frees a caller, `strand_reply_caller`, two finders that decide which callers, and
four trigger sites. All of it is in `kernel/src/sched.rs`; no system call number, no method, no right,
and no ABI change, which is exactly what the block's gate line claimed before it was struck as stale
on the way to `BUILT`.

The helper does the two things in the order that makes them safe. First the sweep. Every
`Object::Reply(caller)` in every capability table in the machine is deleted, and so is one riding in
a thread's `outgoing_cap`, which is where a caller that met no server parks its own reply capability
awaiting a `RECV_CAP` hand-off. Then the abort and the wake, which is `set_ipc_aborted` plus
`wake`, the pair `reap_region_objects` already ran against every waiter it drained. The system call layer
needed nothing: `abi::rendezvous::CALL` already reads `take_ipc_aborted` and returns
`abi::Error::Gone`, so the caller returns through a path that has existed since milestone 12.

It touches only a thread that is genuinely reply-parked, which is `ipc_reply`'s own guard reused, so
it cannot clobber an ordinary receiver's park.

The four triggers, and why there are four rather than the two the block predicted. The block
named the destruction of the server's region or of the rendezvous. Reading the code found the
rendezvous is one path and *the server ceasing to be able to answer* is three, because a thread's
capability table can stop existing by three different routes:

| Trigger | Site | The case it covers |
|---|---|---|
| The rendezvous is torn down | `reap_region_objects`, rendezvous phase | Zircon's: the channel closes and the call in flight fails, without touching the server |
| The server exits or faults | `depart` | **QNX's headline case**, and the one the title names: the server thread fails, exits, or disappears |
| The server is forcibly killed | `schedule`, DECISIONS §16's armed-kill conversion | a killed thread never reaches `depart`, so nothing else would have swept it |
| The server is reaped with its region | `reap_region_objects`, removal phase | an `Embryo`, or a corpse already converted by one of the two above |

The rendezvous trigger cannot be reached by scanning a wait queue, and that is the defect stated
mechanically. A `CALL` caller whose request was taken is linked on no queue at all, so
`drain_waiters` walks past it and only `wait_on`'s `(ep, WaitRole::Reply)` still records that it is
waiting. The scan is over `MAX_THREADS`, and it rescans rather than listing, because a
`[u64; MAX_THREADS]` of victims is a kilobyte of scratch on the deepest frame in the kernel
(`reap_region_objects`'s own comment, and notes/stack-high-water.md). The abort flag is what
terminates the rescan, and it has to be the flag rather than `wait_on`: a wake deferred behind
`on_cpu` leaves the park in place until that core's `finish_switch`, so a predicate reading `wait_on`
alone would spin.

## The proof that this milestone worked

Two tests in `kernel/src/sched.rs`, and both were run against the kernel with the four trigger
calls commented out, where each fails at the assertion it exists for: "the stranded caller never
woke" and "the caller was still parked after its server exited". Both pass on all four boot
configurations the suite runs (aarch64, riscv64, x86_64, and x86_64 under OVMF).

- `a_reply_parked_caller_wakes_with_an_error_when_its_rendezvous_is_reclaimed`. A server collects the
  request, keeps the reply capability and never answers, staying alive on purpose so that the
  rendezvous going away is the only thing that can free anybody. The caller returns `Gone`. Its
  neighbor two tests up, `a_blocked_waiter_wakes_with_an_error_when_its_rendezvous_is_revoked`, is
  the case that always worked, and the difference between the two is exactly one collected
  message.
- `a_server_that_exits_frees_the_caller_it_never_answered`. The server collects and returns. The
  rendezvous outlives it, so nothing else in the kernel is looking at the caller.

The stale-reply half is asserted in both, which is the half that matters: without it this
milestone would have traded a permanent block for a forgeable reply, and a test asserting only the
first would pass while that were true. Two assertions carry it. `outstanding_reply_capabilities`
counts every live `Reply` naming the freed caller anywhere in the machine, including `outgoing_cap`,
and must be zero. And the surviving server in the first test re-reads its own slot through
`sched::current_cap`, which is the same lookup `abi::reply::REPLY` performs before it ever reaches
`ipc_reply`. A capability that is gone there is a reply that cannot be sent at all. Each test
also asserts the server *held* a live capability beforehand, so neither can pass vacuously.

Note what the sweep is doing and what it is not. `ipc_reply`'s guard still checks the `WaitRole` and
discards the rendezvous, so the kernel-internal function would still deliver to a re-parked caller if
anything could call it without a capability. Nothing can: the capability is the gate, and the sweep
takes the capability. That is seL4's non-MCS answer (`cteDeleteOne(callerCap)` off `cancelIPC`)
rather than a call identity in the payload, and a call identity remains the stronger fix somebody
could still take.

## BUGS

- **It does not reclaim the hung component's region**, which is milestone 133's capacity problem and
  stays open. This halves the leak; it does not close it.
- A scan over `MAX_THREADS` per destruction is a linear cost on a path that is not hot but is not
  free either. It is measured now, because `script/bench`'s icount tripwire caught the first
  version: `spawn_reap` went +25.9% and `map_new` +28.7%, both against a 10% bound. The cause was
  not the `MAX_THREADS` scan at all but `strand_callers_of` re-resolving the departing thread
  through the generational thread table once per capability slot, 24 lookups on every thread exit,
  about 830 ticks per `spawn_reap` iteration. Reading the table once into a 192-byte array of
  victims and skipping an empty table took it to +2.2% and +0.006%. **The baselines were not
  re-recorded.**
- **It says nothing about a server that is alive and simply never replies**. That is the case that
  motivated milestone 133 and is answered by a deadline on `CALL` (milestone 106's fork, not this
  one's) or by ending the thread (133's).
- **A server that is alive and holding a reply capability it never uses still strands its caller**,
  and that is not an oversight: it is the same case the third bullet below names. Nothing here
  fires while the server is running, because nothing has happened that the kernel witnesses.
- `ipc_reply`'s guard still checks the role and discards the rendezvous. What makes the forged
  reply impossible is that the capability is gone, not that the guard got stronger, so a future path
  that could reach `ipc_reply` without presenting a capability would reopen the hazard. A call
  identity in `Object::Reply`'s payload is the structural fix and nobody has taken it.
- The `MAX_THREADS` scan and the capability sweep are unmeasured, which the block already said.
  The scan is one pass over 128 threads per rendezvous destroyed; the sweep is 128 x 16 comparisons
  per caller freed. Both are on teardown paths and neither shows in `script/bench`'s icount tripwire,
  because no benchmark tears a server down mid-call.
- The kill-site call sits in `schedule`, the hottest path in the kernel, guarded by
  `killed && state == Running` so it never runs on an ordinary pass. That guard is the whole
  argument, and it is a comment rather than a type.

  `script/fastpath-footprint` caught this and the first attempt was 20% over. Inlined into
  `schedule()`, the helpers put 1,363 bytes on x86_64's IPC fastpath figure (6,639 to 8,002), 26.5%
  on aarch64 and 33.1% on riscv64, for code that runs only when something is being torn down. The
  fix is `#[cold]` plus `#[inline(never)]` on all three helpers, and `strand_` added to that
  script's `COLD` list beside `revoke`, `destroy` and `delete_frame_caps`, which is the teardown
  family it already names for exactly this reason. What remains is the guarded call sites
  themselves, at +0.6% aarch64, +0.3% riscv64, +1.9% x86_64, all inside the 5% bound. **The
  baselines were not re-recorded**, which is the honest form of a change that is meant to cost
  nothing on the hot path.

## Follow-on

- **Recorded.** A live-but-silent server still strands its caller. That is milestone 133's subject
  (ending the thread) and milestone 106's (a deadline on `CALL`), and it is named in this block's
  `BUGS` and beside the feature in `ipc_call`'s own doc comment in `kernel/src/sched.rs`.
- **Recorded.** `Object::Reply` still carries a thread name with no call identity, so the guard in
  `ipc_reply` remains role-shaped and the capability sweep is what holds the property. Written into
  the `BUGS` above and into `strand_reply_caller`'s doc comment, where the next reader of that code
  meets it.
- **Milestone 371.** A call identity in the reply capability's payload, which would make the hazard
  unrepresentable rather than swept: `design/roadmap/0371-a-reply-capability-that-names-a-call.md`.
- **Milestone 133.** Reclaiming the hung component's own region, which this does not touch and must
  not be quoted as doing.

## Index row

`Gone` reaches endpoint wait queues and not reply parks, so a dead server blocks its callers for
the life of the machine
