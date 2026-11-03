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

Reuse: `fuzz-token` in nifeos/fuzz-host already signs a JSON Web Token and trades it for an
installation token
([the fuzz host mints its tokens](the-fuzz-host-mints-its-tokens-from-the-app-key.md)); the reader
is its third profile, `reader`, so the two Apps share one signing routine.

## What it builds

- A helper that runs as root on the machine that launches fix lanes. It reads the
  `nife-fuzz-reader` key from its root-only file and mints an installation token narrowed to
  nifeos/nife and Repository security advisories read.
- A lane asks the helper for one token, through a sudo rule, when it reads an advisory. It passes the
  token to the advisory-reading script alone, in that one command. The token never enters the lane's general environment and never
  lands in a worktree, a log or a pull request.
- The key never leaves the helper's memory. A lane cannot ask for the key, only for a token.
- A token lives one hour. A lane that needs the advisory again asks again.

## How it is checked

A selftest runs the helper against a throwaway key and a stub GitHub on a loopback address, with no
network. The exchange must ask
for exactly the read permission and the one repository. The output must hold a token and never the
key. The helper must refuse when the key file is missing or readable by anyone but root.

## Status

Built as `issue reader` in `fuzz-token` (nifeos/fuzz-host, pull request 4), with install steps in
that repository's `docs/fuzz-token-reader.md`. The sudo question is answered provisionally: an
architect ruled on 2026-11-03 (UTC) that a sudo rule names exactly `fuzz-token issue reader`, and a
lane takes the token for one command:

```sh
NIFE_ADVISORY_READER_TOKEN="$(sudo -n /usr/local/sbin/fuzz-token issue reader)" script/read-advisory <id>
```

The same ruling lets any lane on the machine take a reader token, since every lane runs as one
account. The protections are the fix-lane skill, the replay-line check and sudo's log, and the
account split is revisited after the first fix lane. Unverified: that sudo-rs matches the rule's
arguments exactly and that each sudo hands the helper a pipe, not a terminal, for standard output;
no one has run the rule yet.
