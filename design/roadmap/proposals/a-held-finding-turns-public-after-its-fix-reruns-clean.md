---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A held finding turns public after its fix reruns clean

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from step 4 of the ruling recorded
in [fuzz-findings-without-a-person](fuzz-findings-without-a-person.md), and from the 90-day outer cap
of the disclosure ruling of 2026-10-31 (UTC).

Reuse: the window code of `helpers/continuous-fuzz.sh` replays a seed; the filing service's token
code calls the API.

## What it builds

Once a day, after the host has moved to the newest green main, the service replays the seed of every
advisory it filed that is still a draft, one seed per guest, against the current commit.

- A clean replay publishes the advisory (an update that sets its state to `published`) and moves the
  held log out of the held directory, so the daily report's held count falls.
- A failing replay keeps the advisory private and adds a comment to it naming the commit that still
  fails. The maintainer reads open advisories with such a comment as the cue to raise a new lane.
- An advisory 90 days past its report publishes whatever the replay says, which is the outer cap.
- The daily report gains the count of findings published that day.

## How it is checked

A selftest with a stub replay (pass, fail) and a stub `curl` checks that a pass sends one publish
call, a fail sends one comment and no publish, an advisory past 90 days publishes on a fail, and no
call sends a seed in a public field.

## Status

Not started. It waits on the filing service. Unverified: whether a published advisory shows its
comments to the public, and whether publishing needs more than the advisory write permission.
