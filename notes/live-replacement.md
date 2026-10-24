# Live component replacement

*Milestone 23, DECISIONS §41. The flagship the roadmap points at: a running component is replaced
under a client that is talking to it, and the client's stream is unbroken.*

## The shape

```text
                                  ┌──── the stable name: one endpoint object, forever ────┐
   swap_boundary_witness ──CALL──►│                       SVC                              │◄─RECEIVE_CAP── rust_swappable  (v1, Rust)
   (a client)                     └────────────────────────────────────────────────────────┘◄─RECEIVE_CAP── c_swappable (v2, C)
                                                           ▲
                                                           │ swapper, an unprivileged operator, changes
                                                           │ WHICH of the two is parked in RECEIVE_CAP
```

Five programs, sharing one module (`swap.rs`) the way the supervision tree
shares `supervision_protocol`:

| program | what it is | what it holds |
|---|---|---|
| `swapper` | the operator: builder, supervisor and verifier | a budget, one device capability, four endpoints |
| `rust_swappable` | the component, version 1 (Rust) | the service endpoint (READ), a report endpoint, a coordination channel, the device, a shared page |
| `c_swappable` | the component, version 2, whose answers are computed in C | identical |
| `swap_boundary_witness` | the client, the producer, and the attacker (three roles, one binary) | the service endpoint (WRITE, not READ) |
| `broker` | the queue broker, the ladder's opt-in rung | a front endpoint (READ), a back endpoint (WRITE) |

## Why there is no broker in the fast path

This is the thing to understand about the milestone, and it is a property the kernel already had.

A client names an endpoint, never a peer (DECISIONS §12, notes/ipc-naming.md). The rendezvous is
anonymous in both directions: a server that `RECEIVE`s does not learn who sent, and a client that
`CALL`s does not learn who answered. So a component's identity is not merely hidden from its
clients, it is *not represented anywhere a client can reach*. Any program that speaks the protocol
and holds the right capabilities is the component.

That makes the stable name the endpoint object itself, and a swap a change in who is parked in
`RECEIVE_CAP` on it. Two consequences, both of which a forwarding broker would have had to reimplement
at a cost:

1. The kernel's sender queue is the buffer for the down window. A `CALL` that finds nobody
   receiving parks the caller as a blocked sender, with its message in its mailbox and the one-shot
   `Reply` capability the kernel minted for it riding in `outgoing_cap`. Whenever the *next* server
   calls `RECEIVE_CAP`, it takes both, and answers a caller it was never wired to. So while the
   endpoint has no server at all, requests are not lost, not refused, and not reordered; the caller
   is simply blocked, which is what a synchronous IPC caller already is.
2. The drain is a message traveling in band. The operator's `OPERATION_QUIESCE` goes to *the endpoint
   being drained*, and the sender queue is FIFO, so by the time it arrives the incumbent has answered
   everything queued ahead of it. No quiescence handshake, no timeout, no window to guess at.

The cost of all this is zero: the steady state is `call_reply`, the same path a client and server
already use (notes/benchmarks.md).

What endpoint-only naming does not mean is "whoever holds the endpoint is the server". `SEND` and
`RECEIVE` are gated by *different rights* on the same object, so the same endpoint handed out two ways
is a one-way pipe in whichever direction each holder was trusted with. `swap_boundary_witness`'s
usurper role holds the honest client's exact capabilities and tries to receive on the service
endpoint; it gets `NotPermitted`.

## The steps

```text
  1 BUILT    lay the replacement out, endow it, retype its TCB -- but do not configure or start it.
             A thread that was never started is in nobody's queue, so it cannot take a request the
             incumbent is still there to serve.
  2 DRAINED  CALL OPERATION_QUIESCE on the service rendezvous. FIFO does the waiting. The incumbent replies
             and stops receiving; requests from here on park on the sender queue.
  3 REVOKED  DeviceFrame::REVOKE the device capability. Gone from every holder but the operator.
  4 STARTED  map the registers into the replacement, CONFIGURE, START. It drains the parked
             requests. The down window ends: four syscalls wide.
  5 REAPED   the incumbent is told to read one device register, faults, and its death arrives on the
             supervision endpoint; Rendezvous::REAP collects the corpse and returns its region.
```

