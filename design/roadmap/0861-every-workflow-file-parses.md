---
status: BUILT
raised: 2026-10-08
built: 2026-10-16
promoted_from: every-workflow-file-parses
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 861. Every workflow file parses

*(The title and slug are provisional, and so is the helper's name.)*

## The incident

`script/lint` read workflows line by line and never parsed one. A comment line that loses its
`#` prefix to a line-wrap slip passes every text gate: the pin and swallow checks match their
regexes, and the pre-push hook waves it through. GitHub does not: the file never starts a run,
and for ci.yml and verify.yml that is every pull request blocked on a required check nothing can
post. It is the same failure shape notes/repo-hardening.md records for a required check that
never reports.

## What was built

- `helpers/workflow_parses.py` and its `script/lint` section: every file under
  `.github/workflows/` is parsed, and a failure names the file and the parser's error. The
  selftest runs first, in `helpers/workflow_swallows.py`'s section's shape: the incident's own
  fixture, a tab in the indentation, an ordinary file, and the empty directory that must fail
  loud (milestone 401 (a gate that selects the set it judges)). Text-only, in `--no-cargo`, one
  process for the directory, milliseconds.
- The parser is `ruby -ryaml` (Ruby's stdlib YAML, Psych). `script/bootstrap` installs ruby where
  `ruby -ryaml` fails, and the `lint` job in `ci.yml` installs it explicitly. macOS ships
  `/usr/bin/ruby`, which parses the tree's 25 workflow files clean, and the ubuntu-24.04 runner
  images list Ruby 3.2.3 (the amd64 and arm64 readmes in actions/runner-images, read 2026-10-15;
  arm64 is what ci.yml's jobs run on). No structural or
  regex parse of workflow files exists in the tree beside it.

Reuse: the check's shape is `helpers/workflow_swallows.py`'s section in `script/lint` (selftest
first, then the tree). The parser took Ruby's stdlib over a structural parse written here or a
third-party library; no tree helper parsed YAML (searched `helpers/` and `script/`, 2026-10-08).

## Index row

Every workflow file parses: a `script/lint` section parses each file under `.github/workflows/`
under Ruby's stdlib YAML and fails naming the file and the parser's error. A workflow a
comment-line slip broke then stops a push instead of stranding every pull request on a required
check that never reports. Text-only, in `--no-cargo`, milliseconds.

## BUGS

- This proves each file is YAML, not that it is a valid workflow. GitHub's schema is not
  validated: a misspelled key that stays well-formed (`runs-o:`) parses, and stays the pin,
  swallow and budget checks' business.
- Psych is YAML 1.1 and GitHub parses with its own, so the gate and GitHub can disagree in
  either direction in principle. The incident shape is rejected by both; a file this passes and
  GitHub rejects is the residue.
- A machine without `ruby` on PATH fails the check naming that rather than degrading to a
  weaker parse, so the gate is down there until `script/bootstrap` (or a hand install) puts ruby
  there. That is deliberate: a quiet
  degradation is the failure this check exists to end.
- A lost `#` inside a `run: |` block edits the script's text and stays valid YAML. Nothing that
  reads structure can see it.
- Only `.github/workflows/` is walked. `.github/actions/` (composite actions) holds nothing
  today; if one appears, its files do not parse through this check.

## Follow-on

- **None.** Nothing is queued behind this milestone and nothing was blocked by it. The
  composite-action gap above is the one piece that wants a lane if `.github/actions/` ever
  appears; it would ride the same helper.
