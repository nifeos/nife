# BUGS entries with no milestone

*Name: provisional. `bugs-without-milestones` and this directory were minted on 2026-10-21 (UTC) by the `maintainer/bugs-survey` lane; calef has not ruled on them.*

A survey of every `BUGS` section in the tree on 2026-10-21 (UTC), taken at main `87c7ffa15`. calef
observed that `BUGS` entries never become work: only roadmap milestones' outstanding items and
NOT-STARTED milestones are surveyed and launched. This note lists the entries that are real future
work with no milestone, so calef can pick which to raise. Nothing here is raised; every title below
is a proposal.

## What was surveyed and how

- **Sections.** 1,226 `BUGS` sections in 1,170 files (module headers in `components/`, `crates/`,
  `kernel/`, `helpers/`, `script/`, `xtask/`, notes, briefs, workflows and the `BUGS` blocks inside
  roadmap milestones). `vendor/` is excluded. About 4,700 entries.
- **Classes.** Each entry was put in one class by a reader, working from the entry text and the
  roadmap files.
    - (a) a documented limit that is intended behavior: by design, inherent to hardware or an
      upstream, a deliberate refusal, an accepted trade-off, a scope statement.
    - (b) a gap a milestone already covers (it is named).
    - (c) real future work with no milestone. Listed below.
    - (d) stale, already fixed, or not a limitation.
- **Counts** (hand tallies by the readers, good to a few percent): (a) about 3,200, (b) about 320,
  (c) 667 entries merged into 592 rows below, (d) about 510.
- **Not apples to apples.** The classification is a judgment, made by fourteen readers on fourteen
  slices of the text, each told to lean to (a) when an entry says "by design" or "deliberately". Each
  verified some (b) and (c) claims against the roadmap and took the rest from the entry text. Treat a
  row as "a reader thinks this wants a lane", not as a finding. The (a) share is the least reliable
  number: it is where a reader decides calef would not want a lane.
- **Already raised.** The ps/top thread grouping gap that started this survey already has a milestone
  and is not in the table.

## Findings that are not rows

- **Cited milestone is done, gap is open.** About thirty entries say a milestone will fix them, and
  that milestone is BUILT, REFUSED, REMOVED or SUPERSEDED while the gap remains. They are in the
  table (their gap text names the milestone), and they are the clearest case for a
  check: the citation reads as coverage and is not. Examples: `components/src/swish/running.rs`
  cites 0599 (BUILT), `crates/abi/src/rendezvous.rs` cites 0706 (BUILT, its follow-on has no
  milestone), `components/src/login_audit_receiver.rs` rests on 0480 (REFUSED).
- **Stale entries.** About 510 entries record something since fixed or overtaken (for example the
  unpinned Kani entry, fixed by 0795). They are not listed; the mechanism below would catch the
  cited-milestone subset of them.
- **Build, CI, scripts and tooling** is the largest group (172 rows) and is mostly small gates and checks.

## The entries, by area

Where is the file whose `BUGS` section holds the entry (a roadmap number means the entry is in that milestone's block). Size is the reader's guess: S under a day, M a lane, L more than one lane. Rows with two locations are near-duplicates merged.

- [Kernel, ABI and component plumbing (part 1 of 2)](kernel-abi-and-component-plumbing-1.md): 68 rows.
- [Kernel, ABI and component plumbing (part 2 of 2)](kernel-abi-and-component-plumbing-2.md): 15 rows.
- [Drivers, boot and install (part 1 of 2)](drivers-boot-and-install-1.md): 71 rows.
- [Drivers, boot and install (part 2 of 2)](drivers-boot-and-install-2.md): 3 rows.
- [Network stack](network-stack.md): 18 rows.
- [Filesystem and storage](filesystem-and-storage.md): 28 rows.
- [Shell and user programs](shell-and-user-programs.md): 45 rows.
- [std, C library and packages](std-c-library-and-packages.md): 34 rows.
- [Security, verification and fuzzing](security-verification-and-fuzzing.md): 38 rows.
- [Tests, benchmarks and measurement](tests-benchmarks-and-measurement.md): 73 rows.
- [Build, CI, scripts and tooling (part 1 of 3)](build-ci-scripts-and-tooling-1.md): 72 rows.
- [Build, CI, scripts and tooling (part 2 of 3)](build-ci-scripts-and-tooling-2.md): 77 rows.
- [Build, CI, scripts and tooling (part 3 of 3)](build-ci-scripts-and-tooling-3.md): 23 rows.
- [Docs, briefs and naming](docs-briefs-and-naming.md): 27 rows.

## A mechanism so a (c) entry cannot be added without a milestone

This is a question for calef. It changes decision §71 (a limitation is promoted when it stops being a
fact and becomes a plan) and the developer-lane rule that the `BUGS` convention is not gated, so it
is his to rule on.

Why the gap exists: §71 keeps `BUGS` as disclosure, and promotes only on three triggers. The
developer-lane rule accepts a `BUGS` entry as a home for identified work and says no check can tell an
intention from an observation. So the tree has no step at which an untriggered gap meets a decision to
fund or decline it. That is rung four of the ladder (a note) holding something that needs rung two.

### Options

1. **Disposition tag, gated on new and changed entries.** Every `BUGS` entry ends with a tag, one of
   two: `[limit]` (the author declares it intended) or `[milestone NNNN]`. `script/lint` fails a
   new or edited entry with neither, and fails a `[milestone NNNN]` whose milestone does not exist or
   is BUILT, REFUSED, REMOVED or SUPERSEDED. A gap with no milestone cannot be recorded: the author
   either raises a NOT-STARTED proposed block in the same pull request or declares the limit
   intended. Existing entries are grandfathered by a ratchet (a count file that may only fall), the
   shape the tree already uses for other gates, so there is no 4,700-entry backfill. Weakness: `[limit]`
   is self-declared, so a lane can launder a gap through it; the survey above is the audit for that.
2. **Citation liveness only.** No tag. Lint finds `milestone N` or a four-digit roadmap number in a
   `BUGS` entry and fails when that milestone is BUILT, REFUSED, REMOVED or SUPERSEDED. It runs on the
   whole tree at once and needs no ratchet, because the stale set is about thirty entries. It catches
   the false-coverage case and nothing else; an entry that cites nothing passes.
3. **Repeat this survey on a schedule.** A lane re-runs the sweep each month. Rung four; it depends
   on a person remembering to start it, which is the failure that produced this note.

### Recommendation

Option 2 first, then option 1. Option 2 is a small lint section that fixes about thirty entries on
the day it lands and cannot be laundered. Option 1 is the gate that answers the question, and it
should land second so that it builds on a clean base. If both cost the same, I would still pick
this order: option 1 alone leaves the stale citations in place and treats them as covered.
Option 3 is not recommended on its own.

Whichever lands, §71 needs one sentence amended: an untriggered limitation stays a `BUGS` entry, but
it states `[limit]` or names its milestone. That amendment is calef's.