The roadmap put the revoke second and building the replacement first, and both halves of that are
right for reasons it did not give. Revocation is by *physical page* (DECISIONS §13 (capability)). So
a revoke that ran after the replacement had been endowed with the device would take the
replacement's copy too. Since the kernel mints a `DeviceFrame` once at boot, nothing could hand one
back. What moves
to the far side of the revoke is the endowment, not the build. And the build has to stay first
for a second reason found by running it. Process construction is a few hundred syscalls, and when
the build was moved after the swap trigger the client finished its entire conversation on RISC-V
before
the operator was ready.

## Taking a device back

`PageFrame::REVOKE` on a `PageFrame` un-shares a page from everyone including the caller, because §13
exists to make *reclamation* safe and a page about to be returned to the allocator must not stay
reachable. On a `DeviceFrame` the same method means take-back: every other holder loses the
capability and the mapping, the invoker keeps its own.

The asymmetry is forced, not convenient. A device page is never reclaimed, and the kernel mints its
capability once, at boot, so a symmetric revoke would strand the UART for the rest of the machine's
life. Two objects, two purposes, one verb: reclamation versus exclusive ownership transfer.

This is the "deferred capability-derivation tree finally earning its keep" the roadmap predicted, and
it is one level of that tree: the invoker is the root by construction (it holds `GRANT` and it is the
one asking) and everyone else is a derivative. Revoking one *named* holder while sparing another
still wants the real tree, and still is not built.

## How "the client did not notice" is proven

The shape milestones 29, 33 and 36 used: two witnesses in two address spaces, an attacker with real
authority, and a control that must fail.

