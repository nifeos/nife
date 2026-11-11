#!/bin/sh
#
# helpers/draft-lint-selftest.sh: a draft's `lint` reports failure, never skipped, and a ready pull
# request's `lint` runs the real lint (the architect's ruling of 2026-11-11 (UTC) on question B of
# the proposal "lint fails in the queue after passing on the pull request"). Provisional name, minted
# by lane maintainer/queue-entry-lint-guards.
#
# GitHub Actions cannot run here, so this reads .github/workflows/ci.yml and takes the two halves
# separately. First it runs the draft gate's real `state` step under a stub `gh` that answers
# isDraft, and reads what it wrote to GITHUB_OUTPUT. Then it evaluates the `lint` job's own `if:`
# expressions against those outputs with a small evaluator for the operators this file uses, and
# runs the draft step's script. A step with no `if:` runs only while every earlier step succeeded,
# which is GitHub's rule, so after the draft step fails the real lint cannot start.

here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
root="$(dirname "$here")"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/draft-lint-selftest.XXXXXX")" || exit 1
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"
printf '#!/bin/sh\necho "$STUB_DRAFT"\n' >"$tmp/bin/gh"
chmod +x "$tmp/bin/gh"

PATH="$tmp/bin:$PATH" TMP="$tmp" exec python3 - "$root/.github/workflows/ci.yml" <<'PY'
import os, re, subprocess, sys
import yaml

me = "draft-lint-selftest.sh"
wf = yaml.safe_load(open(sys.argv[1]))
gate, lint = wf["jobs"]["gate"], wf["jobs"]["lint"]
failed = []


def check(what, ok):
    if not ok:
        failed.append(what)


def gate_outputs(event, draft):
    """Run the gate's `state` step as GitHub would for this event; return its outputs."""
    step = next(s for s in gate["steps"] if s.get("id") == "state")
    script = step["run"].replace("${{ github.event_name }}", event).replace("${{ github.sha }}", "abc")
    out = os.path.join(os.environ["TMP"], "output")
    open(out, "w").close()
    env = dict(os.environ, GITHUB_OUTPUT=out, PR_NUMBER="42", GITHUB_REPOSITORY="o/r",
               STUB_DRAFT="true" if draft else "false")
    subprocess.run(["bash", "-e", "-c", script], env=env, capture_output=True, check=True)
    vals = dict(l.split("=", 1) for l in open(out).read().split() if "=" in l)
    # A job output maps a step output; an unset one reads as the empty string.
    return {k: vals.get(re.search(r"steps\.state\.outputs\.(\w+)", v).group(1), "") if "steps.state" in v else ""
            for k, v in gate["outputs"].items()}


def evaluate(expr, ctx, always_ok):
    """GitHub's expression subset this job uses: ==, !=, &&, ||, !, parentheses, string literals,
    needs.gate.outputs.*, github.event_name, steps.*.outputs.* (empty here), and the status
    functions. `always_ok` is whether a status function would be true (always() is; success() is
    not after a failed step)."""
    if expr is None:
        return always_ok == "success"
    e = expr.strip()
    if e.startswith("${{"):
        e = e[3:-2].strip()
    e = re.sub(r"hashFiles\([^)]*\)", "''", e)
    e = re.sub(r"!cancelled\(\)", "True", e)
    e = re.sub(r"always\(\)", "True", e)
    e = re.sub(r"success\(\)", "True" if always_ok == "success" else "False", e)
    e = re.sub(r"needs\.gate\.outputs\.(\w+)", lambda m: repr(ctx["gate"].get(m.group(1), "")), e)
    e = e.replace("github.event_name", repr(ctx["event"]))
    e = re.sub(r"steps\.[\w-]+\.outputs\.\w+", "''", e)
    e = e.replace("&&", " and ").replace("||", " or ")
    e = re.sub(r"!(?!=)", " not ", e)
    if ("success()" not in expr and "always()" not in expr and "failure()" not in expr
            and "cancelled()" not in expr and always_ok != "success"):
        return False  # an `if:` without a status function implies success()
    return bool(eval(e, {"__builtins__": {}}, {"True": True, "False": False}))


def walk(ctx):
    """Which steps of `lint` run, in order, given the context. A failed step ends success()."""
    if not evaluate(lint["if"], ctx, "success"):
        return None, []
    status, ran = "success", []
    for s in lint["steps"]:
        name = s.get("name") or s.get("uses", "?")
        if evaluate(s.get("if"), ctx, status):
            ran.append(name)
            if name == "A draft runs no lint":
                r = subprocess.run(["sh", "-c", s["run"]], capture_output=True, text=True)
                check("the draft step says draft: lint not run", "draft: lint not run" in r.stdout)
                if r.returncode != 0:
                    status = "failure"
    return status, ran


real = "Install the pinned toolchain"
draft = {"event": "pull_request", "gate": gate_outputs("pull_request", True)}
check("the gate marks a draft draft=true", draft["gate"].get("draft") == "true")
status, ran = walk(draft)
check("a draft's lint job runs (not skipped)", status is not None)
check("a draft's lint job fails", status == "failure")
check("a draft's lint runs its draft step", "A draft runs no lint" in ran)
check("a draft runs no real lint step (ran: %s)" % ran, ran[:3] == ["Start the job wall-time clock", next(s.get("uses") for s in lint["steps"] if "checkout" in s.get("uses", "")), "A draft runs no lint"]
      and real not in ran and len(ran) == 4 and ran[-1] == "Check the job's wall time against its budget")

ready = {"event": "pull_request", "gate": gate_outputs("pull_request", False)}
check("the gate does not mark a ready pull request a draft", ready["gate"].get("draft", "") == "")
status, ran = walk(ready)
check("a ready pull request's lint skips the draft step", "A draft runs no lint" not in ran)
check("a ready pull request's lint runs the real lint", real in ran and status == "success")

group = {"event": "merge_group", "gate": {}}
status, ran = walk(group)
check("a merge group's lint runs the real lint", real in ran and "A draft runs no lint" not in ran)

tested = {"event": "push", "gate": {"run": "false", "draft": ""}}
check("a push to main a merge group tested still skips lint", walk(tested)[0] is None)

for f in failed:
    print("%s: %s" % (me, f), file=sys.stderr)
if not failed:
    print("%s: ok" % me)
sys.exit(1 if failed else 0)
PY
