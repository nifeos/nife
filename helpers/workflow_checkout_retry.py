#!/usr/bin/env python3
"""Every `actions/checkout` step in a workflow is a bounded first attempt plus a retry.

    helpers/workflow_checkout_retry.py              # script/lint: fail on a bare checkout
    helpers/workflow_checkout_retry.py --selftest   # fixtures, no tree; script/lint runs it first

Name provisional; naming is an architect's. A python module under `helpers/`, outside
`script/names`' scope.

# WHY

On GitHub-hosted runners the `git fetch` inside `actions/checkout` sometimes goes silent for about
13 minutes and then completes. Six of 1,473 checkout steps did, against a median of 8 seconds, and
each one spent most of a 20-minute job budget and could eject a pull request from the merge queue.
A step's `timeout-minutes` is the only bound that reaches into the action, and a composite action
cannot carry one (`runs.steps[*]` has no `timeout-minutes`), so each site spells the pair out.

# THE SHAPE

    - uses: actions/checkout@<sha> # <tag>
      id: checkout_first_1
      timeout-minutes: 2
      continue-on-error: true  # outcome: re-raised
      with: ...

    - uses: actions/checkout@<sha> # <tag>
      timeout-minutes: 3
      if: steps.checkout_first_1.outcome != 'success'
      with: ...                 # identical to the first step's

The second attempt has no `continue-on-error`, so a job whose two attempts both fail goes red. The
action's `clean` input defaults to true and its `prepareExistingDirectory` cleans a workspace the
first attempt left behind, so the second attempt starts from a usable directory. A job that must
keep the bare action says so on the comment line above the step:

    # checkout-retry: exempt, <reason>

What this cannot see: whether the `with:` values are the right ones, or a checkout written as
anything but a block-style `- uses:` step. `helpers/workflow_parses.py` refuses a file that does not
parse at all.
"""
import glob
import re
import sys

USES = re.compile(r"^(\s*)- uses: actions/checkout@")
EXEMPT = re.compile(r"^\s*#\s*checkout-retry:\s*exempt,\s*\S")


def steps(lines):
    """Yield (start_index, indent, body_lines) for each checkout step, body excluding the first line."""
    i = 0
    while i < len(lines):
        m = USES.match(lines[i])
        if not m:
            i += 1
            continue
        ind = len(m.group(1))
        j = i + 1
        while j < len(lines) and (lines[j].strip() == "" or len(lines[j]) - len(lines[j].lstrip()) > ind):
            j += 1
        k = j
        while lines[k - 1].strip() == "":
            k -= 1
        yield i, ind, lines[i + 1:k], j
        i = j


def key(body, name):
    for b in body:
        m = re.match(r"^\s*" + re.escape(name) + r":\s*(.*)$", b)
        if m:
            return m.group(1)
    return None


def withblock(body):
    out = []
    on = False
    for b in body:
        if re.match(r"^\s*with:\s*$", b):
            on = True
            continue
        if on:
            if b.strip().startswith("#") or b.strip() == "":
                continue
            out.append(b.strip())
    return out


