---
status: NOT-STARTED
raised: 2026-10-08
promoted_from: every-workflow-file-parses
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 861. Every workflow file parses

*(The title and slug are provisional.)*

`script/lint` reads workflows line by line and never parses one. A comment line that loses its `#`
prefix to a line-wrap slip passes every text gate: the pin and swallow checks match their regexes,
and the pre-push hook waves it through. Only a YAML parse (`ruby -ryaml`) catches it.

## What almost happened

GitHub rejects a workflow file that does not parse: the run never starts, and the workflow's page
shows an invalid-file error instead of a check. For `ci.yml` and `verify.yml`, whose checks the
merge queue requires, that is every pull request blocked on a pending check that nothing can
post, which is the same failure shape `notes/repo-hardening.md` records for a required check that
never reports. The lane would have learned it from a runner cycle or from calef, not from a gate.

## What is proposed

A `script/lint` section that parses every file under `.github/workflows/` and fails naming the
file and the parser's error. The open fork, for whoever builds it, is the parser:

- A YAML library. `helpers/workflow_swallows.py`'s header records why the tree has avoided one:
  the runner image and the developer machine do not agree on one. If one is present in both (the
  ubuntu-24.04 image and macOS both ship Ruby's stdlib YAML), that is the honest answer.
- An indentation-aware structural parse, like `helpers/job-budget.py`'s `jobs_of`, catches a
  plain line at the wrong indent (this incident's shape) but not every YAML error. Cheaper, weaker,
  and it fails loud rather than not at all.

Either way the check is text-only, sits in `--no-cargo`, and costs milliseconds.

Reuse: the check's shape is `helpers/workflow_swallows.py`'s section in `script/lint` (selftest
first, then the tree); the parser itself is the fork above, and no existing tree helper parses
YAML (searched `helpers/` and `script/`, 2026-10-08).

## Index row

Every workflow file parses: a `script/lint` section that parses each file under `.github/workflows/` and fails naming the file and the parser's error. A lane nearly shipped a workflow whose comment line had lost its `#` and every text gate passed. Text-only, in `--no-cargo`, milliseconds.
