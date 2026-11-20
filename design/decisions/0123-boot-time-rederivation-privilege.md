---
status: AMENDED
raised: 2026-08-24
decided: 2026-08-24
ratified_by: calef
---

# 123. Boot-time re-derivation: what grants the privilege, and how it dies after one use

*Amended 2026-09-27: the re-deriver is `login`. See the end.*

calef, 2026-08-24, ratified option (a): a boot-only process holding a construction budget and read
access to the schedule store. It re-derives every durable session the store names, then deletes
both capabilities from its own cspace in `root_supervisor`'s exact shape. The four hardening
refinements below are to be built into it, not conditions on the ratification. Raised from milestone
152's own BUGS section (durable delegation), which named this exact gap: "a
privileged, boot-only operation... [but] what object grants that privilege, and how it is scoped so
it cannot be invoked again after boot, is not worked out."

## The question

Milestone 152 decided that boot-time bring-up of a durable session must be re-derivation, not
restoration. Capabilities do not survive a reboot, so the kernel cannot "reload" a session's old
authority. It can only rebuild it fresh, the way a login would, but with no live person presenting
credentials at that moment. The roadmap doc says this should be "a privileged, boot-only operation,
in the same shape as `root_supervisor` handing out its authority once at boot and never again, not a
standing 'impersonate any user' capability left lying around afterward." It does not say what object
holds that privilege, what it is granted, or what mechanism actually prevents it from being invoked a
second time. That is what this decision answers.

This is worth being careful about because of what the operation actually is. It can stand up a
durable session for any user named in the on-disk schedule store. That session is a bundle of
authority equivalent to a live login. It does so without that user presenting a credential. If this
capability persists anywhere reachable after boot finishes, it is a master key. The whole design has
to survive the question "what stops this from being invoked again at 3pm."

## What else was considered

**(a) `root_supervisor`'s own shape, applied to session re-derivation instead of server construction.**
A dedicated boot-only process holds exactly the authority needed to re-derive durable sessions from
the on-disk schedule store. It uses that authority once during bring-up. It then deletes every
capability that authority is made of, including its own copy of whatever names the store. After that
its cspace holds nothing that can re-derive a session. `root_supervisor` proves the same at the end
of its own run: it holds nothing that can `RETYPE` or `RETYPE_OBJ`
(`components/src/root_supervisor.rs` lines 168-182). This is the recommended option below.

**(b) A persistent "master" capability retained by the kernel or an init process for the whole boot's
life, gated by a runtime check (e.g. "boot phase == early").** Rejected. This is weaker on this
tree's own terms (AGENTS.md's ladder, "the ladder, strongest first"). A runtime flag checked before
honoring the capability is rung 2, "a gate that fails loudly," not rung 1, "make the wrong state
unrepresentable." A capability that still exists, gated by a boolean, is one stray code path, one
missed check, or one future refactor away from being invoked outside its window. That window is the
one the boolean was meant to enforce. Capability deletion has no equivalent failure mode. There is
no code path that can re-invoke a capability that has been `cap_delete`d, because the slot is empty.
The flag approach also adds new kernel-visible state (a boot-phase indicator something can read or,
worse, race). `root_supervisor`'s shape needs zero of it. This is exactly the "make the wrong state
unrepresentable vs. a gate that fails loudly" tradeoff AGENTS.md's ladder names, and the ladder's
own ordering answers it.

Option (c), a kernel-enforced single-use bit, is in
[`single-use-bit.md`](0123-boot-time-rederivation-privilege/single-use-bit.md).

Option (d), triggering re-derivation lazily, is in
[`lazy-trigger.md`](0123-boot-time-rederivation-privilege/lazy-trigger.md).

## What this tree already does in the analogous case

