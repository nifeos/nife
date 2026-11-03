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

Reuse: the window code of `helpers/continuous-fuzz.sh` replays a seed; the filer's ledger (`ledger_state()` in
`bin/fuzz-filer` of nifeos/fuzz-host) says which findings are filed, and with which advisory id.

## What it builds

Once a day the service replays the seed of every advisory the filer's ledger records as filed that is still a draft. A ledger line is a UTC time, a finding id, `pending` or `filed`, and an advisory id or `-`, tab separated, and the last line of a finding wins. Each advisory's description starts with a `Finding:` line holding the finding id, and the service matches an advisory to its ledger line by it. It runs
one seed per guest, against the newest green main the host runs.

- A clean replay publishes the advisory (an update that sets `patched_versions` to the commit that fixed it and its state to `published`) and moves the
  held log out of the held directory, so the daily report's held count falls.
- A failing replay keeps the advisory private and adds a comment to it naming the commit that still
  fails. The maintainer reads open advisories with such a comment as the cue to raise a new lane.
- An advisory 90 days past its report publishes whatever the replay says, which is the outer cap.
- The daily report gains the count of findings published that day.

## How it is checked

A selftest runs with a stub replay and a stub `curl`. A pass must send one publish call. A fail must
send one comment and no publish, unless the advisory is past 90 days, when it publishes. No call may
carry a seed in a public field.

## Status

Not started. It reads the filer's ledger, which exists. Publishing needs only the advisory write permission, once the advisory carries a package and range.
GitHub answers 422 without them, and the filer supplies them
([the test](../../../notes/fuzzing/finishing-the-setup.md#the-advisory-permission)). Unverified:
whether a published advisory shows its comments to the public. Also unverified: whether the REST API
can add a comment at all.
