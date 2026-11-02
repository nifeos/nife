---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# Fix lanes get a one-hour reader token

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from the reader App ruling
recorded in [fuzz-findings-without-a-person](fuzz-findings-without-a-person.md).

Reuse: `helpers/smelter-token` already signs a JSON Web Token and trades it for an installation
token; this takes that code. The mint helper on the fuzz host
([the fuzz host mints its tokens](the-fuzz-host-mints-its-tokens-from-the-app-key.md)) does the same
for the other App, and the two should share one signing routine.

## What it builds

- A helper that runs as root on the machine that launches fix lanes. It reads the
  `nife-fuzz-reader` key from its root-only file and mints an installation token narrowed to
  nifeos/nife and Repository security advisories read.
- A lane's launch asks the helper for one token. It passes the token to the advisory-reading script
  alone, as `GH_TOKEN` in that one command. The token never enters the lane's general environment and never
  lands in a worktree, a log or a pull request.
- The key never leaves the helper's memory. A lane cannot ask for the key, only for a token.
- A token lives one hour. A lane that needs the advisory again asks again.

## How it is checked

A selftest runs the helper against a throwaway key and a stub `curl`, with no network. The exchange must ask
for exactly the read permission and the one repository. The output must hold a token and never the
key. The helper must refuse when the key file is missing or readable by anyone but root.

## Status

Not started. Unverified: how a lane launcher on a machine without a root daemon reaches a root-only
helper. A narrow sudo rule for the one helper is the likely form, and the milestone measures and
records it.