`root_supervisor.rs` is exactly this shape, already built and running. It holds `ROOT_UT` (a
construction budget) and a report endpoint, and builds two child servers from that budget. It
delegates the wiring capabilities to them, then calls `cap_delete` on every one of its own copies
including `ROOT_UT` itself. It then proves the deletion worked by invoking `RETYPE` and `RETYPE_OBJ`
on the now-deleted `ROOT_UT` and asserting both fail with `NoSuchSlot` (`invoke(ROOT_UT, abi::untyped::RETYPE, ...)`
returning a negative code, lines 174-176). It reports that proof over its own `REPORT` endpoint
rather than merely asserting it in a comment. This is not a one-off. `cap_delete`-after-use is an
established idiom across the tree, not invented for this decision. `login.rs`'s `mint()` deletes its
own copy of `tcb`, `ready`, and `narrow_ep` once a caretaker is stood up and handed its narrowed
authority (lines 597-623). `spawner.rs`, `credentialer.rs`, `swapper.rs`, `builder.rs`,
`timetable.rs`, and `crates/system_initializer` all do the same thing at the point a piece of
authority has been handed off and the holder no longer needs its own copy. The pattern this decision
recommends is not new to the tree. It is the tree's standard answer to "how do you make sure a
capability is never used again," applied to a case (boot) that has not needed it before.

## What is prior art outside the tree

UEFI's `ExitBootServices` is the same shape at the firmware layer. The boot-time services table
(disk and console access, memory allocation) is explicitly and irrevocably torn down once control
passes to the OS. Calling into it afterward is undefined by spec, not merely discouraged.
Measured/secure boot chains generally follow the same rule. A privilege is exercised once during a
trusted bring-up window and is never reachable again, rather than being a standing credential
checked at every use. Neither is read in depth here. The in-tree precedent (`root_supervisor`) is
doing the real work of this decision. This section is included only because AGENTS.md's six
questions ask for it explicitly.

## Is the premise true?

Checked rather than assumed. A repo-wide grep for `impersonate` finds exactly two hits outside this
milestone's own roadmap doc, both unrelated. One is a comment about badge-less capabilities in
`design/decisions/0101-notification-objects.md`. The other is a comment about NVMe completion-tag
wraparound in `kernel/src/non_volatile_memory_express.rs`. A grep for
`re-derivation`/`rederivation`/`boot-only` finds only this milestone's own roadmap doc and unrelated
prose in other notes (`notes/x86-port.md`, `notes/arch-audit.md`, `notes/frames.md`, none describing
session re-derivation). There is no existing boot-time re-derivation mechanism, no "impersonate"
capability, and no session-revival code path anywhere in the tree today. This is genuinely unbuilt,
exactly as milestone 152's BUGS section says.

The sibling on-disk schedule store this operation would read from also does not exist yet. There is
no `design/decisions/0122-*` file in this tree as of this writing, and no per-user schedule
persistence format anywhere in `crates/` or `user/src/`. That dependency is real and is called out
explicitly below rather than guessed at.

## What each option costs

The recommended shape (a), concretely:

- **What it needs granted.** A construction budget and read access to the on-disk schedule store
  from milestone 152's second piece. The budget is an `Untyped`, sized for however many durable
  sessions the store names, the same kind of budget `login.rs`'s `mint()` spends to build a
  caretaker. That second piece is a sibling decision (the store's format, write path, and
  read-at-boot path) that may not land before this one. This decision does not depend on knowing its
  shape, only on there being *some* read capability to grant. That is exactly how `login.rs`'s
  caretaker is handed a narrowed file-service endpoint without this decision needing to know the
  file service's internals.
- **What it must NOT be granted.** Network access (nothing about re-deriving local sessions needs
  it). The ability to re-derive on demand after the boot window closes. That is the entire scoping
  question and is answered below, not by omission here. Anything that would let it enumerate users
  rather than iterate the store it was hard-wired to read at construction. This is milestone 126
  (the `procps` package)'s enumeration-is-authority rule, already invoked by 152's own reattachment
  design for the same reason.
- Whether it needs a new dedicated process or folds into an existing one. Either is viable and this
  decision does not need to pick. One shape is a small dedicated boot-only process (provisional name
  only, something like `session_reviver`; naming is calef's call per AGENTS.md, not this lane's). It
  runs once between `system_initializer`'s early boot and normal service startup. The other is a
  phase folded into `root_supervisor` or `system_initializer` itself, given the store-read
  capability and the construction budget as part of its own boot endowment. The cost difference
  between "new process" and "new phase in an existing one" is a few hundred lines either way and is
  not the load-bearing question. The load-bearing question is what happens to the capability once
  re-derivation finishes, which is the same in both cases.
