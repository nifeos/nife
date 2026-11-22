---
status: AMENDED
raised: 2026-09-19
decided: 2026-10-05
ratified_by: calef
---

# 162. Whether a holder can give up a mapping, and what gives it up

This section is the decision milestone 95 (an unmap primitive, and the mappings init never lets go)
is built on. The block calls it *"a design fork for calef before it is a task"*.

(one amendment block below: milestone 95's two semantics, ruled on 2026-10-05.)

## Ruling

calef, 2026-10-05 (UTC): *"A now, C optional."*

**A is decided: `AddressSpace::UNMAP(va)`.** This space gives up this window, and no other holder is
touched. **C, the one-page loader, is optional**: a boot-time refinement that nothing requires.

The reason. C alone cannot close the window, because the last page stays mapped unless something
gives it up, and only A does that. And a program giving up a window it made for its own reasons is a
case only A answers. **B is refused**, for the reasons in the table. It reads as a second revoke and
invites confusion with §41 (the endpoint is the broker, and a device is revoked by taking it back).
A frame mapped at two addresses in one space has no unambiguous answer.

**Two semantics stay open**, and milestone 95's lane owes them as amendments to this section before
its build merges: what UNMAP does to a va that was never mapped, and whether the frame capability
survives the unmap. UNMAP must not contradict §13 (capability revocation and untyped reclamation) or §41.
(Amended 2026-10-05: both are ruled. See the amendment at the end.)

The recommendation below is kept as it was written on 2026-09-19, and is history now.

## What is being decided

Whether a holder can give up a mapping it made, and if so by what method. Three parts, and the
third can make the first two moot:

1. Does unmap live on the address space or on the frame.
2. What it does to a mapping some other holder also has.
3. Whether a loader that never holds more than one page at a time removes the need for it.

## The finding, re-measured 2026-09-19 and unchanged

There is no unmap in the ABI. `crates/moho_abi` gives an address space `MAP_INTO` (with `MAP_RO`,
`MAP_RW`, `MAP_CODE`) and a listing method, and a frame `MAP`; nothing removes a mapping. So the
progenitor maps each page it lays down for a child in order to write it and never lets go, which
`notes/trusted-init.md`'s honest-limits section states in its own words:

> is never unmapped (it cannot be: nothing in the ABI unmaps a page), so the progenitor can read and
> write any page it laid down for a child.
>
> -- notes/trusted-init.md

Reaping a job hides this for jobs, because reclaiming a region revokes every mapping of its pages
first ([§13](0013-frame-revocation.md)), but the boot servers are never reclaimed. The note names the
consequence precisely: the console's, the line editor's, the input driver's, the shell's and the
terminal sink adapter's memory is still reachable from the progenitor, and giving the
construction budget away does not touch that.

There is a second, smaller instance in the same note: the shell's output frame stays mapped in the
progenitor for life, because `Frame::REVOKE` would take the page from the shell too.

## What this tree already does in the analogous case

Take-back already exists, and it is not this. [§41](0041-endpoint-as-broker.md) (a device is
revoked by taking it back) made `Frame::REVOKE` on an `Object::DeviceFrame` delete every capability
naming the page from every cspace *except the invoker's*. That is the right shape for handing a
device between two servers and the wrong shape here: the progenitor wants to drop its own window
and leave the child's alone, which is the exact inverse.

And [§13](0013-frame-revocation.md) already decides what happens to another holder's mapping when
a region is reclaimed. Whatever unmap does must not contradict it, which is a constraint on the
options rather than an answer.

## The options

| | shape | cost |
|---|---|---|
| A | `AddressSpace::UNMAP(va)`, symmetric with the `MAP_INTO` that created the window. | A new method on an existing object, which AGENTS.md allows within the model provided its semantics are recorded here. Says what it means: *this* space gives up *this* window, and no other holder is touched. Needs a rule for unmapping a va that was never mapped, and a decision on whether the frame capability survives. |
| B | `Frame::UNMAP`, the holder naming the frame rather than the address. | Reads as a second revoke and invites confusion with §41's take-back, which already dispatches on the object kind. A frame mapped at two addresses in one space has no unambiguous answer. |
| C | No new method: a one-page loader. The builder maps a page, writes it, releases it before the next, so the window is never wider than one page and closes when construction ends. | No system call surface at all, which is the strongest thing that can be said about an option here. It does not remove the window, it bounds it: the last page stays mapped unless something gives it up, so C is likely A plus a loader change rather than an alternative to it. Its price is boot time, and it is measurable. |

Recommendation as raised (2026-09-19, superseded by the ruling above): measure C first, then A. Not
because C is less work (it is a restructure of the loader against a one-line method, so it is more).
It is because the measurement decides whether the system call surface has to grow at all, and this
project's rule is that the surface stays small and every method is deliberate. If a one-page loader
costs nothing observable at boot, the question becomes whether A is still worth a permanent boundary
change for the residual last page; if it costs real time, A is the answer and the measurement said
so.

The one question C cannot answer is what a *program* does when it wants to give up a window it
made for its own reasons. Nothing in this tree asks for that today, which is why C is worth
measuring rather than dismissing.

## Would we still choose this if both options cost the same

Yes, and this is worth saying because the recommendation looks like the cheap one. If a loader
restructure and a new system call were the same amount of work, the ordering would be unchanged. The
surface is a boundary every future program is written against, and taking the measurement first is
what makes the addition deliberate rather than reflexive. The effort argument would run the other
way, since C is the larger change.

## How reversible it is

A is the irreversible half, and it is the half calef chose. A new method on the address-space object is part of the surface
forever. C is a refactor of one program and can be undone in an afternoon. Nothing outside this
tree has acted on either.

## What was blocked until this was answered

Milestone 95, now unblocked on the method (A) and owing the two amendments above. Its proof shape is already settled and needs nothing from here: the progenitor
writes to a boot server's page and faults, as a negative control, which is the shape milestone 22
used.

Not blocked: the record. The residual is written where a reader meets it, in
`notes/trusted-init.md`'s honest limits, and stays there whatever is decided.

## Amendment (2026-10-05): what UNMAP refuses, and what it leaves alone

Ruled by calef on 2026-10-05 (UTC) on [#1678](https://github.com/nifeos/nife/pull/1678)
(milestone 95, an unmap primitive, and the mappings init never lets go), answering the lane's
[`## What I need from you`](https://github.com/nifeos/nife/pull/1678#issuecomment-5989908200).
Recorded by the maintainer, because a lane does not edit this directory. Both behaviors were
already built as the lane's provisional answers, so the rulings change no code. The full argument
and the prior art are in `notes/unmap.md`, which lands with #1678.

**`UNMAP` of a `va` with nothing mapped is refused with `BadPointer`**, and nothing changes. The
same answer covers a misaligned or kernel-half `va` and a page already given up. calef, 14:14 UTC:
"Refuse" ([ruling 1](https://github.com/nifeos/nife/pull/1678#issuecomment-5996276244)). The method
exists to close a window, and a silent success on a wrong address says the window is shut while it
is still open. Prior art splits: Zircon (`ZX_ERR_NOT_FOUND`) and Mach (`KERN_INVALID_ADDRESS`)
refuse, Linux `munmap` and seL4 succeed. Refusing is also the more reversible choice. A caller
written against the error keeps working if it ever becomes success, and the reverse does not hold.
It matches `MAP_INTO`, which refuses an address already mapped with the same code rather than
replacing it.

**The frame capability survives `UNMAP` untouched.** `UNMAP` reads, consumes and changes no
capability; one the caller still holds can map the page again. calef, 14:17 UTC: "Yes"
([ruling 2](https://github.com/nifeos/nife/pull/1678#issuecomment-5996329151)). `UNMAP` names an
address, and the mapping record holds only the run's base, which every derivative of a capability
shares (§132 (what `PageFrame::REVOKE` owes an overlapping run)), so there is no single slot it could
consume. It matches seL4's `Page_Unmap`, which leaves the capability in its slot, remappable.

Neither contradicts §13 or §41: `UNMAP` touches only the named space's record, so a revoke still
finds every other mapping of the page.

Not part of this amendment: the lane's third question, how a running process names its own address
space. As the object model stands, `UNMAP` reaches a space under construction but not the
progenitor's window into a running one, because no capability names a running space. That fork is
open, in `design/roadmap/0789-a-running-process-can-name-its-own-address-space.md`, and it
blocks milestone 95 reaching BUILT, not #1678's merge. (Ruled the same day as
§249 (a running address space stays nameable), except the slot init's capability occupies.)
