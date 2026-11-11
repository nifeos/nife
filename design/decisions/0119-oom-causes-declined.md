---
status: DECIDED
raised: 2026-08-23
decided: 2026-08-23
ratified_by: calef
---

# 119. Splitting `OutOfMemory`'s three causes is declined for want of a customer

calef, 2026-08-23, on milestone 153 (`OutOfMemory` collapses three distinct causes into one error code)'s fork: *"Let's defer until we have a
customer."*

## The question

`Untyped::SPLIT` and `RETYPE_OBJECT` (method 2, its name spelled out by the ruling of 2026-11-11
(UTC)) both return `Error::OutOfMemory` for three unrelated causes: the caller's own region budget
exhausted, the caller's own cspace full, or `MAX_REGIONS` (the system-wide concurrently-live region
table) full. The first two are facts about the caller. The third is a fact about everyone else on
the machine, fixable by nothing the caller can do locally. Distinguishing them touches the syscall
ABI's shared `Error` enum, so the milestone named it a fork rather than guessing.

## The decision

**Declined for now.** No caller is confused by this today. It was found while *pricing* milestone
152's future durable-session cost, not from an actual `MAX_REGIONS`-exhaustion bug anyone hit. Same
shape as `std::thread::spawn` (§105), hard links (§110), and live component state handoff (§116):
revisit when a customer needs it, not before.

## Why this was still worth checking properly before deferring

Two things point at the eventual shape, checked rather than argued, so the deferral is informed
even though it doesn't commit to anything:

This tree already solved the identical shape of problem once. `crates/timetable`'s
`Unbacked`/`Refusal` split exists for exactly this reason, in a different subsystem: "a `Refusal` is
a fact about *the line*... an `Unbacked` is a fact about *the scheduler*." When this tree has
previously hit "one error code collapsing a caller-fact and a system-fact," it split the code, not
left it collapsed.

POSIX already drew this exact line, verified rather than recalled. `EMFILE` (a process's own
file-descriptor limit exhausted, a per-process fact) is a distinct code from `ENFILE` (the whole
system's file table full, a system-wide fact nothing the caller does fixes locally) -- [errno(3),
Linux man-pages](https://man7.org/linux/man-pages/man3/errno.3.html). That is nife's cause 2 versus
cause 3, precisely. `ENOMEM` covers the general allocation-failure case, roughly nife's cause 1.

Both precedents point the same direction: when this is eventually built, new cause-specific `Error`
variants (matching `timetable`'s `Unbacked`/`Refusal` shape and POSIX's `EMFILE`/`ENFILE` split) is
the better-supported option, over a separate diagnostic-only query or leaving it collapsed forever.
This is non-binding guidance for whoever eventually has a real customer, not a commitment. Re-check
it against what that customer's failures actually look like rather than building to a description
with nothing to correct it. That is the same caveat §116 (live component state handoff is declined)
attached to its own guidance.

## What this does not decide

The exact variant names and wire shape, if and when this is built, are left to that lane. Whether
`MAX_REGIONS` itself should be raised as durable sessions land (already the expected response per
DECISIONS §109) is a separate, already-anticipated move and not blocked by this decision either
way.

## What it unblocks

Nothing was gated on this (the milestone's own text says so). It closes the open question so
milestone 153 can record DECLINED rather than sitting as a live fork with no customer to answer it.