- **The scoping mechanism itself, and why it is not merely "happens not to be invoked again."** The
  process (whichever one holds this) re-derives every session the store names. It then calls
  `cap_delete` on the store-read capability and the construction budget, in the same breath
  `root_supervisor` deletes `ROOT_UT`. After that, its own cspace has no slot that names either
  capability. There is not a flag saying "do not use this," not a check that runs before use, but an
  empty slot. `NoSuchSlot` is what the kernel returns to *any* invocation attempt against a deleted
  capability, unconditionally. That is because the capability table is what the kernel consults and
  there is nothing in it. There is no runtime check to bypass, no phase variable to race, and no way
  for a later bug in this same process (or a compromise of it) to reconstruct the deleted
  capability. The reason is that nothing else in the system was ever the store-read capability's
  owner. Therefore nothing else can re-derive or re-grant it. This is the same proof shape
  `root_supervisor` already performs and reports. It attempts the operations the deleted capability
  would have permitted, over its own proof-reporting endpoint, and asserts they now fail. A
  re-deriver could do the same, attempting a further store read or a further session build after its
  deletion pass and reporting the failure. That would make the "cannot be invoked again" claim a
  demonstrated fact rather than an assertion, exactly what this decision is closing.

Cost of option (b), the runtime-flag alternative, measured against this: it needs a new piece of
kernel-visible state (a boot-phase indicator) that does not exist today. It also needs a check
inserted at whatever invocation point the master capability's method dispatches through. And it
needs an argument for why that check cannot be skipped, raced, or reached through a path someone
adds later without noticing it is security-relevant. None of that is quantifiable in lines of code
today because no such flag exists to measure. The point is qualitative and it is the one the ladder
already settles. An added check is weaker than an absent capability, full stop, regardless of how
few lines the check would be.

## How reversible is this, and who has already acted on it

Nobody has acted on this yet. Milestone 152's design is worked out but "nothing here is built" (the
roadmap doc's own header). This decision is about a mechanism, not a fact that has left the machine.
It is cheap to change: no code exists yet that depends on the answer, no wire format, no persisted
secret.

