# Dependency-aware orchestration

*Milestone 23's fourth residual, and the last one this milestone names: "if component B is a client
of component A, swapping A means quiesce B, swap, resume." `crates/capability_plan`'s `depends_on`,
`LiveInstance` and `dependents`; `components/src/swapper.rs`'s `queued()`. Read notes/live-replacement.md
and notes/component-manifest.md first if you have not; this builds on both.*

## The defect, in the roadmap's own words

> Dependency-aware orchestration. If B is a client of A, swapping A means quiesce B, swap, resume;
> the supervisor needs the dependency graph and a quiescence protocol.

The quiescence protocol already existed before this lane, in one instance: `broker`'s
`BOP_DOWN`/`BOP_UP` (notes/live-replacement.md's latency ladder). What did not exist was the graph.
`queued()` sent `BOP_DOWN` unconditionally, because in the one system this tree runs there is exactly
one component (`broker`) that ever needs it, so "always warn it" and "warn whoever the graph names"
produced the same four syscalls and nothing distinguished the two. The manifest itself said nothing
about it: `capability_plan::Requirements` declared what a component *needs*, never that another
component *supplies* it, so there was no dependency graph to compute an answer from even in
principle.

## Why the graph cannot live where the roadmap's own wording suggests

The natural first reading is "a `CapNeed` should say which contract satisfies it." That does not
survive contact with a manifest already in this tree. `hot_swap_contract::CLIENT` declares one `Use`
need named `service`. The supervisor routes that name to a console (or backend) instance on the
direct channel and to `broker`'s front endpoint on the queued one, per
`notes/component-manifest.md`'s own rule (the name is the component's and the object is the
supervisor's). A `CapNeed` that named its supplier would be wrong on one of the two wirings, every
time, by construction. Role resolution is the supervisor's business and a capability-level
dependency claim cannot be, on pain of contradicting the manifest mechanism's own reason for
existing.

So the graph lives one level up, on `Requirements` itself, as a set of contract names rather than
role names: `depends_on: &'static [&'static str]`. This is coarser than "which capability," and the
coarseness is deliberate. It answers "which other running components can I not silently tolerate the
absence of," independent of which role name happens to route to which object in any particular
wiring.

## The rule that resolves the CLIENT ambiguity: only server-shaped dependents need telling

A pure consumer, no matter which contract answers its `service` need, never needs an entry in
`depends_on`. This is not a simplification; it is DECISIONS §41 applied a second time. A `CALL` that
finds nobody receiving parks on the endpoint's own sender queue, and the *next* server to `RECEIVE_CAP`
drains it, in order, with nothing lost. That is exactly the mechanism that made the roadmap's
imagined forwarding broker unnecessary in the simple case (notes/live-replacement.md's "three things
the build settled"). It generalizes for free: any component whose only relationship to a dependency
is calling through it and blocking has nothing that needs warning, because blocking is already the
correct, lossless behavior.

What does need warning is a component that would otherwise stop serving its own clients while its
dependency is down, because it cannot afford to let its one serving thread sit inside a blocked
`CALL`. `broker` is exactly this: single-threaded, pass-through, and it would stop answering
producers for the whole down window if it just called through and blocked. That is the edge
`depends_on` exists to name, and it explains why `hot_swap_contract::CLIENT.depends_on` is empty while
`hot_swap_contract::BROKER.depends_on` is `&["backend"]`.

The falsifiable rule, stated once so a future contract author can apply it without re-deriving it:

> **Populate `depends_on` only for a component that itself serves others while forwarding
> synchronously to the contract being named.** A pure consumer's `depends_on` is always empty.

## What `dependents` computes, and what it deliberately does not

`capability_plan::dependents(target_contract, live)` takes the running system as a caller-supplied
list of `LiveInstance { id, reqs }` and returns, in the order given, the ids whose `Requirements`
name `target_contract` in their own `depends_on`. An instance of the target contract is excluded from
its own answer even if a malformed declaration named itself.

Direct dependents only, and no attempt at transitivity. A three-hop chain (`C` depends on `B`
depends on `A`) is not walked to find `C` when `A` is the swap target. Whether `C` needs telling
depends on whether `B`'s own quiescence protocol keeps `B` available to `C` throughout. That is a
property of `B`'s implementation this crate has no way to see from two contract names. Guessing
wrong in either direction is a real cost: telling `C` when it did not need it is unnecessary
latency, and not telling it when it did is the failure this whole residual exists to prevent.
Recorded as `BUGS` rather than built speculatively.

## What runs under test

`queued()` builds the two-instance live registry `[(1, BACKEND), (2, BROKER)]`, calls
`dependents("backend", &live)`, and reports the verdict (`RPT_DEPENDENTS`) before acting on it. What
changed from before this lane: `BOP_DOWN` is no longer unconditional. The operator loops over the
graph's own answer and sends it only to the ids the graph named, and `BOP_UP` is sent in the reverse
of that same order on the way back up. In this tree's one queued system that is still exactly one
message each way, because there is exactly one component with a `depends_on` edge to `backend`. The
behavior is unchanged; what changed is which code decided it.

The direct channel gets the negative case for the same reason `capability_plan`'s own test suite
pairs every positive assertion with one. `dependents("console", &[(1, CONSOLE)])` must return empty,
because nothing in that system declares a dependency on `console`. A mechanism that only ever
returns something has not been shown to refuse correctly.

Both are asserted in `system_tests/src/user/live_swap_tests.rs` via `RPT_DEPENDENTS`, on both architectures.

## EXAMPLES

Read what a contract's dependency looks like, without running anything.

```sh
grep -A12 'pub const BROKER' crates/hot_swap_contract/src/lib.rs
```

Ask who must be told before a contract is swapped, given the components a supervisor is
currently running:

```rust
use capability_plan::{dependents, LiveInstance};

let live = [
    LiveInstance { id: 1, reqs: &hot_swap_contract::BACKEND },
    LiveInstance { id: 2, reqs: &hot_swap_contract::BROKER },
];
let order = dependents("backend", &live)?;
for id in order.quiesce_order() {
    // tell each one to stop forwarding, in whatever way its own contract defines
}
// swap backend
for id in order.quiesce_order().iter().rev() {
    // tell each one to resume, in the reverse order they were warned
}
```

**Add a dependency to a new contract.** One field, at the declaration site, no supervisor change:

```rust
pub const CACHE: Requirements = Requirements {
    contract: "cache",
    caps: &[
        CapNeed { role: "requests", direction: Direction::Serve },
        CapNeed { role: "origin", direction: Direction::Use },
    ],
    maps: &[],
    pages: 32,
    // cache forwards misses to `origin` synchronously and would stop serving its own clients
    // while `origin` is being swapped unless told to switch into a degraded mode first.
    depends_on: &["origin"],
};
```

## BUGS

**No transitive orchestration.** `dependents` finds direct dependents only, and the reason is not an
oversight but the finding stated above. Whether a dependent's own dependents need telling in turn is
a property of that dependent's own decoupling mechanism, which two contract names cannot express. A
supervisor with a chain deeper than one hop must currently walk it by hand, calling `dependents`
again from each dependent found, and deciding case by case whether to stop.

**No quiescence protocol is generalized.** `dependents` says *who*; it says nothing about *how* a
told component is supposed to degrade. `broker`'s `BOP_DOWN`/`BOP_UP` remains a protocol specific to
one contract, and a supervisor orchestrating a system with several different forwarding shapes would
need to know each one's own verbs. A uniform "prepare to lose your dependency" / "resume" pair of
verbs, if this tree ever needs one, is a new decision (a wire contract two or more programs would
have to agree on, AGENTS.md's expensive category) and not something this lane decided on nobody's
behalf.

**No non-cooperative fallback.** notes/hung-component.md's own finding applies here directly: the one
step a dependency-aware supervisor needs from a dependent (telling it to degrade) is exactly the step
a hang makes unavailable. If `broker` itself stopped answering without dying, this mechanism has
nothing to say about restoring service through it: `broker` is not `Rendezvous::REAP`-collectable any
more than the console component was, and nothing here builds the fallback notes/hung-component.md
already declined to build. This is not new work avoided; it is the same open decision (a timed wait,
and what a supervisor may do about a component that never cooperates) inherited rather than
re-decided.

**`depends_on` is unchecked for self-reference and cycles at the declaration level.**
`Requirements::problem()` does not look at `depends_on` at all. A contract that named itself would be
caught at the query site (`dependents` excludes the target instance from its own answer, by
`contract`, unconditionally). But a two-contract cycle (`A` depends on `B`, `B` depends on `A`)
would compile. It would produce a `dependents` answer for each target that is technically correct
and operationally useless. A supervisor swapping `A` would be told to warn `B`. Warning `B` (a
contract that itself depends on `A`) while `A` is mid-swap is not obviously safe, and this crate has
no opinion about it. No contract in this tree declares one, so this was not exercised, and a lane
building a second forwarding contract should check its own graph by hand until this has a mechanism.

**Every name here is provisional**: `depends_on`, `LiveInstance`, `Dependents`, `dependents`,
`quiesce_order`, `OrchestrationRefusal`, `MAX_LIVE`. `dependents` in particular is one word doing two
jobs (a query function and, informally, "orchestration") and may not survive contact with a real
second consumer.

## See also

- DECISIONS §41 (the endpoint is the broker), whose sender-queue argument this residual's whole
  "pure consumers need no telling" rule rests on
- notes/live-replacement.md for the swap itself and the latency ladder `broker` sits on
- notes/component-manifest.md for `Requirements`, `Provisions`, `plan`, and the `use`/`offer` split
  this residual's `depends_on` field extends
- notes/hung-component.md for the non-cooperative case this residual's fallback still owes, and the
  two decisions (a timed wait, and what may be done to an uncooperative component) it is blocked on
- Fuchsia's component topology (`offer`, transitively resolved through a component's ancestors) is
  the nearest prior art for naming a supplier by contract rather than by object.
- Kubernetes' `readinessProbe`/graceful-shutdown ordering and systemd's `Before=`/`After=` unit
  ordering are the general shape of "tell a dependent before you take its dependency away." They
  were checked rather than copied: neither is capability-routed and neither has this tree's
  single-hop scope limit
