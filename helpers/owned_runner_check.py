#!/usr/bin/env python3
"""A job that can land on an owned host runs only for the merge queue and `main`, and no workflow
that runs for a fork's pull request with base privileges checks out the fork's code.

    helpers/owned_runner_check.py              # script/lint: every rule, over .github/workflows/
    helpers/owned_runner_check.py FILE...      # the same rules over the named workflow files
    helpers/owned_runner_check.py --selftest

Name provisional (lane/owned-runners-canary, 2026-11-03 UTC). The runbook is
notes/owned-runners.md.

# WHY

The organization has owned runners (label `nife-owned-x86_64`). A pull request from a fork runs the
fork author's code, and code the author controls must never run on owned hardware (ruled
2026-11-03 UTC). The host admits only a merge group and a push to `main`, so a workflow that sends
another event there does not leak; it hangs the check and blocks the queue. This check is the rung
above that refusal: the tree fails to land before a job can wait on a runner that will never take
it, and before a workflow can hand a fork's code a base-repository token.

# THE RULES

An owned job is one whose `runs-on:`, `strategy:` (a matrix that feeds `runs-on`) or `with:` (an
input to a reusable workflow) names the label `nife-owned-x86_64` or reads the repository variable
`NIFE_RUNNER_OWNED`.

1. **An owned job's workflow lists no trigger but `merge_group` and `push`.** `pull_request`,
   `pull_request_target`, `workflow_run`, `workflow_dispatch`, `workflow_call`, `issue_comment`,
   `schedule` and every other event fail, in the mapping, the list or the scalar form. The whole
   file is judged, not the job: a job-level `if:` is code that a later edit can loosen, and the
   workflow's trigger is not.
2. **An owned job carries a job-level `if:` that names `merge_group` or `push` through
   `github.event_name` and does not name `pull_request`.** A job with no such guard has a `runs-on`
   that nothing but the trigger keeps off another event, and the first edit to the trigger removes
   that.
3. **One form is exempt from rules 1 and 2: the event-aware switch.** A `runs-on:` that is exactly
   `${{ vars.NIFE_RUNNER_OWNED == 'true' && (github.event_name == 'merge_group' || github.event_name == 'push') && 'nife-owned-x86_64' || 'ubuntu-NN.NN' }}`
   names the owned label only on a merge group or a push, and resolves to the hosted label on every
   other event, a pull request included. The event clause is inside `runs-on`, so the job needs no
   `if:` and its workflow may take other events. Any other shape is judged by rules 1 and 2.
4. **A `push` trigger in a workflow with an owned job names `branches: [main]` and no tags.** The
   switch and the guard both read `push` without its ref, and the host admits a push to `main`
   only, so a push to any other branch or a tag would hang the job.
5. **Every owned job sets `timeout-minutes`.** An owned slot holds a container until the job ends,
   and the host's own limit is longer than any job budget here.
6. **No job in a workflow that runs for `pull_request_target` or `workflow_run` names the pull
   request's head**: `github.event.pull_request.head.sha` or `.ref`, `github.head_ref`,
   `github.event.workflow_run.head_sha` or `.head_branch`, `refs/pull/`, or `gh pr checkout`. Those
   two events run with the base repository's token and secrets, and the head is the fork's code.
   This rule covers every runner, hosted included. It refuses the name, not only a checkout of it,
   because the name in an `env:` is one step away from a checkout.

The check reads text, not YAML, because a runner's Python and this Mac's both lack a YAML module.
It finds a job as a two-space key under `jobs:` and its fields as four-space keys, and a trigger as
a two-space key under `on:`.

# BUGS

- The guard is judged by the names in the `if:` text, not by evaluating it. An `if:` that mentions
  `merge_group` and is true for a pull request anyway (`merge_group || true`) passes.
- A reusable workflow that sets an owned `runs-on` inside the called file is judged when that file
  is checked, by rule 1, which refuses its `workflow_call` trigger. A caller that builds the label
  from a variable other than `NIFE_RUNNER_OWNED` is not seen.
- A matrix value is read as text: a matrix that names the owned label anywhere makes the job owned,
  and one that computes the label from another expression is not seen.
- Rule 6 reads names. A `pull_request_target` job that fetches the head by an expression it builds
  some other way passes.
- None of this binds a merge group, whose workflow files are the pull request's own: a pull request
  that edits a workflow runs its edit in the queue before this check can fail it. The barrier there
  is who may put a pull request in the queue (notes/owned-runners.md, "Who can reach the pool").
"""

import os
import re
import sys

