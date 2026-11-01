---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The fuzz host files each held finding as an advisory

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from step 1 of the ruling recorded
in [fuzz-findings-without-a-person](fuzz-findings-without-a-person.md).

Reuse: `curl` and `openssl` sign the App's token and call GitHub's repository security advisories
API, as `helpers/smelter-token` already signs one for the `smelter` App. No new dependency.

## What it builds

A plain service on the fuzz host, a shell script with a systemd timer beside
`helpers/continuous-fuzz.sh`, files one private security advisory on nifeos/nife for each held
window that has none yet. It is a program, not an agent or a language model.

- It reads the held directory, which it alone of the repository's programs may open, and
  `windows.tsv` for the window's driver, first seed, commit and status.
- It takes the replay seed from the held log's `replay:` line, or for a crash or hang the window's
  first seed and count, and writes an advisory body holding the class (`held-finding`, `held-crash`,
  `held-hang`), the driver, the commit, the seed or range, and the one-line replay command.
- It records the advisory id beside the held log, in a file under the held directory, so a second
  run files nothing twice.
- It mints a one-hour installation token from the filing App's key for each run, narrowed to
  nifeos/nife and the advisory permission, and keeps it in memory only.
- It never writes a seed, input or advisory body to standard output, standard error or the journal.
  Its log lines name the held file's age and the advisory id only.

## How it is checked

A selftest in the style of `helpers/continuous-fuzz-selftest.sh` runs the service in dry-run against
a stub `curl` and a stub key, with no network. It plants three fake held windows (one of each class)
and checks that the stub received three create calls with the expected fields, that a second run
sends none, that nothing it printed contains a planted seed, and that it refuses to run without the
key file. `script/lint` runs it.

## Status

Not started. Unverified: whether an App holding only the advisory permission may create an
advisory, since the API names "a security manager or administrator" as the caller. The first real
filing settles it, after a person places the key
([placing the credentials](placing-the-fuzz-advisory-credentials.md)). The App's name and whether it
is a new App are open questions for an architect.