def check_text(text, where):
    lines = text.split("\n")
    problems = []
    found = list(steps(lines))
    retry_of = set()
    # a second attempt is validated with its first; collect ids a step's `if` tests
    for idx, (i, ind, body, end) in enumerate(found):
        cond = key(body, "if") or ""
        m = re.search(r"steps\.(\w+)\.outcome\s*!=\s*'success'", cond)
        if m:
            retry_of.add(i)
    for idx, (i, ind, body, end) in enumerate(found):
        line = i + 1
        if i in retry_of:
            continue
        k = i - 1
        exempt = False
        while k >= 0 and lines[k].strip().startswith("#"):
            if EXEMPT.match(lines[k]):
                exempt = True
            k -= 1
        if exempt:
            continue
        sid = key(body, "id")
        if not sid:
            problems.append("%s:%d: bare actions/checkout; use the bounded first attempt plus retry "
                            "(helpers/workflow_checkout_retry.py header)" % (where, line))
            continue
        if key(body, "timeout-minutes") != "2":
            problems.append("%s:%d: the first checkout attempt needs `timeout-minutes: 2`" % (where, line))
        coe = key(body, "continue-on-error") or ""
        if not coe.startswith("true"):
            problems.append("%s:%d: the first checkout attempt needs `continue-on-error: true`" % (where, line))
        nxt = found[idx + 1] if idx + 1 < len(found) else None
        if nxt is None or nxt[0] not in retry_of or lines[i + 1 + len(body):nxt[0]].count("") != nxt[0] - (i + 1 + len(body)):
            problems.append("%s:%d: the first checkout attempt is not followed directly by its retry "
                            "step" % (where, line))
            continue
        _, _, body2, _ = nxt
        cond2 = key(body2, "if") or ""
        if ("steps.%s.outcome != 'success'" % sid) not in cond2:
            problems.append("%s:%d: the retry must test `steps.%s.outcome != 'success'`" % (where, nxt[0] + 1, sid))
        if key(body2, "timeout-minutes") != "3":
            problems.append("%s:%d: the retry needs `timeout-minutes: 3`" % (where, nxt[0] + 1))
        if key(body2, "continue-on-error") is not None:
            problems.append("%s:%d: the retry must not set `continue-on-error`, so a double failure is red"
                            % (where, nxt[0] + 1))
        if withblock(body) != withblock(body2):
            problems.append("%s:%d: the retry's `with:` differs from the first attempt's" % (where, nxt[0] + 1))
    return problems


GOOD = """jobs:
  a:
    steps:
      - uses: actions/checkout@abc # v7
        id: checkout_first_1
        timeout-minutes: 2
        continue-on-error: true  # outcome: re-raised
        with:
          fetch-depth: 0

      - uses: actions/checkout@abc # v7
        timeout-minutes: 3
        if: steps.checkout_first_1.outcome != 'success'
        with:
          fetch-depth: 0

      - run: true
"""


def selftest():
    assert check_text(GOOD, "t") == [], check_text(GOOD, "t")
    cases = {
        "bare": "jobs:\n  a:\n    steps:\n      - uses: actions/checkout@abc # v7\n        with:\n          fetch-depth: 0\n",
        "first timeout": GOOD.replace("timeout-minutes: 2", "timeout-minutes: 9"),
        "first swallow": GOOD.replace("continue-on-error: true  # outcome: re-raised\n", ""),
        "retry timeout": GOOD.replace("timeout-minutes: 3", "timeout-minutes: 9"),
        "retry swallows": GOOD.replace("        timeout-minutes: 3\n", "        timeout-minutes: 3\n        continue-on-error: true\n"),
        "retry if": GOOD.replace("!= 'success'", "== 'failure'"),
        "with differs": GOOD.replace("fetch-depth: 0\n\n      - run", "fetch-depth: 1\n\n      - run"),
        "no retry": GOOD.split("\n      - uses: actions/checkout@abc # v7\n        timeout-minutes: 3")[0] + "\n      - run: true\n",
    }
    for name, text in cases.items():
        assert check_text(text, "t"), "fixture %r was not refused" % name
    exempt = "      # checkout-retry: exempt, measures the runner\n"
    assert check_text(cases["bare"].replace("      - uses", exempt + "      - uses"), "t") == []
    assert check_text(cases["bare"].replace("      - uses", "      # checkout-retry: exempt,\n      - uses"), "t")
    print("workflow_checkout_retry: selftest ok (%d refusals)" % len(cases))


def main():
    if "--selftest" in sys.argv:
        selftest()
        return 0
    files = sorted(glob.glob(".github/workflows/*.yml") + glob.glob(".github/workflows/*.yaml"))
    problems = []
    count = 0
    for f in files:
        text = open(f).read()
        count += len(list(steps(text.split("\n"))))
        problems += check_text(text, f)
    if count == 0:
        print("workflow_checkout_retry: no actions/checkout step found in %d files; the pattern may have rotted" % len(files), file=sys.stderr)
        return 1
    for p in problems:
        print("lint: " + p, file=sys.stderr)
    if not problems:
        print("workflow_checkout_retry: %d checkout steps, all bounded with a retry or exempt" % count)
    return 1 if problems else 0


sys.exit(main())
