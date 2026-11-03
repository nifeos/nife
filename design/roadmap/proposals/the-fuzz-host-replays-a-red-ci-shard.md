---
status: PROPOSED
raised: 2026-11-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The fuzz host replays a red CI shard

Raised by lane/fuzz-setup-reconcile on 2026-11-03 (UTC), while it reconciled the fuzz setup page with
what is built. [Milestone 1012](../1012-a-short-sharded-fuzz-pass-in-ci.md) names this work as a
follow-on "written by another lane", and no file held it. The integrator mints the number at
promotion.

Reuse: the window logic of `helpers/continuous-fuzz.sh`, whose `--ci-range SHARD` gives a shard's
driver and first seed; the held-findings store; and the filer in nifeos/fuzz-host, which files every
held log as a private advisory.

## Background

The `fuzz-guests` job runs one short in-guest pass per shard on every merge group, on a hosted runner,
as a non-required check. When a shard goes red, the job prints a pointer: the commit, the shard number
and the line "the fuzz host replays this shard". It prints no seed and no guest text, because a hosted
log is public. Nothing replays the shard today, so a red job is a pointer to nobody.

## What it builds

The host side of that pointer, in three parts. Each part is the one milestone 1012 lists.

1. **The host learns that a shard went red.** Open: the trigger needs a path from GitHub to the host.
   The host already reads public check runs without a credential when it follows main, so the first
   candidate is the same poll. It would look for a failed `fuzz-guests` run on a merge-group commit,
   and read the commit and shard from the pointer line alone.
2. **The host replays the shard.** It checks out the commit and runs `--ci-range SHARD`, so the replay
   is deterministic. The commit of a merge-group run lives on a `gh-readonly-queue/` branch that
   GitHub deletes after the merge, so the host must fetch it while GitHub still holds it, or take the
   tree some other way. That is the open question milestone 1012 left here.
3. **A reproduction becomes a held finding.** It enters the existing path: the supervisor holds it, the
   filer files a private advisory, and a fix lane reads the advisory. Nothing that reproduces reaches a
   public log, release or artifact.

## How it is checked

A selftest runs the replay against a stand-in guest that prints a `replay:` line, and against a clean
one. The first must leave one held log and no output beyond a finding id and an age. The second must
leave nothing. A stub of the check-run listing must show that a green run, a skipped run and a red run
on an unrelated job each start no replay.

## Open questions for the milestone's lane

- Poll or push: whether the poll above is enough, or the job needs a way to call the host. A push
  needs a credential in the workflow, which 1012 refused to add.
- How the host gets a commit that GitHub is about to delete.
- Whether a replay that comes back clean is worth a note in the daily report, since a flaky shard is
  the false-red rate that 1012 still has not measured at four shards.

## Status

Not started. The filer it feeds is built and not installed
([`finishing-the-setup.md`](../../../notes/fuzzing/finishing-the-setup.md)).
