---
status: DECIDED
raised: 2026-08-23
decided: 2026-08-23
ratified_by: calef
---

# 114. `pmap` gets its listing: `ENUMERATE` extends to the address-space object

calef, 2026-08-23, approving milestone 126 (who else is running, and who is allowed to ask)'s `pmap` fork on the recommendation
below: *"Yes, pmap first."*

## The question

Milestone 126's gate named one open decision blocking `pmap`. The decision is whether
`Rights::ENUMERATE` extends to the address-space object (`Aspace`, renamed `AddressSpace` by
DECISIONS §113 (eleven kernel object and identifier names)), the same way it already extends to
`Endpoint`/`Rendezvous` for `ps`/`pgrep`'s survey.

## The decision

**Yes.** A new method on the address-space object, gated by `Rights::ENUMERATE`, lists what is
mapped without granting the ability to map or unmap. `pmap` is built against it.

## Why this was not really a live fork

The call site has been waiting for exactly this since the object was created.
`kernel/src/system_call.rs`'s `RETYPE_OBJ` handler for `ASPACE` carries a comment written when the
object was first built: "`Aspace` does not consult `ENUMERATE` today and is expected to when `pmap`
is built." Every address-space object already receives `Rights::ALL` at creation (the same invariant
`Endpoint`/`Rendezvous` uses). So the bit has been present and unused the whole time; nothing is
retrofitted.

The analogous case is already built and proven. `Endpoint::SURVEY` (milestone 126's first stratum,
2026-08-16) gates on `Rights::ENUMERATE` and adds no new system call number. Three Kani harnesses hold
the property that the domain a monitor sees and the domain a supervisor may collect from cannot
diverge. This is the same shape one object type over: read a mapping without being able to change
it.

Cost and reversibility, per the six-questions framework. There is no new right, no new system call
number, and no wire commitment two programs already depend on. A new method on an existing object
type is exactly what CLAUDE.md's system-call-surface rule calls fine to add within the established
capability model. Fully reversible if it turns out wrong.

**One real cost the first pass at this decision missed, caught rereading `notes/process-view.md`
before writing this up rather than after.** Every address-space capability minted since 2026-08-17
already carries `ENUMERATE`, because of the `Rights::ALL`-on-creation invariant `Endpoint` also
uses. Consider every existing holder of a delegated (not self-created) address-space capability that
happened to keep `ENUMERATE`, because nobody had a reason to strip a bit that did nothing yet. The
day the new method starts consulting the bit, each such holder gains a real new power nobody
assessed at delegation time. This is not hypothetical. The milestone's own first stratum already
found and fixed the identical shape once (a `ps` riding on `READ` could reap, because `READ` covered
a right nobody had separated out yet). Whoever builds `pmap` audits existing address-space
delegation sites before the method ships. They narrow any that should not carry `ENUMERATE`, the
same way the `READ`/`ENUMERATE` split was applied retroactively for `Endpoint`. This does not change
the decision; it changes what "built" requires to be true before the method goes live.

## What this does not decide

The exact method name and wire shape (mirroring `SURVEY`'s shape is the expectation, not a
requirement) is left to whoever builds `pmap`. That builder records the new method's semantics in
`design/decisions/` per the standing rule for new methods within the established model.

## What it unblocks

Milestone 126's `pmap`, the second of its four still-blocked view programs (`pmap`, `top`, `pwdx`,
`w`). `top` (per-thread CPU accounting), `pwdx` and `w` (a process display name) remain blocked on
work this decision does not touch.

## Addendum, 2026-10-06: a foreign cursor answers `BadPointer`

Milestone 779 (fuzz the surface a confined process can reach) found `LIST` following a
caller-supplied cursor into the revocation log's page chain with no check. The result was a kernel
data abort on the fuzzer's random cursor, and at worst the page's contents returned as mapping
records. The fix made `revoke::list_mapping` walk the cursor's page against the space's own chain,
inside the one `SPACES` hold. The fix raised one question this section's "left to whoever builds"
line had not answered: what the method answers when the cursor is foreign on a *live* space.

calef, 2026-10-06, on the recommendation in pull request #1745: *"A"* (option A, `BadPointer`
over `DONE`; [the ruling comment](https://github.com/nifeos/nife/pull/1745#issuecomment-6027772247)).

`Error::BadPointer`, not `DONE`. `DONE` means the listing is exhausted. A legitimately stale cursor,
one whose space died mid-walk, already gets `DONE` from the space-lookup miss. A foreign cursor on a
live space is different. The caller's word is garbage, and folding garbage into `DONE` would hand it
a silently truncated listing rather than an error. `pmap` would print a wrong map instead of
learning it is broken. Every other method that takes a caller-supplied word answers `BadPointer` for
garbage. So this is the boundary's existing convention, one object type over. The cost is symmetric:
`pmap` only feeds back cursors the kernel minted, so a well-formed caller never sees either answer.