Witness one, the client, in its own address space. `swap_boundary_witness` calls sixty-four times in
a plain loop, holding one capability for its whole life. It never reconnects, never retries, and has
no code path for "the server went away" because there is no such event to have one for. It checks,
from what it saw, that every call returned. Every reply echoed the sequence number that asked for it
(so the kernel's one-shot `Reply` never misrouted). Every digest matched its own independent
computation of the same definition. And the version word went up exactly once, somewhere strictly
inside the conversation.

That last one is worth stating precisely. The client *can* tell a swap happened, because the reply
carries a version word put there for exactly that purpose. The claim is that its stream was
unbroken, not that a swap is undetectable by a client that goes looking.

Witness two, the operator, in a different address space. A page `swapper` owns and maps read/write
into each instance; each stamps its own version at the index of every request it serves. Read after
every writer is dead, it says two things the client cannot. No sequence number went unserved
(nothing was lost in the down window). And the version never goes backwards, which is the
"there were never two owners" assertion, because two instances serving concurrently would interleave.
The two witnesses are cross-checked against each other on *where* the swap happened; neither is taken
on the other's word.

The control that must fail. After the revoke, the outgoing instance is told to read one UART
register. It faults, and the kernel's fault message carries the device's own page. Before the revoke
the identical read succeeded (each instance probes at startup and reports), which is what makes this
a receipt rather than a coincidence. A run in which that read *succeeds* is failed loudly rather than
silently: the instance reports `RPT_PROBE_SURVIVED` and the test refuses the run.

The attacker. Endowed with exactly the honest client's capabilities, including a real working
capability to the stable endpoint, it tries to park itself in `RECEIVE_CAP` and take the client's
requests. `NotPermitted`.

And the replacement is written in C (`fixtures/c/c_swappable.c`, over the seam DECISIONS §31 built).
That is the strongest form of the claim available: what held across the swap is the *contract*, not
a recompile of the same source. The C holds no capability and makes no syscall, because the Rust
shell around it holds every capability and makes every syscall; its entire interface to the system
is `(uint64_t) -> uint64_t`.

## The latency ladder

The roadmap's rule is opt-in per channel, never the default, and it is a rule because of a
number.

| rung | what it is | steady-state cost | what it decouples |
|---|---|---|---|
| 0 (default) | the shared endpoint; no process in the path | zero (`call_reply`) | lifecycle, at the price of blocking the caller during the window |
| 1 (opt-in) | `broker`, a queue-server process | 1.99x a direct call, ~1.2 us under HVF | lifecycle, with the producer never blocking |
| 2 | a durable broker that writes the backlog to storage |: | its own crash. Not built. |

`broker` is pass-through when both ends are up. It forwards the two words and hands the backend's
answer straight back, holding the client's `Reply` capability across the hop. When the operator
tells it the backend is away it answers `ACCEPTED` immediately and holds the item in its own `.bss`.
That lives in the region it was built in, so the bound is a bound on *its* memory and the kernel's
footprint is unchanged. (A runaway producer gets `QUEUE_FULL`, which is backpressure as a value
rather
than a policy hidden inside a server). On the way back up it drains in arrival order before it
answers, so "the broker is up" and "the backlog is delivered" are one event to anyone watching.

Its control messages travel in band on its own front endpoint, for the same reason
`OPERATION_QUIESCE` does. Synchronous rendezvous means a server blocks on one endpoint, and a second
one would need the
wait-any primitive DECISIONS §26.5 deliberately does not have.

## The system reclaims itself, and the test asserts it

Every child the operator starts is supervised, and every corpse is collected through the supervision
endpoint (DECISIONS §32), which returns each instance's region to the operator's budget. So
reclaiming the budget at the end can only *succeed* if all five splits are gone: §16 refuses a region
whose children are still carved out of it.

That is an assertion rather than housekeeping, for a reason with nothing to do with tidiness.
`untyped::create` takes a contiguous run of frames, so leaking the three systems fragments the frame
allocator badly enough that a *later, unrelated* test cannot get the progenitor's own eight-megabyte
region. The failure surfaces nowhere near its cause, which is the usual signature of a leak.

### BUGS: the frame-hygiene `debug_assert!` was a race, and it fired on CI

Removed 2026-08-03, in the change this section said it deserved. By that day it had failed the
cpu matrix on `main` twice (once on `rv64`, the control model) and once on a Dependabot PR whose
diff touched only workflow files. So the analysis below had been confirmed at the rate it predicted,
and the assertion was costing red CI runs on innocent changes. The rest of this section is the
analysis as recorded at the time, kept because the failure shape (a wait or an assertion written
against something wider than the property) recurs in this tree.

`run_swap` ended with `debug_assert!(before >= memory::free_frames())`, where `before` was the free
count sampled at the top of the run. It intermittently failed on the `sifive-u54` cpu-matrix leg,
with the outgoing instance's expected device fault printed just above it. Found 2026-08-03 during
milestone 72; not fixed on that branch, because the analysis says the assertion is the defect and
that deserves its own change rather than riding on an unrelated one.

Intermittent, and the rate is worth writing down so the next sighting is not read as a regression.
On the one branch where it has been watched closely it went success, success, failure, success
across four completed `cpu matrix` jobs. That was on a diff that changes no executable line in this
file's neighborhood. One failure in four is what this looks like.

It contradicts the comment directly above it, which says the property is "hygiene, deliberately
not asserted on". Both were written in the same commit, so one has been wrong since milestone 23.

The comment is the one that is right, though not for the reason it gives. The scenario the
comment describes, the operator's own address space and TCB coming home late through the ordinary
reaper, satisfies the assertion either way. Those frames were allocated *after* `before` was taken,
so returning them can only bring the count back up *toward* the baseline, never past it. The
assertion can only fire on frames arriving from outside the run, which is an earlier test's
teardown landing mid-run. That is the "a wait written against something wider than the property"
shape notes/riscv-parity-scope.md records twice already, in its `thread_count()` form.

Measured, because the margin is the whole story. On a quiet machine the run ends two frames
*below* its baseline, every time, on all three live-swap tests:

```text
[M72b] baseline at entry=21987, after settling=21987, drift=0
[M72b] before=21987 now=21985 delta=-2
```

Two frames of headroom, and the baseline does not drift on a quiet machine. So any three frames
arriving from an earlier test's in-flight teardown trip it, and a loaded CI runner is exactly where
that happens.

**What is not demonstrated**: which teardown supplied them. Eight `script/cpu-matrix sifive-u54` runs
under four host burners did not reproduce it, and neither did forcing a two-second settle at either
end of the run. The direction is established (frames arrive from outside the run) and the source is
not.

**Do not read the fault beside it as an anomaly.** The CI log shows
`user thread N killed: scause 0xd ... stval 0x0000000003100005` immediately before the panic, and
`0x3100005` is `DEV_VA + 5`. That is the outgoing instance dying on the device it no longer has,
which is the control this whole milestone rests on and which the test asserts on directly.

## What is built beside it

- **State handoff**, to §209 (state handoff is an opaque blob over a granted frame, and it is
  optional): `swapper`'s `ROLE_HANDOFF` carries a component's tally across the swap on a handoff
  page. A replacement that cannot absorb it does not commit. It runs on all three
  architectures. See notes/state-handoff.md, including the one-page limit that `redoxfs_server`
  will hit.
- **A component manifest**: the operator's endowments are not literals in its own source.
  `hot_swap_contract` carries the capability half of its own contract, `swapper` wires every component
  from a declaration, and the slot agreement is a compile-time derivation. See
  notes/component-manifest.md, including the honest limit: a manifest is compiled in rather than
  shipped beside a binary, which is a wire format and so a decision left to the architect.
- **Dependency-aware orchestration**: `capability_plan::depends_on` names which contracts a
  component cannot silently tolerate the absence of, and `dependents` answers who must be warned
  before a given contract is swapped. `queued()`'s `BOP_DOWN`/`BOP_UP` are driven by that answer
  rather than sent unconditionally. Direct dependents only; see notes/dependency-orchestration.md.
- **A hung component**: `swapper`'s `ROLE_HUNG` runs the swap against an incumbent that stops
  answering, and notices because its bounded `OPERATION_QUIESCE` goes unanswered. It gets the
  service back with no authority the operator did not already hold, because the step that needed the
  incumbent's cooperation is the step a hang makes redundant. It then destroys the hung instance's
  region, which it kept at build. Milestone 133 (ending a permanently blocked thread, and deciding
  who may) finishes the instance in place, and its stranded caller gets `Gone` (milestone 254 (a
  caller stranded by a server that died)).
- **A dependent that will not answer its warning**: skipping the warning costs latency and loses
  nothing (`ROLE_UNWARNED`), and the warning is advisory, per DECISIONS §231 (a swap's warning to a
  dependent is advisory, and the supervisor never waits for it). See
  notes/non-cooperative-fallback.md.

## What this does not yet demonstrate

- A hung component a supervisor can *see*. The domain reports the hang as `BLOCKED`, which is what a
  healthy idle server reports as, and `Rendezvous::REAP` answers `StillAlive` for every member. The
  operator notices a hang only at a swap, when its quiesce goes unanswered within a bound, and then
  destroys the hung component's region, which answers its stranded caller `Gone`. Nothing watches a
  component between swaps. See notes/hung-component.md.
- The whole console. The component swapped here owns the real UART and is shaped like a console
  server. `line_editor` is swapped live by `terminal_supervisor`; `display_terminal` and
  `compositor` are not, and each is a different problem. See notes/interactive-stack-swap.md.

### BUGS: one unexplained riscv64 failure under load

2026-10-07 (UTC), round 9 of milestone 139 (drive the unsafe count down): `a_client_of_the_stable_rendezvous_cannot_become_its_server`
failed on riscv64 at stage 27 (the operator's reap) with the host load average at 104 on 8 cores.
It passed 5 of 5 alone. Cause unknown.

## See also

- [live-replacement/the-operator.md](live-replacement/the-operator.md): the operator's design, role by role
- DECISIONS §41 (the endpoint is the broker), §12 (a one-shot reply capability), §13 and §16 (revocation),
  §26 (the fault endpoint), §31 (the C seam), §32 (a supervisor may collect a corpse)
- notes/component-manifest.md, notes/hung-component.md, notes/dependency-orchestration.md and
  notes/state-handoff.md for the residuals that have landed
- notes/ipc-naming.md, notes/supervision.md, notes/object-revocation.md, notes/c-seam.md
- notes/benchmarks.md for `broker_rtt` and what the default rung costs
