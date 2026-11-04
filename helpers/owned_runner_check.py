#!/usr/bin/env python3
"""A job that can run for a pull request never lands on an owned host, and a job that does land on
one is guarded to the merge queue and `main`.

    helpers/owned_runner_check.py              # script/lint: both rules, over .github/workflows/
    helpers/owned_runner_check.py FILE...      # the same rules over the named workflow files
    helpers/owned_runner_check.py --selftest

Name provisional (lane/owned-runners-canary, 2026-11-03 UTC). The runbook is
notes/owned-runners.md.

# WHY

The organization has owned runners (label `nife-owned-x86_64`). A pull request from a fork runs the
fork author's code, and code the author controls must never run on owned hardware (ruled
2026-11-03 UTC). The host refuses a `pull_request` job at its own door, so a workflow that sends
one there does not leak; it hangs the check and blocks the queue. This check is the rung above that
refusal: the tree fails to land before a pull request can wait on a runner that will never take it.

# THE RULES

An owned reference is a `runs-on:` value that names the label `nife-owned-x86_64` or reads the
repository variable `NIFE_RUNNER_OWNED`. The switch expression names the label, so one match covers
both.

1. **No owned reference in a workflow that can run for a pull request.** A workflow can run for one
   when its `on:` lists `pull_request` or `pull_request_target`, in the mapping, the list or the
   scalar form. The whole file is judged, not the job: a job-level `if:` is code that a later edit
   can loosen, and the workflow's trigger is not.
2. **Every job with an owned reference carries a job-level `if:` that names `merge_group` or `push`
   through `github.event_name` and does not name `pull_request`.** A job with no such guard has a
   `runs-on` that nothing but the trigger keeps off a pull request, and the first edit to the
   trigger removes that.

The check reads text, not YAML, because a runner's Python and this Mac's both lack a YAML module.
It finds a job as a two-space key under `jobs:` and its fields as four-space keys.

# BUGS

- The guard is judged by the names in the `if:` text, not by evaluating it. An `if:` that mentions
  `merge_group` and is true for a pull request anyway (`merge_group || true`) passes.
- A reusable workflow (`uses:` at job level) that sets `runs-on` inside the called file is judged
  when that file is checked, and a caller that passes the label in as an input is not seen at all.
  No workflow here does either.
- A `runs-on` built in a matrix (`${{ matrix.runner }}`) is not resolved; the matrix values are
  not read. The check refuses nothing it cannot see.
"""

import os
import re
import sys

WORKFLOW_DIR = ".github/workflows"
OWNED = re.compile(r"nife-owned-x86_64|NIFE_RUNNER_OWNED")
PULL_REQUEST = re.compile(r"\bpull_request(_target)?\b")
JOB_KEY = re.compile(r"^  ([A-Za-z0-9_-]+):\s*(#.*)?$")
FIELD_KEY = re.compile(r"^    ([A-Za-z0-9_-]+):(.*)$")


def code_lines(text):
    """The file's lines with whole-line comments blanked, so a comment never decides a verdict."""
    return ["" if l.lstrip().startswith("#") else l for l in text.split("\n")]


def trigger_text(lines):
    """The text of the top-level `on:` entry: the key's own line and its more-indented lines."""
    out = []
    on = False
    for l in lines:
        if re.match(r"^(on|\"on\"|'on'):", l):
            on = True
            out.append(l.split(":", 1)[1])
            continue
        if on:
            if l.strip() and not l.startswith((" ", "\t")):
                break
            out.append(l)
    return "\n".join(out)


def jobs(lines):
    """Each job as (name, {field: text}); a field's text is its line's value plus its more-indented
    continuation lines, so a block scalar or a list is read whole."""
    result = []
    in_jobs = False
    current = None
    field = None
    for l in lines:
        if l.startswith("jobs:"):
            in_jobs = True
            continue
        if not in_jobs:
            continue
        if l.strip() and not l.startswith(" "):
            break
        j = JOB_KEY.match(l)
        if j:
            current = (j.group(1), {})
            result.append(current)
            field = None
            continue
        if current is None:
            continue
        f = FIELD_KEY.match(l)
        if f:
            field = f.group(1)
            current[1][field] = f.group(2)
        elif field is not None and l.strip():
            current[1][field] += "\n" + l
    return result


def guarded(condition):
    if condition is None:
        return False
    names = re.findall(r"github\.event_name\s*==\s*'([a-z_]+)'", condition)
    return bool(names) and set(names) <= {"merge_group", "push"} and \
        not PULL_REQUEST.search(condition)