But the shape recommended here (a) needs zero new kernel primitives. `cap_delete`, `Untyped::SPLIT`,
and ordinary capability derivation are all §16 (object revocation)'s existing mechanism. Nothing
about re-deriving a session at boot requires a new system call, a new object type, or a change to the
capability model. That is a strong point in its favor independent of the scoping argument above. It
is buildable today, on the mechanism this tree already has and has already proven correct (§16's
amendments, and the Kani harness pinning `SPLIT`'s rights-inheritance invariant). It does not need a
new primitive designed, proved, and added to the system call surface first. Option (b) or (c), by
contrast, would each need exactly that: (b) a new kind of kernel-checked runtime state, (c) a new
capability method. Both are the expensive kind of decision under AGENTS.md's "move fast on what can
be undone" tenet (the system call surface is named there explicitly as a boundary, not a habit). So even
setting the scoping argument aside, (a) is the only option of the three that does not itself require
a further, slower decision before it could be built.

## The recommendation

Option (a): a boot-only process holding a construction budget and read access to the schedule store.
It re-derives every durable session the store names. Then it deletes both capabilities from its own
cspace, in `root_supervisor`'s exact shape. The scoping mechanism is local capability deletion,
proven the same way `root_supervisor` already proves it: by attempting the now-forbidden operation
afterward and reporting the failure. Whether this lives as a new dedicated process (provisional
name, calef's to ratify) or a phase inside `root_supervisor`/`system_initializer` is a smaller, more
reversible question. This decision does not need to settle it. Either shape gets the same scoping
property, which is the part that matters.

## Hardening the window, amended 2026-08-24

calef asked, after reviewing the recommendation above, whether the residual risk in option (a) can
be narrowed further. The risk is this window: the re-deriver holds the store-read capability and the
construction budget until its own `cap_delete` pass runs. For that long, a compromise of that
process grants exactly the "impersonate any user in the store" power this decision exists to
prevent. None of what follows eliminates that window. No real system does, when the capability must
exist at all, even momentarily. `root_supervisor` accepts the identical shape, and so does UEFI's
`ExitBootServices` (cited above). These are refinements to build *into* option (a), narrowing the
window and its blast radius. They are not a different option and not a claim that the risk goes to
zero.

**1. Shrink the window per-identity rather than per-boot, using `Untyped::SPLIT`.** This decision's own
"How reversible is this" section already notes that `SPLIT` is existing, proven mechanism (DECISIONS
§16) that option (a) needs zero new primitives to use. Do not hold the undivided store-read
capability across the whole re-derivation pass and delete it once at the end. Instead, split off
only what one identity's re-derivation needs, derive that session, and move to the next. A mid-run
compromise's blast radius shrinks to "identities not yet processed" rather than the whole store, as
re-derivation proceeds.

**2. Gate the re-deriver's own binary through measured boot before it is ever granted the capability.**
`crates/system_initializer` already performs exactly this check for every program it loads. It reads
the program from the archive and calls `measured_boot::verify_in_manifest`. It refuses to hand back
an ELF on either `Unmeasured` (the table says nothing about this name) or `Mismatch` (milestone
104's discipline). The same check, applied to the re-deriver before it is ever handed the store-read
capability, is real defense against a tampered binary holding that power. It does nothing against a
logic bug in the correct, unmodified code while it runs. Such a bug stays the harder half of the
residual risk and is not addressed by any of these four refinements.

**3. Keep the process's own code surface minimal, as a stated design property rather than an incidental
one.** The smaller and simpler the re-deriver is, the lower the odds a bug exists in the window at all.
This is already implicit in "a small dedicated boot-only process" elsewhere in this file. It is
stated here so whoever builds it treats minimality as a requirement rather than something that
happened to be true of a first draft.

**4. A liveness watchdog for the failure mode this file does not otherwise cover.** Everything above
addresses *misuse* of the capability during the window. A distinct, non-malicious failure mode is
the re-deriver hanging or crashing *before* its deletion pass runs. That leaves the capability live
past its expected window by accident. Something else needs to notice this and act, whether that is
killing the stuck process or a fallback revocation path reached some other way. Named here, not
designed. This file's own discipline elsewhere is to record an open sub-question rather than guess
at its mechanism, and this is one of them.

None of these four change the recommendation. Option (a) remains the right base shape. Neither
option (b) nor option (c) removes this residual-window risk, while both add real cost of their own
that this file's existing analysis already prices. Those costs are (b) a new kind of kernel-checked
runtime state and (c) a new capability method on the system call surface. These four are refinements to
build into option (a) once someone picks it up. They are recorded now so the eventual builder
inherits them rather than rediscovering them.

## What this does NOT decide

- The schedule store's on-disk format, write path, or read-at-boot path: milestone 152's second
  piece. This decision assumes only that some read capability over that store exists to grant.
- Credential revocation's own mechanism, or its consequence: settled by
  §108 (disabling a user's login credentials kills their durable session), in
  [its file](0108-credential-revocation-kills-durable-session.md). It tears a session down; this
  stands one back up.

## Amended 2026-09-27: `login` is the re-deriver

calef ruled Fork 7 of milestone 152 (durable delegation) on 2026-09-27 (UTC), option A. At start-up,
`login` re-derives every durable session that is not suspended, with the code that restores a stored
schedule at login. `session_reviver` is retired. The analysis is in
[the fork 7 appendix](../../notes/durable-delegation/boot-rederivation-in-login.md).

Option (a)'s deletion property no longer applies, and it protected nothing. `login` holds every
capability the re-deriver held, with equal or wider rights, for its whole life (the appendix's
question 4). What stops a 3pm invocation is `login`'s code. The pass runs once, before its receive
loop, and a re-derived session goes only to a login the credential service authenticates. Of the
four refinements, the first is moot, the second holds as it does for `login`, and the third is met
by reuse. The fourth stays undesigned, and each session's wait is bounded by `END_WAIT_SECS`.
