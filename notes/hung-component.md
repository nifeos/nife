# The hung component

*Milestone 23's third residual, and the one §32 named and declined: a component that stops answering
without dying. The swap itself is DECISIONS §41 and notes/live-replacement.md. `crates/hot_swap_contract`, `components/src/swapper.rs`'s
`ROLE_HUNG`, and `a_supervisor_notices_a_hung_component_and_reclaims_it` in
`system_tests/src/user/live_swap_tests.rs`.*

Case (c) and question 3's stranded caller are partly resolved in the kernel. Milestone 133 (ending
a permanently blocked thread, and deciding who may), ruled by calef on 2026-09-03, makes the region
holder's `DESTROY` finish a `Blocked` resident in place. Milestone 254 (a caller stranded by a
server that died) wakes its stranded callers with `abi::Error::Gone`.
Only a holder of the supervision endpoint alone still cannot end a child (133's refused proposal B).
The hung test runs the two together: the operator destroys the hung incumbent's region, and the
caller it stranded gets `Gone`. notes/blocked-thread-teardown.md has the decision.

## The problem

Every failure this system handles is a death. A component faults or exits, and the kernel is the
witness. It stamps a five-word message onto the supervision endpoint its spawner designated
(DECISIONS §26). The supervisor reads it, calls `Rendezvous::REAP` with the tid the kernel put in the
message (§32), and the region comes home to the builder. That chain is complete, proven on both
architectures, and it is the whole of what a supervisor can do.

A component that stops answering without dying produces none of it. It holds its endpoint. It
holds its device. Its region is live and mapped. Its thread is `Blocked` or, if it is spinning,
`Running`. Nothing faults or exits, no message arrives, and every mechanism in this tree reads that as a
healthy server. Meanwhile a client is parked inside a `CALL` that will
not return, and its supervisor is blocked in `RECEIVE` on an endpoint that will never deliver.

DECISIONS §26.1 calls it
"alive but wedged" and says polling "remains the right tool" for it. §32's second consequence names
it exactly and declines it:

> The honest limitation: a supervisor that must restart a *hung* child (livelocked, not crashed,
> so no death message ever arrives) still needs the stronger right. That case is real, it is the
> watchdog case, and it is deliberately not solved here. When milestone 23's live replacement needs
> it, it is a new decision.

Milestone 23's live replacement needs it. This note is what a lane found: which of four questions
the machine answers, which need an architect, and where §32's sentence above is half wrong.

## Three hangs, not one, and they have different answers

The three have different detectability and different terminal states, and treating them as one is
what makes "add a watchdog" sound like a single task.

| shape | what `SURVEY` sees | can the kernel tear it down today? |
|---|---|---|
| (a) livelocked, spinning in userspace | `READY` / `RUNNING`, always | yes. `MemoryRegion::DESTROY` arms §16's kill and the scheduler converts the thread to a corpse at its next preemption |
| (b) blocked on an endpoint whose region the supervisor can destroy | `BLOCKED` | yes, with collateral. Destroying that region drains the endpoint's wait queues, aborts the blocked IPC with `abi::Error::Gone`, and the armed kill then lands |
| (c) blocked on an endpoint the supervisor cannot reach | `BLOCKED` | no, when written. Yes since 2026-09-04: the region holder's `DESTROY` finishes it in place (milestone 133) |

Case (c) is in `reap_region_objects`'s comment of 2026-08-16, and its mechanism is the
load-bearing fact for question 4: `MemoryRegion::DESTROY` arms a kill that is spent
only in `schedule()`, for a `Running` thread, so a thread `Blocked` forever never spends it. That is
why the "stronger right" §32 points at was, when written, insufficient for case (c). The comment and
the mechanism step by step are in [`hung-component/the-armed-kill.md`](hung-component/the-armed-kill.md).

## The four questions

### 1. Who notices, and with what authority?

A watchdog holds `ENUMERATE` and nothing else, and there is no hole in the names-not-acts rule
because noticing and acting are separated into two processes rather than two rights in one.

`abi::endpoint::SURVEY` (milestone 126) is the whole view: a cursor walk over the supervision
subtree, returning `(tid, state)` per member, gated on `Rights::ENUMERATE` and pointedly not `READ`,
because `READ` is what `RECEIVE` and `REAP` take. calef's ruling on 2026-08-17 is that a domain names
its members and does not act on them.

A watchdog looks like the first thing that needs both halves, and it is not: it does not need to
act, it needs to be believed. It produces a verdict about liveness, and what acts on that verdict is
the party already holding the authority, the *supervisor* (`READ` on the supervision endpoint, so
it may reap) or the *builder* (a region capability, so it may destroy). Both of those relationships
exist and neither wants widening.

So the shape is three parties and no new authority anywhere:

```text
    watchdog  ──ENUMERATE──►  the domain          (may look; cannot reap, cannot kill, cannot serve)
        │
        │ a verdict, over an ordinary endpoint it holds WRITE on
        ▼
    supervisor ──READ──────►  the domain          (may reap a corpse; §32)
        │
        │ (for the terminal case only)
        ▼
    builder   ──region cap─►  the hung component  (may destroy; §16, §24)
```

That arrangement is better than a watchdog that can kill. A compromised watcher can only lie about
liveness, and the supervisor that acts can refuse the lie. The provisional name for such a program
is `liveness_watch`, an architect's call; nothing was minted and no such program was built. The
three properties and the naming reasons are in
[`hung-component/liveness-watch.md`](hung-component/liveness-watch.md).

### 2. What counts as hung?

At a swap, a quiesce unanswered within the supervisor's bound. `swapper`'s operator asks for the
drain with a deadline (milestone 106 (a wait that ends on either the interrupt or the deadline)'s
timed wait) and treats no answer as a hang. A `CALL` takes no deadline, so a stand-in makes the call
and the deadline is on its forwarded answer. The bound is `QUIESCE_BOUND_MS`, 250 ms, about forty
times the slowest healthy answer measured on three ISAs. `terminal_supervisor` does the same for
`line_editor`, with a stand-in built from its own image and its own `QUIESCE_BOUND_MS`, measured
against `line_editor`'s answer (`line_editor::component::supervisor`).

The bound belongs to the supervisor, not to the manifest and not to the client. A contract cannot
know the machine, and a client that is parked inside its `CALL` cannot act. It is denominated in
time, which can convict a healthy component of being slow. It is acceptable here because the
quiesce is owed work of known size, waiting behind only the requests already queued, and because it
is asked only by a supervisor about to replace the component anyway.

Outside a swap the better denomination is progress: a monotone counter in a page the supervisor
owns (§41 already has `hot_swap_contract::LOG_VA`), and a verdict of *work is owed and the counter
has not moved across k observations*. That is milestone 62 (tests)'s argument one level out, *"a
bound expressed in something other than the property under test."* It is not built.

The argument owner by owner, the progress counter's costs, and where the built bound departs from
it are in [`hung-component/what-counts-as-hung.md`](hung-component/what-counts-as-hung.md).

### 3. What happens to the client that is already blocked?

Nothing but the server can answer a caller whose server hangs mid-`CALL`: the one-shot `Reply`
capability is minted without `GRANT` and lives in the hung component's table, so the operator cannot
answer for it. What the operator can do is end the server. Destroying the hung component's region
finishes it in place (milestone 133), and the reclaim answers every caller it held a `Reply` for
with `abi::Error::Gone` (milestone 254). The hung test runs exactly that, and the client asks the
replacement again. A server that is hung and alive, and that nobody ends, still strands its caller.

The path through the code, the operator's position and the test's evidence are in
[`hung-component/stranded-caller.md`](hung-component/stranded-caller.md).

### 4. Is killing even the right response?

Killing is neither necessary nor sufficient.

Not necessary, for restoring the service. This is where §32's sentence is half wrong, and the
test is the argument. Milestone 23's four steps against a hung incumbent:

```text
  1 BUILT    unchanged. Lay the replacement out, endow it with everything but the device, do not
             configure or start it.
  2 DRAINED  UNAVAILABLE, and unnecessary. OPERATION_QUIESCE needs the incumbent to answer, which is the
             one thing it does not do, and its going unanswered within the bound is how the hang is
             noticed. Quiescing exists to make the incumbent stop receiving, and a hung component
             has already stopped. **The step that needs its cooperation is the step the hang makes
             redundant.**
  3 REVOKED  unchanged. DeviceFrame::REVOKE take-back (§41) is GRANT-gated on the operator's own device
             capability and asks the holder for nothing. It works on a live, wedged, wholly
             uncooperative holder exactly as on a quiesced one.
  4 STARTED  unchanged. The replacement parks in RECEIVE_CAPABILITY on the stable rendezvous and picks up
             whatever queued behind the silence, because the stable name is the rendezvous object and
             the kernel's sender queue is the buffer (§41).
  5 REAPED   UNAVAILABLE. There is no corpse. Rendezvous::REAP answers StillAlive. In its place,
             DESTROY on the incumbent's region, which the operator kept at build: it finishes the
             incumbent where it sits (milestone 133) and answers its stranded caller Gone.
```

So a supervisor recovers the service with no authority it did not already hold, and §32's
`Rendezvous::REAP` was never in the path. What §32 is right about is step 5: reclaiming a hung
component's memory needs more than the supervision endpoint, namely the region capability, which a
supervisor that builds its children holds if it keeps it. The sentence conflates restarting a
service with reclaiming a region.

Reclaiming matters for capacity, not tidiness: without it every hang costs a region for the life of
the machine, and a supervisor survives as many hangs as it has spare budget. With it, the
replacement's region is spoken for only until the `DESTROY` returns the old one.

Restarting is what the machinery already does. §40's rule is that there is no reaper of last
resort, and live replacement is a restart with a stable name in front of it, so a hung component's
replacement is the four steps minus the one that needed cooperation. That response is built.

## What is built, and what is not

Built: the case, on all three ISAs. `swapper`'s `ROLE_HUNG` (`ROLE_HUNG_WITHOUT_DEVICE` on `x86_64`)
runs the direct channel's system against an incumbent that swallows one request and stops answering,
silently. The test asserts six results. The operator notices by its bounded quiesce, the domain
reads as idle, and every reap is refused. The service is restored with no new authority, the
incumbent's region comes back while it is still hung, and its stranded caller sees `Gone`. The
results, the silent wedge and the quiesce's stand-in are in
[`hung-component/the-wedge-test.md`](hung-component/the-wedge-test.md).

Not built: a watchdog program. Detection exists only inside a swap, where the supervisor's own
quiesce is the question; a watch over a running component wants the progress bound of question 2
and the `liveness_watch` shape of question 1, an architect's name.

Not built: any kernel-surface change. No new syscall, method or rights bit, and no change to
`SURVEY` or `REAP`. The obvious extension is a decision: see `BUGS`.

## EXAMPLES

Read what a supervisor can see about a hung child. The whole view, and its limit, in one
declaration:

```sh
grep -B4 -A30 'pub const SURVEY' crates/abi/src/lib.rs
```

Run the case on one architecture, or all three.

```sh
script/test --arch riscv64 --test a_supervisor_notices_a_hung_component
script/test 2>&1 | grep -i 'notices_a_hung'
```

Watch a domain from a program that cannot touch it, endow that program with one right, and restore a
service under a hung component: the Rust for each, with `ps`'s precedent quoted from
`crates/system_initializer`, is in [`hung-component/examples.md`](hung-component/examples.md).

## BUGS

**Detection exists only at a swap.** The operator notices a hang when its own quiesce goes
unanswered within `QUIESCE_BOUND_MS`. Nothing watches a component between swaps.

**`SURVEY` does not say what a thread is blocked *on*, so no wait-for graph is possible.** It is
the extension that would turn detection from a heuristic into a decision procedure. A cycle in
the "waiting for" graph is a deadlock, provably and with no threshold at all (notes/deadlock.md,
Coffman condition 4, and Postgres's wait-for graph as the prior art). It is also a kernel-surface
change to a method milestone 126's lane landed hours ago, and it is arguably `ENUMERATE`-shaped
rather than `READ`-shaped, so it is a decision and not a task. Recorded here rather than attempted.
The cost of not having it: every liveness verdict in this system is a heuristic with a knob.

~~A hang can cost two unreclaimable regions.~~ Closed 2026-09-04 by milestones 133 and 254 (see
the correction at the top), and struck here 2026-10-08.

**The shared witness page cannot be taken back from a hung holder.** `DeviceFrame::REVOKE` on a
`DeviceFrame` is take-back and spares the invoker; on an ordinary `PageFrame` it is symmetric and takes
the page from the invoker too (§13, §41, and the asymmetry is deliberate). So an operator cannot
un-share a writable page from a component it no longer trusts without losing its own view of it. A
hung component that later wakes can therefore still scribble on the supervisor's witness page. The
test does not hit this because the hung incumbent is destroyed and never runs again. A supervisor
that replaces a hung holder without destroying it is exposed. See notes/shared-page-audit.md.

**The queue rung does not protect against a hang, and it looks as though it should.** `broker`'s own
comment is honest about the reason: it "does not probe the backend and cannot: liveness is exactly the
thing a synchronous rendezvous cannot report on". In pass-through it *calls* the backend, so a hung
backend hangs the broker, which then stops receiving on its front endpoint, and producers park on the
kernel's sender queue where nothing can count them. Rung 1 decouples a scheduled absence (the
operator said `BOP_DOWN`) and not an unscheduled one. `QUEUE_FULL` looked like a clock-free "this has
gone on too long" signal and is not reachable in the hung case.

**The three hang shapes are not all tested.** The test exercises case (b)/(c)'s observable half: a
component blocked on an endpoint, indistinguishable from healthy. Case (a), the spinner, is
distinguishable by `SURVEY` alone in principle: a healthy request/reply server is `BLOCKED`
between requests and a spinner is never `BLOCKED`. It is not tested here, because establishing
"never" needs repeated sampling and that is the load-sensitive assertion this lane refused to write.
It is the easiest of the three to detect and the easiest to kill; a lane that gets a bound it may use
should take it first.

**Every name in this residual is provisional.** `wedge`, `WEDGE_SEQ`, `ROLE_HUNG`,
`ROLE_HUNG_WITHOUT_DEVICE`, `ROLE_QUIESCER`, `NOTE_QUIESCE_ANSWERED`, `WAS_GONE`, `RPT_SURVEY`,
`RPT_UNCOLLECTABLE`, `RPT_UNANSWERED`, `RPT_RECLAIMED`, `survey_counts`, `QUIESCE_BOUND_MS`, and the
proposed `liveness_watch`. `wedge` is at least the word this tree already uses for the condition
(§26's "alive but wedged", milestone 62's "a wedged one"). `caretaker` and `undertaker` were
unavailable: both are spent on capability-narrowing programs.

**§32 and §41 both carry a sentence this lane's evidence contradicts**, and a lane may not edit
`design/decisions/`. §32's consequence 2 says a supervisor restarting a hung child "still needs the
stronger right"; §41's last bullet says a livelocked instance "would need the stronger right, which is
§32's recorded watchdog case". Both are right about reclaiming the component's memory and wrong about
restarting its service, and the four-step table above is the correction. The integrator owns the edit.

## What is left of milestone 23

State handoff is still the crux, and a hang makes it harder: a hung component cannot serialize
anything. Dependency-aware orchestration has since been built (notes/dependency-orchestration.md),
and its quiescence step is the one a hang makes unavailable, so each edge still needs a
non-cooperative fallback, which is not built. See
[`hung-component/what-is-left-of-milestone-23.md`](hung-component/what-is-left-of-milestone-23.md).

## See also

- DECISIONS §32 (a supervisor may collect a corpse without being able to build one). Its
  consequence 2 is where this case was named and declined.
- §41 (the endpoint is the broker); §26 (the fault endpoint), whose sub-decision 1 calls this "alive
  but wedged"; §12 (a one-shot reply capability).
- §13 (capability revocation) and §16 (object revocation), where the armed kill lives; §24 (interrupting the foreground process),
  the forcible `^C` tier; §40 (there is no reaper of last resort).
- notes/live-replacement.md for the swap itself and its two witnesses; notes/component-manifest.md for
  why a deadline is not a manifest field; notes/process-view.md for `SURVEY` and `ENUMERATE`
- notes/deadlock.md for the four Coffman conditions and why detection means killing somebody;
  notes/supervision.md, notes/teardown.md, notes/sink-protocol.md (`abi::Error::Gone`)
- design/roadmap/0106-deadline-wait.md for the timed wait the operator's bounded quiesce uses.
- design/roadmap/0062-time-sensitive-tests/the-problem.md for the progress-versus-duration argument this note
  borrows, and 78 for the assertions it is currently removing
- Prior art: MINIX 3's reincarnation server (which does poll, with a per-service timeout an
  administrator sets), QNX's high-availability manager, Erlang/OTP supervisors, and the Linux kernel's
  `hung_task` detector, which reports and never acts, for the same reason recommended here