def check(path, text):
    """The problems in one workflow file, as strings; empty when it passes."""
    lines = code_lines(text)
    problems = []
    for name, fields in jobs(lines):
        owned = OWNED.search(fields.get("runs-on", ""))
        if not owned:
            continue
        if PULL_REQUEST.search(trigger_text(lines)):
            problems.append(
                f"{path}: job {name} resolves runs-on to an owned runner, and the workflow can "
                "run for a pull request (on: names pull_request or pull_request_target)")
        if not guarded(fields.get("if")):
            problems.append(
                f"{path}: job {name} resolves runs-on to an owned runner and has no job-level "
                "`if:` that names only github.event_name == 'merge_group' or 'push'")
    return problems


GOOD = """\
on:
  merge_group:
  push:
    branches: [main]
jobs:
  canary:
    if: github.event_name == 'merge_group' || github.event_name == 'push'
    runs-on: [nife-owned-x86_64]
    steps:
      - run: true
  hosted:
    runs-on: ubuntu-26.04
"""

BAD_PULL_REQUEST = """\
on:
  pull_request:
  merge_group:
jobs:
  canary:
    if: github.event_name == 'merge_group'
    runs-on: nife-owned-x86_64
"""

BAD_SWITCH_ON_PULL_REQUEST = """\
on: [pull_request_target, merge_group]
jobs:
  build:
    if: github.event_name == 'merge_group'
    runs-on: ${{ vars.NIFE_RUNNER_OWNED == 'true' && 'nife-owned-x86_64' || 'ubuntu-26.04' }}
"""

BAD_NO_GUARD = """\
on:
  merge_group:
jobs:
  canary:
    runs-on: [nife-owned-x86_64]
"""

BAD_PULL_REQUEST_GUARD = """\
on:
  merge_group:
jobs:
  canary:
    if: github.event_name == 'merge_group' || github.event_name == 'pull_request'
    runs-on: [nife-owned-x86_64]
"""

GOOD_COMMENT_ONLY = """\
on:
  pull_request:
jobs:
  build:
    # runs-on: nife-owned-x86_64 would be refused here
    runs-on: ubuntu-26.04
"""

GOOD_BLOCK_IF = """\
on: merge_group
jobs:
  canary:
    if: >-
      github.event_name == 'merge_group'
    runs-on:
      - nife-owned-x86_64
"""

GOOD_MATRIX_NEEDS = """\
on:
  merge_group:
  push:
jobs:
  write:
    if: github.event_name == 'merge_group' || github.event_name == 'push'
    runs-on: [nife-owned-x86_64]
  check:
    needs: write
    if: github.event_name == 'merge_group' || github.event_name == 'push'
    runs-on: [nife-owned-x86_64]
    strategy:
      fail-fast: false
      matrix:
        instance: [1, 2, 3, 4, 5]
"""

BAD_MATRIX_NO_GUARD = """\
on:
  merge_group:
jobs:
  write:
    if: github.event_name == 'merge_group'
    runs-on: [nife-owned-x86_64]
  check:
    needs: write
    runs-on: [nife-owned-x86_64]
    strategy:
      matrix:
        instance: [1, 2]
"""


def selftest():
    cases = [
        ("good", GOOD, 0),
        ("comment only", GOOD_COMMENT_ONLY, 0),
        ("block scalar guard and list runs-on", GOOD_BLOCK_IF, 0),
        ("writer and matrix checker, both guarded", GOOD_MATRIX_NEEDS, 0),
        ("matrix checker with no guard", BAD_MATRIX_NO_GUARD, 1),
        ("owned label in a pull_request workflow", BAD_PULL_REQUEST, 1),
        ("switch expression in a pull_request_target workflow", BAD_SWITCH_ON_PULL_REQUEST, 1),
        ("owned label with no guard", BAD_NO_GUARD, 1),
        ("guard that names pull_request", BAD_PULL_REQUEST_GUARD, 1),
    ]
    failed = 0
    for label, text, want in cases:
        got = len(check("fixture.yml", text))
        if got != want:
            print(f"owned_runner_check --selftest: {label}: expected {want} problem(s), got {got}",
                  file=sys.stderr)
            failed += 1
    return 1 if failed else 0


def main(argv):
    if argv == ["--selftest"]:
        return selftest()
    paths = argv or sorted(
        os.path.join(WORKFLOW_DIR, f) for f in os.listdir(WORKFLOW_DIR) if f.endswith(".yml"))
    problems = []
    for p in paths:
        with open(p, encoding="utf-8") as fh:
            problems += check(p, fh.read())
    for p in problems:
        print(p, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
