---
status: NOT-STARTED
promoted_from: the-fuzz-host-files-held-findings-as-advisories
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1020. The fuzz host files each held finding as an advisory

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from step 1 of the ruling recorded
in [fuzz-findings-without-a-person](1015-fuzz-findings-without-a-person.md).

Reuse: the mint helper `fuzz-token` in nifeos/fuzz-host signs the App's token; the filer reads the
token file it writes and calls GitHub's repository security advisories API. No new dependency.

## What it builds

A plain service on the fuzz host, a Python program with a systemd timer, files one private security advisory on nifeos/nife for each held
window that has none yet. It is a program, not an agent or a language model.

- It reads the held directory, which it alone of the repository's programs may open, and
  `windows.tsv` for the window's driver, first seed, commit and status.
- It takes the replay seed from the held log's `replay:` line. For a crash or hang it takes the
  window's first seed and count instead.
- It files each advisory with a package (ecosystem and name) and a `vulnerable_version_range` that it
  can supply, and leaves `patched_versions` unset, because no fix is known at filing. GitHub refuses to publish one without a range (422,
  "Advisory is not publishable"), and the publish proposal depends on it.
- The advisory body holds the class (`held-finding`, `held-crash` or `held-hang`), the driver, the
  commit, the seed or range, and the one-line replay command.
- It records the advisory id in a ledger, `/var/lib/fuzz-filer/ledger.tsv`, so a second run files
  nothing twice. The ledger sits outside the held directory, because the daily report counts every
  file under that directory as a finding.
- It reads the one-hour installation token of the `nife-fuzz` App that the mint helper writes,
  and never the key.
- It never writes a seed, input or advisory body to standard output, standard error or the journal.
  Its log lines name the held file's age and the advisory id only.

## How it is checked

A selftest, `tests/fuzz-filer-selftest` in nifeos/fuzz-host, runs the service against a stub GitHub
on a loopback address and a synthetic held directory, with no network. It plants three fake held windows, one of each class,
and checks that the stub received three create calls with the expected fields. A second run must
send none, nothing it prints may contain a planted seed, and it must refuse to run without a token. `script/lint` runs it.

## Status

Built as `fuzz-filer` in nifeos/fuzz-host, with its install steps in that repository's
`docs/fuzz-filer.md`. It departs from the text above in four ways: it is Python, not shell; the
ledger lives outside the held directory; `patched_versions` is not set at filing; and the code is in
nifeos/fuzz-host, not here. An architect ruled on 2026-11-03 (UTC) that it runs as `lane`, the
account that also runs the fuzzed code, and that it uses one-hour installation tokens from a root
timer with no long-lived credential. The accepted risk is that code reaching main runs as an account
that can read the filer token. Unverified: that GitHub accepts `<= <commit>` as a range for the
ecosystem `other`, that the replay command runs, and that 1000000 is the seed count of a crash or
hang window. A test showed that an App with the advisory permission alone can create, complete,
publish and publicly read an advisory
([the result](../../notes/fuzzing/finishing-the-setup.md#the-advisory-permission)).

## Index row

A plain service on the fuzz host files one private security advisory per held window, with the range GitHub needs to publish it later. A held finding is no longer a log file only a person can find.
