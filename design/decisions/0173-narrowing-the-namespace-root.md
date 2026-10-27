---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-27
ratified_by: calef
---

# 173. Narrowing the root of the shell's namespace: a verb on the wire, or a shallower root

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice B, which read milestone 328's
`DECISION` gate and found it naming no section. The fork is milestone 31's, stated in that block
under *"The two shapes a grant cannot take"* and carried forward by the 2026-09-03 proposal sweep.
*(Section number provisional until the merge queue lands it.)*

## The ruling

Decided 2026-10-27 (UTC), option A, in the same sitting as the ruling on §98. `filesystem_contract` gains a narrowing verb, verb 23. It takes a held directory handle
and a rights mask, mints a second handle whose rights are the held rights run through `Rights::attenuate`, and
resolves no name. The ruling declines options B and C: the shell's root stays where it is, and nobody
records the top prompt as a limitation. Milestone 328 builds the verb, and the client and the server
both call the proved `Rights::attenuate` rather than restating it.

## What is being decided

Milestone 31's claim is that typing a name is the grant. At the top prompt it is false: `rm
rmtree/rm-solo` works and `rm gate.txt` is refused, and the only difference is one level of path.

A subtree caretaker attenuates by performing one `OPENDIR` *into* the directory it was granted. The
root of the shell's namespace has no name to descend into, so there is nothing to attenuate through.
Two answers, and they are permanent in different ways:

1. A narrowing verb on `filesystem_protocol`, meaning "the directory I already hold, with fewer
   rights", with no name resolution.
2. An interactive boot whose shell starts one component below the image root, so the root the
   user meets always has a parent.

## The tree as it stands, read rather than recalled

`crates/filesystem_contract` carries verbs 0 through 22 (`OPEN` through `SETMTIME_AT`) and none of
them means what option 1 needs. `dir::Rights::attenuate` exists and is client-side only:

```rust
pub const fn attenuate(self, requested: u64) -> Self {
    Rights(self.0 & requested)
}
```

One `&`, a total function with no failure mode, and machine-checked: `attenuate_never_widens` is a
Kani proof over an arbitrary parent and an arbitrary request, with a replayable falsification beside
it. `OPENDIR` and `MKDIR` narrow *through* that function; nothing applies it to a handle already
held.

So option 1's arithmetic is built and proved. What is not built is a verb that reaches it, and the
server side that mints a second handle from a first.

## The neighboring decision, which should be read with this one

[§98](0098-opendir-cannot-attenuate.md) is `PROPOSED` and is a rights change to the same wire,
raised by milestone 122 (a directory handle `std` can hold)'s lane. `OPENDIR` cannot be asked for
"the parent's rights, whatever they are", so a `Dir` asks for `dir::ALL` and then discovers what it
holds by probing one right at a time. Its proposal is a sentinel in `OPENDIR`'s rights word, about
thirty lines across the protocol, the server and the PAL.

**The two do not subsume each other and the difference is worth stating**, because a reader meeting
both will assume one design covers them. §98 is about a client that cannot read its own capability
and therefore cannot ask for the right amount; this is about a handle with no parent to ask through.
A sentinel in `OPENDIR` still needs a name to open. A narrowing verb still leaves a client unable to
discover what it holds.

What they do share is a page in calef's queue. Both add rights machinery to `filesystem_protocol`
and both are irreversible in the same way. Answering them in one sitting costs less than answering
them six weeks apart, and answering §98 alone would be the more expensive order, because it ships a
rights-shaped addition to this wire without the other case in view.

## The options

| | shape | cost |
|---|---|---|
| **A** | A narrowing verb, verb 23. Takes a held directory handle and a rights mask, mints a second handle, resolves no name. | Small in the server, and the arithmetic is `attenuate` with its proof already written. It is an addition to something two programs agree on, so it cannot be un-shipped. Verb 23 is a number every future reader of this protocol carries. |
| **B** | An interactive boot whose shell is rooted one component below the image root. | Nothing on the wire, nothing to un-ship, and no new verb. It changes what every relative path at that prompt means, which cannot be un-taught: every example, every note and every person's habit moves one level. |
| **C** | Neither, and the limitation is recorded where the reader meets the feature. | Free, and it is the option milestone 31's block did not take, because `notes/dir-capability.md` already carries the sibling case (a grant more than one level down) as a `BUGS` entry and this one is a fork rather than a limitation. |

The fork arrived as options without a recommendation, because both A and B are in the column
AGENTS.md prices as irreversible: A is a thing two programs agree on, and B is a fact that lands in
a reader's head. The ruling chose A.

**What a lane could measure before the ruling, and it is not much.** A's size is already known from
§98 (OPENDIR cannot be asked to attenuate)'s neighboring estimate (about thirty lines for a smaller change on the same surfaces). B's cost
is not a measurement at all; it is a judgment about what a prompt means, which is why it is calef's.

## What this unblocks

Milestone 328 (a grant on the root of the shell's namespace), entirely. Nothing else cites it.
`script/swish-check` gates the working case on both ISAs, and gains the top-prompt case.
