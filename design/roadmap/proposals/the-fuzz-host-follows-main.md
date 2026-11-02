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

A selftest with a stub `git` and a stub API response checks the cases. A green fast-forward moves.
A non-fast-forward, a red check, a pending check and an unreadable response each stay put. A changed
unit file writes the journal line and installs nothing.

## Status

Built: `helpers/fuzz-follow-main.py`, the update step in
`helpers/continuous-fuzz.sh`, and `helpers/fuzz-follow-main-selftest.sh`, which `script/lint` runs.
It reaches the host through step P1 of
[`finishing-the-setup.md`](../../../notes/fuzzing/finishing-the-setup.md), a person's step.

The named list is main's required checks, read from the branch's rules through the public API with
no credential (HTTP 200, verified 2026-11-02 UTC). A look whose fetch finds nothing new makes no API
request; a look at a new commit makes three. Verified 2026-11-02 (UTC) on main's last 25 first-parent
commits: every gate of a merge-queue commit appears as a successful check run on that commit once it
is main. 7 of the 25 carried only skipped runs of the required checks, because they were earlier
commits of a merge-queue batch whose last commit carried the gates; the follower never moves to
those, and main's tip is always a batch's last commit.
