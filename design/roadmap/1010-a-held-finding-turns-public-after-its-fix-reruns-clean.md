---
status: REFUSED
promoted_from: a-held-finding-turns-public-after-its-fix-reruns-clean
raised: 2026-11-01
refused_by: none
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1010. A held finding turns public after its fix reruns clean

Refused by calef on 2026-11-04 (UTC). The continuous fuzzer ends at reporting: the host files one private
advisory per held finding and does nothing after. A person works the advisories with the regular lane
workflow and publishes each one. The host does not replay a fix, publish an advisory or move a held log.

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from step 4 of the ruling recorded
in [fuzz-findings-without-a-person](1015-fuzz-findings-without-a-person.md), and from the 90-day outer cap
of the disclosure ruling of 2026-10-31 (UTC).

Reuse: the window code of `helpers/continuous-fuzz.sh` replays a seed; the filer's ledger (`ledger_state()` in
`bin/fuzz-filer` of nifeos/fuzz-host) says which findings it filed, and with which advisory id.

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

## Revisit

- **Condition.** A person asks for the host to publish after a fix, for example because the 90-day
  cap passes unnoticed on an open advisory. The service below then comes back as written, and it
  reads the filer's ledger, which exists.

## Index row

Refused 2026-11-04: the fuzz host files held findings and stops, and a person publishes each advisory after its fix. The daily replay that would have published it is not built.
