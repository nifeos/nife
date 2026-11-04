#!/usr/bin/env python3
"""Does this run take the heavy gates, or only the cheap set a pull request runs?

    helpers/full_ci.py decide       # a workflow's draft gate: writes heavy=true|false
    helpers/full_ci.py --selftest   # script/lint: fixtures, no network

Name: provisional, minted by lane/heavy-gates-only-in-queue on 2026-11-03 UTC. A helper under
`helpers/`, outside `script/names`' scope, so its provenance is this paragraph. Named for the
`full-ci` label it reads.

# THE RULE

The heavy gates run only in the merge queue (provisional, ruled 2026-11-03 UTC). A pull request
runs the cheap set: rustfmt, the full script/lint with its clippy and per-target builds, the
host tests, the mutation gate and the short build checks. The kernel suites under QEMU, the Kani proofs and
re-falsification, fuzzing, the icount bench and the crate probes run in the merge group, and on a
push to `main` that no merge group tested. notes/merge-queue.md lists each job and its side.

`heavy` is false in exactly two cases, and true in every other:

- a `pull_request` run whose pull request does not carry the `full-ci` label right now, read live
  from the API as the draft gate reads the draft flag (the payload's labels are a snapshot);
- a `workflow_dispatch` run started with `full_ci` set to false.

A label read that fails answers true, so the gate fails toward running, as the draft gate does. A
merge group, a push and any other event answer true without looking. The workflows test the event
name beside this output as well, so a merge group runs the heavy set even if this script is wrong.

# WHY A JOB-LEVEL SKIP

A job skipped by its `if:` reports Success under its own name, which is how a pull request
satisfies the ruleset's required checks without running them (ci.yml's gate comment has the
documentation). The merge group then runs the same names unskipped, and the queue merges only on
that run's results, so a skip on the pull request cannot hide a failure from the merge.

# BUGS

The label read costs one API call per gate, three gates per pull request run. Adding the label
starts no run by itself; `.github/workflows/full-ci-label.yml` dispatches one.
"""

import json
import os
import subprocess
import sys

LABEL = "full-ci"


def decide(event, full_ci_input, labels):
    """heavy for one run: `labels` is the live label list, or None when the read failed."""
    if event == "workflow_dispatch":
        return full_ci_input != "false"
    if event != "pull_request":
        return True
    if labels is None:
        return True
    return LABEL in labels


def live_labels(repository, number):
    """The pull request's labels right now, or None when `gh` cannot say."""
    if not number:
        return None
    try:
        out = subprocess.run(
            ["gh", "pr", "view", number, "--repo", repository, "--json", "labels"],
            capture_output=True, text=True, timeout=60, check=True,
        ).stdout
        return [label["name"] for label in json.loads(out)["labels"]]
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, TypeError):
        return None


def main_decide():
    event = os.environ.get("EVENT_NAME", "")
    labels = None
    if event == "pull_request":
        labels = live_labels(os.environ.get("GITHUB_REPOSITORY", ""), os.environ.get("PR_NUMBER", ""))
    heavy = decide(event, os.environ.get("FULL_CI_INPUT", ""), labels)
    if event == "pull_request":
        seen = "unreadable" if labels is None else (", ".join(labels) or "none")
        print(f"==> labels: {seen}")
    word = "true" if heavy else "false"
    print(f"==> heavy gates: {'run' if heavy else 'skipped; the merge queue runs them'} (heavy={word})")
    path = os.environ.get("GITHUB_OUTPUT")
    if path:
        with open(path, "a", encoding="utf-8") as out:
            out.write(f"heavy={word}\n")
    return 0


def selftest():
    cases = [
        ("a merge group runs the heavy set", decide("merge_group", "", None) is True),
        ("a push runs the heavy set", decide("push", "", None) is True),
        ("a pull request without the label skips it", decide("pull_request", "", ["debt-paydown"]) is False),
        ("a pull request with no labels skips it", decide("pull_request", "", []) is False),
        ("the full-ci label runs it", decide("pull_request", "", ["debt-paydown", LABEL]) is True),
        ("an unreadable label list runs it", decide("pull_request", "", None) is True),
        ("a dispatch runs it by default", decide("workflow_dispatch", "", None) is True),
        ("a dispatch with full_ci=true runs it", decide("workflow_dispatch", "true", None) is True),
        ("a dispatch with full_ci=false skips it", decide("workflow_dispatch", "false", None) is False),
        ("a dispatch ignores labels", decide("workflow_dispatch", "false", [LABEL]) is False),
        ("a label read with no number is unreadable", live_labels("o/r", "") is None),
    ]
    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f"full_ci selftest FAILED: {name}", file=sys.stderr)
    print(f"full_ci selftest: {len(cases) - len(failed)}/{len(cases)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    args = sys.argv[1:]
    if args == ["--selftest"]:
        sys.exit(selftest())
    if args == ["decide"]:
        sys.exit(main_decide())
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    sys.exit(2)
