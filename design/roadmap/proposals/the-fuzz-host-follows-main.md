---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The fuzz host follows main

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from the second ruling recorded in
[fuzz-findings-without-a-person](fuzz-findings-without-a-person.md): the host updates itself from main
with no person pulling.

Reuse: `git fetch` and `git merge --ff-only`, and `curl` against the public API without a
credential. No new dependency.

## What it builds

An update step in `helpers/continuous-fuzz.sh`, run between windows at most every 5 minutes.

1. Fetch only `main` of `https://github.com/nifeos/nife.git`.
2. Refuse a commit whose history does not contain the current one (`git merge-base --is-ancestor`).
3. Read the check runs of the new commit from `api.github.com` with no token, and move only when
   every check in a named list of main's gates has concluded success. Red, pending, an error or a rate
   limit keeps the current commit.
4. Fast-forward the clone, then the slot worktrees once their current windows end, and re-execute the
   supervisor. The toolchain check runs again, since a new commit can move the pin.
5. If `helpers/systemd/` changed, write one journal line saying that the unit files changed and that a
   person with root must install them. Apply nothing.

Nothing here widens what the fuzz account may do. It uses no credential and reads only public data.

## How it is checked

A selftest with a stub `git` and a stub API response checks five cases: a green fast-forward moves;
a non-fast-forward, a red check, a pending check and an unreadable response each stay put; a changed
unit file writes the journal line and installs nothing.

## Status

Not started. Waits for #3045, which also edits `helpers/continuous-fuzz.sh`. Verified on 2026-11-01
(UTC): the check runs of a main commit read without a credential (HTTP 200), and the documented
unauthenticated limit is 60 requests an hour per address. Main's commit carried 125 check runs, so
one look is two pages and every 5 minutes costs 24 requests an hour. Many runs are watchers or skipped
jobs (a cancelled merge-drain run is not a red gate), which is why the step reads a named list.
Unverified: whether every gate on a merge-queue commit appears as a check run on that commit
once it is main, rather than only on the queue's own ref.
