---
status: DECIDED
raised: 2026-08-18
decided: 2026-10-27
ratified_by: calef
---

# 98. `OPENDIR` cannot be asked to attenuate, so a held directory probes for its own rights

Raised 2026-08-18 by milestone 122 (`OPENDIR` reaches the PAL)'s lane, which shipped the workaround and
said so rather than presenting it as a design. What is proposed is the replacement, and it is a
wire change, which is why it is calef's and not a lane's.

Nothing was blocked: milestone 122 is built and correct. The cost of leaving the workaround was that
the tree carried it in the place a reader meets the type.

## The ruling

Decided 2026-10-27 (UTC), yes, in the same sitting as §173 (narrowing the root of the shell's
namespace). `OPENDIR`'s rights word accepts a sentinel meaning "the parent's rights, whatever they
are". The ruling deletes the `Dir::held_rights` probe and the same trap in `MKDIR`. The sentinel
cannot widen anything, because the result is `parent & parent`.

## The rule this collides with

Every verb in the std PAL asks for the minimum right it needs, because over-asking is `EPERM`
rather than attenuation, and a client cannot read its own capability to find out what it holds. That
rule is right and it is why `readdir` does not ask for `dir::ALL`. The same file records that
`readdir` nearly shipped asking for exactly that, which would have passed every test in the suite and
failed through every narrowed grant.

**The rule has no answer for a held object.** A `Dir` is not a verb. What will be asked of it later
is not knowable when it is minted, so there is no minimum to ask for.

## What shipped

A `Dir` asks for `dir::ALL`, and when a narrowed grant refuses, it discovers what is actually there by
asking for one right at a time. Six extra messages, at most once per `Dir::open`, and one message in
the common case. Correct, bounded, honest, and a workaround.

*(Dated note, 2026-09-26, decisions-hygiene lane.)* "Six" was the size of `dir::ALL` on 2026-08-18.
Milestone 47 (navigation and naming: `cd`, `pwd`, `ls`, `mkdir`, `rm`) added a seventh right,
`dir::SETTIME`, on 2026-08-24 (`dir::ALL == 127`), and the probe kept naming six, so a `Dir` under a
narrowed grant never learned whether it held `SETTIME`. No `Dir` operation asks for that right, so
nothing observed it. The probe now walks the bits of `dir::ALL`
(`patches/std-nife/overlay/std/src/sys/fs/nife.rs`, `Dir::held_rights`) and costs seven messages
today. Every "six" below means one per right, and the question this section asks is unchanged.

## What was considered, and why each lost

- Ask for a fixed useful mask. Every fixed mask has a grant it breaks under. See `readdir` above:
  this tree nearly shipped that mistake once and the suite would not have caught it.
- Mint the handle per operation. Then `Dir` is not holding a capability, which is the point of
  the type, and it reintroduces the time-of-check-to-time-of-use the type exists to avoid.
- Hold a path and re-walk when a new right is first needed. The same TOCTOU, and it is the
  generic fallback wearing a capability's clothes.

## The sentinel, priced

A sentinel in `OPENDIR`'s rights word meaning "the parent's, whatever they are." It cannot widen
anything, because the result is `parent & parent`. About thirty lines across `fs_proto`, `fs_server`
and the PAL, and it deletes both the probe and the same trap in `MKDIR`.

## Why this was an architect's

It is a thing two programs agree on, which the *move fast on what can be undone* tenet puts in the
irreversible column alongside names, dependencies and the syscall surface. The code is a morning; the
un-shipping is not.

## BUGS

- The probe's cost is asserted, not measured. "Six messages, nothing measurable cares" is
  reasoning from the shape of the code. No benchmark covers `Dir::open` under a narrowed grant, and
  milestone 121 (ripgrep on nife)'s per-component IPC measurement, which would price the neighboring case, has not
  started.
