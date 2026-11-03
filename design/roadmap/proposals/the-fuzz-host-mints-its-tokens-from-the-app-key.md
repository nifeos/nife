---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The fuzz host mints its tokens from the App key

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from the credential ruling recorded
in [fuzz-findings-without-a-person](fuzz-findings-without-a-person.md).

Reuse: `helpers/smelter-token` already signs a JSON Web Token with `openssl` and trades it for an
installation token. This takes its signing and exchange code, not its keychain or session parts.

## What it builds

- A mint helper that runs as root. It reads the `nife-fuzz` App's key from its root-only file and
  signs a JSON Web Token. It trades that for an installation token narrowed to nifeos/nife, Contents
  write and Repository security advisories write.
- The helper writes the token where only the fuzz account can read it, and the token lives one hour.
  The key itself never leaves the helper's memory.
- A person installs the helper outside the clone, as with the unit files, so the host's update from
  main never replaces code that runs as root.
- `helpers/fuzz-daily-publish.sh --publish` reads that token as `GH_TOKEN`. It refuses by name when
  the token is missing or stale. The filing and publishing services read the same token.

## How it is checked

A selftest runs the helper against a throwaway key it generates and a stub `curl`, with no network.
It checks that the JSON Web Token verifies against the key's public half, and that the exchange asks
for exactly the two permissions and the one repository. It also checks that the token file has the
right mode and that nothing printed contains the key.

## Status

Built as `fuzz-token` in nifeos/fuzz-host@0c85820b2ff280c993613cbc66f95f71548ea229
([the repository](https://github.com/nifeos/fuzz-host)), with its selftest,
unit, timer and install section. It narrows each token further than this proposal asked: one token
per service, Contents write alone for the daily release and Repository security advisories write
alone for the filer. The token file sits under `/run/fuzz-token/`, and systemd's `EnvironmentFile=`
reads it as root before the service drops to the fuzz account, so no third account sits between
them. Not installed on the host. `helpers/fuzz-daily-publish.sh` does not yet refuse a stale token
by name. App keys do not expire, and installation tokens last one hour (GitHub's documentation, read
2026-11-01 UTC).
