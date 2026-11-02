---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# Fix lanes read the advisory

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from steps 2, 3 and 5 of the
ruling recorded in [fuzz-findings-without-a-person](fuzz-findings-without-a-person.md).

Reuse: `gh api` reads the advisory. No new transport.

## What it builds

- A script a fix lane runs to read one advisory: `GET /repos/nifeos/nife/security-advisories/<id>`
  with a one-hour token of the `nife-fuzz-reader` App, handed to the lane at launch. It prints the advisory to
  the lane's terminal and writes nothing to disk inside a worktree. It refuses, by name, when no
  credential is present, and it never searches for one.
- The rule text in the developer-lane and maintainer skills:
  - No lane opens, copies, prints or moves anything under the fuzz host's held directory. The
    advisory is the only way a lane learns a finding.
  - A fix lane for a held finding claims with `script/claim --no-claim`, as §90 (the claim is a
    draft pull request) allows for this kind of lane. It commits locally and often, pushes nothing
    until the fix is ready, then opens the pull request ready for review with auto-merge armed and a
    neutral title. Local commits survive a dead session in the worktree; the machine's loss before
    the push loses them.
  - The fix pull request carries the fix and a regression test of the broken invariant that names no
    seed, input or replay line. Its title, body, commit messages and comments name no seed or input
    either. The seed joins the fixed-seed regression list in a second pull request opened after
    the advisory turns public.
  - The maintainer raises at most one lane per open advisory and records on the advisory which lane
    holds it.
- A gate: `script/lint` fails a commit message or pull request body that carries a `replay:` line or
  a `NIFE_*_SEEDS=` assignment, the two forms a reproducer takes in this tree.

## How it is checked

The script's selftest runs it against a stub `gh` with no network and checks the refusal without a
credential. The gate's selftest feeds it a planted replay line and a clean message.

## Status

Not started. Unverified: how long a fix pull request stays public before it merges. That window runs
from the push to the merge, and it grows whenever a hold applies (`needs-architect` on a fix that
touches the syscall surface, or a red trunk). The first fix measures it. It waits on
[fix lanes get a one-hour reader token](fix-lanes-get-a-one-hour-reader-token.md) for a real read, not
for its selftests.
