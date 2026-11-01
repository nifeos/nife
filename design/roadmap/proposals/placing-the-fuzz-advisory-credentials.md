---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Placing the fuzz advisory credentials

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from the ruling recorded in
[fuzz-findings-without-a-person](fuzz-findings-without-a-person.md): a person places one credential
once, and no lane handles it.

## What it builds

A runbook for a person, `notes/fuzzing/advisory-credentials.md`, in the style of
`notes/automation-identity.md`: numbered steps a person follows in a browser and on the host.

- Create or narrow the filing App: repository permission "Repository security advisories", read and
  write, and nothing else beyond Metadata; installed on nifeos/nife alone.
- Place its private key on the fuzz host, readable only by the fuzz account, at the path the filing
  service reads, then enable the service's timer.
- Create the read-only credential for fix lanes (Repository security advisories, read only, nifeos/nife
  alone) and place it in the lane environment.
- Say plainly that the host runs main's code unattended (the fuzz host follows main), so anything
  merged to main runs within one window as the account that holds the filing key. The key's power is
  the advisory permission alone, which bounds what a bad merge can do with it.
- How to rotate or revoke either credential, and how to confirm the service works: a dry run, then
  the first real filing.

No lane writes a key, a token or a path that holds one into the tree, and no lane runs these steps.

## How it is checked

`script/lint`'s link and prose checks. A person following it end to end is the real check, and the
note says so in its `BUGS`.

## Status

Not started. It waits on an architect's answer to which App files the advisories and what form the
read-only credential takes.