WORKFLOW_DIR = ".github/workflows"
OWNED = re.compile(r"nife-owned-x86_64|NIFE_RUNNER_OWNED")
PULL_REQUEST = re.compile(r"\bpull_request(_target)?\b")
ALLOWED_TRIGGERS = {"merge_group", "push"}
HEAD_CODE_TRIGGERS = {"pull_request_target", "workflow_run"}
PULL_REQUEST_HEAD = re.compile(
    r"github\.event\.pull_request\.head\.(sha|ref)|github\.head_ref|"
    r"github\.event\.workflow_run\.head_(sha|branch)|refs/pull/|gh pr checkout")
TRIGGER_KEY = re.compile(r"^  (?:- )?([a-z_]+):?\s*(#.*)?$")
OWNED_FIELDS = ("runs-on", "strategy", "with")
EVENT_AWARE_SWITCH = re.compile(
    r"\$\{\{ vars\.NIFE_RUNNER_OWNED == 'true' && "
    r"\(github\.event_name == 'merge_group' \|\| github\.event_name == 'push'\) && "
    r"'nife-owned-x86_64' \|\| 'ubuntu-[0-9]+\.[0-9]+' \}\}")
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


def triggers(lines):
    """The event names the top-level `on:` lists, in the scalar, the list or the mapping form."""
    text = trigger_text(lines).split("\n")
    head = text[0].strip()
    if head.startswith("["):
        return {e.strip().strip("'\"") for e in head.strip("[]").split(",") if e.strip()}
    if head:
        return {head.strip("'\"")}
    return {m.group(1) for m in map(TRIGGER_KEY.match, text[1:]) if m}


def push_main_only(lines):
    """True when the workflow has no `push` trigger, or one limited to `branches: [main]` with no
    tags and no ignore list; a bare `push` in the scalar or list form runs for every branch."""
    events = triggers(lines)
    if "push" not in events:
        return True
    text = trigger_text(lines).split("\n")
    block = []
    inside = False
    for l in text[1:]:
        if re.match(r"^  push:", l):
            inside = True
            continue
        if inside:
            if l.strip() and not l.startswith("    "):
                break
            if l.strip():
                block.append(l.strip())
    if any(re.match(r"(tags|tags-ignore|branches-ignore):", b) for b in block):
        return False
    branches = None
    for i, b in enumerate(block):
        m = re.match(r"branches:\s*(.*)$", b)
        if not m:
            continue
        if m.group(1).startswith("["):
            branches = [x.strip().strip("'\"") for x in m.group(1).strip("[]").split(",")]
        else:
            branches = [x[1:].strip().strip("'\"") for x in block[i + 1:]
                        if x.startswith("- ")]
    return branches == ["main"]


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
    events = triggers(lines)
    for name, fields in jobs(lines):
        if events & HEAD_CODE_TRIGGERS and PULL_REQUEST_HEAD.search("\n".join(fields.values())):
            problems.append(
                f"{path}: job {name} names the pull request's head in a workflow that runs for "
                f"{' and '.join(sorted(events & HEAD_CODE_TRIGGERS))}, which holds the base "
                "repository's token while the head is the fork's code")
        if not any(OWNED.search(fields.get(f, "")) for f in OWNED_FIELDS):
            continue
        if "timeout-minutes" not in fields:
            problems.append(f"{path}: job {name} can run on an owned runner and sets no "
                            "timeout-minutes")
        if not push_main_only(lines):
            problems.append(
                f"{path}: job {name} can run on an owned runner, and the workflow's push trigger "
                "is not limited to branches: [main] (the host admits a push to main only)")
        if EVENT_AWARE_SWITCH.fullmatch(fields.get("runs-on", "").strip()):
            continue
        others = sorted(events - ALLOWED_TRIGGERS)
        if others:
            problems.append(
                f"{path}: job {name} resolves runs-on to an owned runner, and the workflow can "
                f"run for {', '.join(others)} (only merge_group and push may reach one)")
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
    timeout-minutes: 10
    steps:
      - run: true
  hosted:
    runs-on: ubuntu-26.04
"""

GOOD_PUSH_ONLY = """\
on:
  merge_group:
  push:
    branches: [main]
jobs:
  isolation:
    if: github.event_name == 'push'
    runs-on: [nife-owned-x86_64]
    timeout-minutes: 10
    steps:
      - run: true
"""

BAD_PULL_REQUEST = """\
on:
  pull_request:
  merge_group:
jobs:
  canary:
    if: github.event_name == 'merge_group'
    runs-on: nife-owned-x86_64
    timeout-minutes: 10
"""

BAD_SWITCH_ON_PULL_REQUEST = """\
on: [pull_request_target, merge_group]
jobs:
  build:
    if: github.event_name == 'merge_group'
    runs-on: ${{ vars.NIFE_RUNNER_OWNED == 'true' && 'nife-owned-x86_64' || 'ubuntu-26.04' }}
    timeout-minutes: 10
"""

BAD_NO_GUARD = """\
on:
  merge_group:
jobs:
  canary:
    runs-on: [nife-owned-x86_64]
    timeout-minutes: 10
"""

BAD_PULL_REQUEST_GUARD = """\
on:
  merge_group:
jobs:
  canary:
    if: github.event_name == 'merge_group' || github.event_name == 'pull_request'
    runs-on: [nife-owned-x86_64]
    timeout-minutes: 10
"""

GOOD_COMMENT_ONLY = """\
on:
  pull_request:
jobs:
  build:
    # runs-on: nife-owned-x86_64 would be refused here
    runs-on: ubuntu-26.04
"""

EVENT_AWARE = ("${{ vars.NIFE_RUNNER_OWNED == 'true' && (github.event_name == 'merge_group' || "
               "github.event_name == 'push') && 'nife-owned-x86_64' || 'ubuntu-26.04' }}")

GOOD_EVENT_AWARE = f"""\
on:
  pull_request:
  merge_group:
  push:
    branches: [main]
  workflow_dispatch:
jobs:
  bench:
    runs-on: {EVENT_AWARE}
    timeout-minutes: 25
"""

BAD_EVENT_AWARE_NAMES_PULL_REQUEST = GOOD_EVENT_AWARE.replace(
    "github.event_name == 'push'", "github.event_name == 'pull_request'")

BAD_EVENT_AWARE_NO_CLAUSE = """\
on:
  pull_request:
jobs:
  bench:
    runs-on: ${{ vars.NIFE_RUNNER_OWNED == 'true' && 'nife-owned-x86_64' || 'ubuntu-26.04' }}
    timeout-minutes: 25
"""

BAD_EVENT_AWARE_REGROUPED = GOOD_EVENT_AWARE.replace(
    "&& (github.event_name == 'merge_group' || github.event_name == 'push') &&",
    "&& github.event_name == 'merge_group' || github.event_name == 'push' &&")

BAD_EVENT_AWARE_OWNED_FALLBACK = GOOD_EVENT_AWARE.replace("'ubuntu-26.04'", "'nife-owned-x86_64'")

BAD_EVENT_AWARE_PUSH_ANY_BRANCH = GOOD_EVENT_AWARE.replace("  push:\n    branches: [main]\n",
                                                           "  push:\n")

BAD_EVENT_AWARE_NO_TIMEOUT = GOOD_EVENT_AWARE.replace("    timeout-minutes: 25\n", "")

GOOD_BLOCK_IF = """\
on: merge_group
jobs:
  canary:
    if: >-
      github.event_name == 'merge_group'
    runs-on:
      - nife-owned-x86_64
    timeout-minutes: 10
"""

GOOD_MATRIX_NEEDS = """\
on:
  merge_group:
  push:
    branches:
      - main
jobs:
  write:
    if: github.event_name == 'merge_group' || github.event_name == 'push'
    runs-on: [nife-owned-x86_64]
    timeout-minutes: 10
  check:
    needs: write
    if: github.event_name == 'merge_group' || github.event_name == 'push'
    runs-on: [nife-owned-x86_64]
    timeout-minutes: 10
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
    timeout-minutes: 10
  check:
    needs: write
    runs-on: [nife-owned-x86_64]
    timeout-minutes: 10
    strategy:
      matrix:
        instance: [1, 2]
"""

BAD_NO_TIMEOUT = GOOD.replace("    timeout-minutes: 10\n", "")

BAD_SCHEDULE = GOOD.replace("  merge_group:\n", "  merge_group:\n  schedule:\n    - cron: '0 8 * * *'\n")

BAD_WORKFLOW_DISPATCH = GOOD.replace("  merge_group:\n", "  merge_group:\n  workflow_dispatch:\n")

BAD_ISSUE_COMMENT = GOOD.replace("  merge_group:\n", "  merge_group:\n  issue_comment:\n"
                                 "    types: [created]\n")

BAD_WORKFLOW_CALL = GOOD.replace("  merge_group:\n", "  merge_group:\n  workflow_call:\n")

BAD_WORKFLOW_RUN_LIST = GOOD.replace(
    "on:\n  merge_group:\n  push:\n    branches: [main]\n", "on: [merge_group, workflow_run]\n")

BAD_PUSH_ANY_BRANCH_LIST = GOOD.replace(
    "on:\n  merge_group:\n  push:\n    branches: [main]\n", "on: [merge_group, push]\n")

BAD_PUSH_TAGS = GOOD.replace("    branches: [main]\n", "    branches: [main]\n    tags: ['v*']\n")

BAD_PUSH_OTHER_BRANCH = GOOD.replace("branches: [main]", "branches: [main, 'lane/**']")

BAD_MATRIX_RUNS_ON = """\
on:
  pull_request:
jobs:
  build:
    runs-on: ${{ matrix.runner }}
    timeout-minutes: 10
    strategy:
      matrix:
        runner: [ubuntu-26.04, nife-owned-x86_64]
"""

BAD_REUSABLE_CALLER = """\
on:
  pull_request:
jobs:
  build:
    uses: ./.github/workflows/build.yml
    with:
      runner: nife-owned-x86_64
"""

BAD_TARGET_CHECKOUT_HEAD = """\
on:
  pull_request_target:
jobs:
  label:
    runs-on: ubuntu-26.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
        with:
          ref: ${{ github.event.pull_request.head.sha }}
      - run: script/lint
"""

BAD_TARGET_GH_PR_CHECKOUT = """\
on: pull_request_target
jobs:
  label:
    runs-on: ubuntu-26.04
    steps:
      - run: gh pr checkout ${{ github.event.number }} && make
"""

BAD_WORKFLOW_RUN_HEAD = """\
on:
  workflow_run:
    workflows: [ci]
    types: [completed]
jobs:
  report:
    runs-on: ubuntu-26.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
        with:
          ref: ${{ github.event.workflow_run.head_sha }}
"""

GOOD_TARGET_BASE_ONLY = """\
on:
  pull_request_target:
    types: [labeled]
jobs:
  label:
    runs-on: ubuntu-26.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
      - run: helpers/label.sh "${{ github.event.pull_request.number }}"
"""

GOOD_PULL_REQUEST_HEAD = """\
on:
  pull_request:
jobs:
  check:
    runs-on: ubuntu-26.04
    steps:
      - run: echo "${{ github.event.pull_request.head.sha }}"
"""


def selftest():
    cases = [
        ("good", GOOD, 0),
        ("comment only", GOOD_COMMENT_ONLY, 0),
        ("owned job guarded by push alone", GOOD_PUSH_ONLY, 0),
        ("block scalar guard and list runs-on", GOOD_BLOCK_IF, 0),
        ("writer and matrix checker, both guarded, branches as a list", GOOD_MATRIX_NEEDS, 0),
        ("matrix checker with no guard", BAD_MATRIX_NO_GUARD, 1),
        ("owned label in a pull_request workflow", BAD_PULL_REQUEST, 1),
        ("switch expression in a pull_request_target workflow", BAD_SWITCH_ON_PULL_REQUEST, 1),
        ("event-aware switch in a pull_request and dispatch workflow", GOOD_EVENT_AWARE, 0),
        ("event-aware switch whose clause names pull_request",
         BAD_EVENT_AWARE_NAMES_PULL_REQUEST, 2),
        ("switch with no event clause", BAD_EVENT_AWARE_NO_CLAUSE, 2),
        ("event-aware switch with its parentheses removed", BAD_EVENT_AWARE_REGROUPED, 2),
        ("event-aware switch with an owned fallback", BAD_EVENT_AWARE_OWNED_FALLBACK, 2),
        ("event-aware switch under a push to any branch", BAD_EVENT_AWARE_PUSH_ANY_BRANCH, 1),
        ("event-aware switch with no timeout", BAD_EVENT_AWARE_NO_TIMEOUT, 1),
        ("owned label with no guard", BAD_NO_GUARD, 1),
        ("guard that names pull_request", BAD_PULL_REQUEST_GUARD, 1),
        ("owned job with no timeout", BAD_NO_TIMEOUT, 1),
        ("owned job under schedule", BAD_SCHEDULE, 1),
        ("owned job under workflow_dispatch", BAD_WORKFLOW_DISPATCH, 1),
        ("owned job under issue_comment", BAD_ISSUE_COMMENT, 1),
        ("owned job under workflow_call", BAD_WORKFLOW_CALL, 1),
        ("owned job under workflow_run, list form", BAD_WORKFLOW_RUN_LIST, 1),
        ("owned job under a bare push, list form", BAD_PUSH_ANY_BRANCH_LIST, 1),
        ("owned job under a push that takes tags", BAD_PUSH_TAGS, 1),
        ("owned job under a push to another branch", BAD_PUSH_OTHER_BRANCH, 1),
        ("owned label in a matrix that feeds runs-on", BAD_MATRIX_RUNS_ON, 2),
        ("owned label passed to a reusable workflow", BAD_REUSABLE_CALLER, 3),
        ("pull_request_target checks out the head", BAD_TARGET_CHECKOUT_HEAD, 1),
        ("pull_request_target runs gh pr checkout", BAD_TARGET_GH_PR_CHECKOUT, 1),
        ("workflow_run checks out the head", BAD_WORKFLOW_RUN_HEAD, 1),
        ("pull_request_target on the base only", GOOD_TARGET_BASE_ONLY, 0),
        ("pull_request names its own head", GOOD_PULL_REQUEST_HEAD, 0),
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
