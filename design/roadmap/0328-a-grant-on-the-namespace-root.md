---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: 173
machine_requirements: none
specific_machine: none
needs_person: no
---
# 328. A grant on the root of the shell's namespace

Filed 2026-09-03 as an unnumbered proposal by the milestone 247 sweep,
from milestone 31's block; numbered 2026-09-19 by milestone 433. **Premise re-checked 2026-09-19 and
it holds.** `crates/filesystem_contract` carries verbs 0 through 22 and none of them means "the
directory I already hold, with fewer rights": `dir::Rights::attenuate` is a pure client-side
constructor that `OPENDIR` and `MKDIR` narrow *through*, not an operation on a held handle. Milestone
31's block still records the refusal in "The two shapes a grant cannot take", and no interactive boot
roots the shell below the image root.

Both permanent answers were an architect's call. On 2026-10-27 (UTC) §173 (narrowing the root of
the shell's namespace) ruled: option A, a narrowing verb, verb 23 on `filesystem_contract`. It takes a
held directory handle and a rights mask, mints a second handle through `Rights::attenuate`, and
resolves no name. The client and the server both use the proved `Rights::attenuate`; neither restates
it. The shell's root does not move. This milestone builds the verb and the test that `rm gate.txt`
at the top prompt is granted narrowed, as `rm rmtree/rm-solo` already is.

**In brief.** A subtree caretaker attenuates by performing one `OPENDIR` *into* the directory it was
granted. The root of the shell's namespace has no name to descend into, and `filesystem_contract` has
no verb meaning "the directory I already hold, with fewer rights". So `rm rmtree/rm-solo` works at
the prompt and `rm gate.txt` is a refusal, and the only difference between them is one level of
path. Closing it is either a narrowing verb on the contract (`Rights::attenuate`, no name
resolution, small in the server) or an interactive boot whose shell starts one component below the
image root.

## Why this matters

Milestone 31's whole claim is that typing a name is the grant. At the top prompt that claim is
false, for a reason a user cannot see and cannot work around: the same command on the same file
succeeds or is refused depending on how deep the file sits. `script/swish-check` gates the working
case on both ISAs, so the gap is not a regression risk. It is a permanent hole in the headline
demonstration. It is the first thing a newcomer typing at the prompt will hit, because the top
of a namespace is where anyone starts.

The two answers are not equal and the choice is not close to free. The narrowing verb is the smaller
code change and the larger commitment: it adds a right-attenuating operation to a wire two programs
agree on, forever. Rooting the shell below the image root puts nothing on the wire and costs
meaning instead, since every relative path at that prompt then refers to somewhere else.

## Where it came from

Milestone 31's block, under "The two shapes a grant cannot take": *"This is a design fork and
belongs to calef, because both answers are permanent: a narrowing verb on the contract (small in the
server, `Rights::attenuate` with no name resolution, and an addition to something two programs agree
on) or an interactive boot whose shell is rooted one component below the image root (nothing on the
wire, and it changes what every other command at that prompt means)."*

`notes/dir-capability.md` carries the sibling limitation beside the feature: a grant more than one
level down is a chain of caretakers and is also still a refusal. That one has a home in the BUGS
convention. This one does not, because it is a fork rather than a limitation.

## Index row

Milestone 31's whole claim is that typing a name is the grant, and at the top prompt that claim is
false: `rm rmtree/rm-solo` works and `rm gate.txt` is a refusal, and the only difference is one level
of path. A subtree caretaker attenuates by performing one `OPENDIR` into the directory it was
granted, the root of the shell's namespace has no name to descend into, and `filesystem_contract` has
no verb meaning "the directory I already hold, with fewer rights". The two answers are not equal and
neither is free. A narrowing verb (`Rights::attenuate` on the wire, no name resolution, small in the
server) is an addition to something two programs agree on and cannot be un-shipped. An
interactive boot rooted one component below the image root puts nothing on the wire and changes what
every other command at that prompt means. It is the first hole a newcomer typing at the prompt will
hit, because the top of a namespace is where anyone starts.
