---
status: PROPOSED
raised: 2026-09-19
---

# 182. Is a string two binaries agree on a name for `script/names`' purposes, or is it data?

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 398 (provenance for the names two programs agree on)'s
`DECISION` gate naming no section. The finding itself is milestone 283 (one provenance block per file, in the spelling the parse
reads)'s: its gate fired on a record
nobody knew was there. *(Section number provisional until the merge queue lands it.)*

## What is being decided

`script/names` enumerates four kinds of named thing: a crate, a program, a `script/` entry point and
a Cargo package. A fifth kind carries provenance at the thing exactly as milestone 115 asks, on a
surface nothing has ever listed. The decision is whether that surface is in scope, and it is a
scope question rather than a mechanism one: scope decides how long the worklist calef is handed
becomes.

Three sub-questions follow from a yes, and they are why this is not one line:

1. Is a wire string a name here, or data?
2. Where does its provenance block live? Not the file header: milestone 283 made that spelling
   mean *this file's* one block, which belongs to the crate, so a second one is unreachable by
   construction.
3. How is the set enumerated? The four existing kinds are a directory listing or a manifest
   walk. A wire string is a `const` in an arbitrary file, which is neither, and a scheme that cannot
   enumerate its own surface is the hole this section is about, one level in.

## Is the premise true

Checked 2026-09-19 in this worktree. Yes. `crates/measured_boot/src/lib.rs:303` carries

```rust
pub const PROGRAM_MEASUREMENTS: &str = "program_measurements";
```

under a comment saying in its own prose that the name is provisional, that it "sits outside the four
surfaces `script/names` enumerates", and why it deliberately does not wear the `Name:` spelling.
`script/names` still enumerates exactly those four.

`"program_measurements"` is an archive entry name: the kernel's trust root names it and init reads
it out of the archive at boot. It is a string two programs agree on, which AGENTS.md puts in the
expensive, hard-to-reverse category beside a wire format and an opcode number. It has a provisional
name, its author said so, and `--unratified` has never listed it.

## What this tree already does in the analogous case

The `package` kind is the precedent and it is exact. It was added on 2026-08-18 because
`script/names std_exerciser` had been answering *"neither a name in the tree nor a recorded
refusal"* for weeks about eight real packages, and the argument written at the time carries whole:
a registry with a hole is worse than no registry, because it answers confidently about the names
it happens to cover.

And this gap is already a recorded limitation rather than an unknown.
[design/naming/provenance-limits.md](../naming/provenance-limits.md) carries it, under "The tree has
more kinds of name than the table covers", which is the one place the worklist's scope is stated. It
also records a live casualty: `fs_maker` was resolved to
`mkfs` by whoever was mid-task, because `redoxfs_server/src/bin/` is a place nothing looks. That is
a `BUGS` entry, which under §71 is a fact rather than a plan, and this section is where it becomes a
plan or is refused on the record.

`script/names` reads four kinds by marker (`design/naming.md`, "Four more kinds are read by
marker"), one of them an `item`: any function, constant, type, field or variant whose `///` doc holds
a `Name:` paragraph. That is a mechanical answer to sub-questions 2 and 3 above, since the block
lives in the constant's own doc and enumeration is by marker rather than by directory. It does not
answer question 1, whether a wire string is a name at all, and `PROGRAM_MEASUREMENTS` carries no
marker: `script/names program_measurements` still answers "neither a name in the tree nor a recorded
refusal". Its doc comment's reason for not wearing `Name:` (milestone 283's one-block-per-file
spelling) predates the `item` kind and is stale. The worklist figures below are from 2026-09-19.

## What it costs, counted rather than asserted

The counting is the cheap half and it now has a first answer. `pub const <NAME>: &str` matches
77 times across `crates/`, `components/`, `fixtures/` and `kernel/` in this worktree. That is an
upper bound rather than the count, because not all 77 are agreed between two binaries; many are
one crate's own strings.

For scale, `script/names --unratified` is 71 deep of 222 today, with 1 unrecorded and 27
recorded. So the widening under discussion is at most a doubling of the tree's named surface and
plausibly much less, which is the figure the scope question turns on.

The other candidate sets, unenumerated, which is itself the point: the `*_proto` crates' operation
names and error codes, which already carry blocks *for the crate* and not for the words on the wire;
and public function and method names, calef's since 2026-08-23 and counted by nothing, which
milestone 276's own `BUGS` already records.

## Recommendation

Count before widening, and then choose between two answers rather than three. The block that
became milestone 398 said this and it is right: if the real figure is a dozen, the answer is
probably a list in `design/naming.md` and no new machinery. If it is closer to 77, the answer is
probably that wire strings are declared out of scope and the record says so on purpose, the way
`unrecorded` is a first-class answer rather than a gap.

No recommendation between those two, because the choice is what size of worklist calef accepts,
which is not a lane's to price. What this section does refuse is the third option of leaving it
unstated: a hole nobody has decided to keep is the failure the `package` kind was added to fix.

## How reversible, and who has acted on it

Mixed, and that asymmetry is the argument for deciding it. The registry is cheap to widen or
narrow. The names it does not cover are the least reversible things in the tree, because they are
agreed between two binaries; every day a wire string goes unlisted is a day it can be shipped
provisionally and then inherited.

## What is blocked until this is answered

Milestone 398. Nothing else: `PROGRAM_MEASUREMENTS`' record is preserved in prose one hop from
the constant. What it does not have is a gate.
