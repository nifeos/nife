#!/usr/bin/env python3
"""A workflow step that swallows its own failure says where the failure goes instead.

    helpers/workflow_swallows.py              # script/lint: fail on an unlabelled swallow
    helpers/workflow_swallows.py --selftest   # fixtures, no tree; script/lint runs it first

Milestone 641 (a mechanism that reports clean says over how many units, and zero is loud),
provisional number. The file name and the label vocabulary below are provisional too; naming is an
architect's. A python module under `helpers/`, outside `script/names`' scope, so its provenance is
this paragraph. The reasoning and the per-site labels are in notes/denominators.md.

# WHY

From outside, a check that examined nothing looks exactly like a check that found nothing. The
weekly falsification sweep replayed zero patches on three Mondays and reported success each time
(notes/corrections/2026-09-23-the-sweep-that-swept-nothing.md), because `continue-on-error: true`
discarded the exit status of a script that had refused to run. A step's exit status is one channel
carrying two claims: the VERDICT ("it ran and found something for a human") and the OUTCOME ("it did
not run"). Suppressing a verdict can be right. Suppressing an outcome never is, unless somebody
decided so and wrote down why.

A script under `script/` can make its own zero loud: milestone 401 (a gate that selects the set it
judges can pass by checking nothing) did that half, and a script that exits non-zero on an empty
population needs no workflow cooperation. A workflow can hide that zero
again in exactly two ways, and this checks both:

1. **`continue-on-error` with anything but `false`.** Each one carries a trailing label on the same
   line, `# outcome: <kind>`, and the kind is checked against the job:
   - `re-raised`: the step has an `id:`, and a LATER step in the same job has an `if:` testing
     `steps.<id>.outcome == 'failure'` or `!= 'success'`. That later step is where the job goes red.
   - `reported`: the step has an `id:`, and a later step in the same job reads
     `${{ steps.<id>.outcome }}` as a value, so the failure travels into whatever that step writes
     (a pull request body, a summary).
   - `exception, <reason>`: the outcome is swallowed on purpose. The reason is required and is all a
     machine can ask for; whether it is a good reason is a reviewer's.
   `steps.<id>.conclusion` never counts. With `continue-on-error` the conclusion of a failed step is
   `success`, so a test on it can never fire; it is the one spelling that looks right and is not.
2. **A pipeline into `tee` without pipefail.** GitHub's default `run` shell is `bash -e {0}`, which
   gives a pipeline `tee`'s status, and `tee` succeeds. A step that pipes a command into `tee` must
   say `shell: bash` (GitHub then adds `-o pipefail`) or `set -o pipefail` itself. A pipeline headed
   by `echo` or `printf` is exempt: it has no outcome to lose.

# WHAT IT CANNOT SEE (recorded where a reader meets it)

- `|| true`, `2>/dev/null`, `if: always()` and a selector inside a `run:` block that can select
  nothing. notes/denominators.md labels the first three by family; most `|| true` sites feed an
  empty string into a test that fails toward doing more, which is the safe direction, and no parse
  can tell that from the other kind.
- A job-level `defaults: run: shell:` is not read; a step in such a job that pipes into `tee` names
  its shell itself or is flagged.
- A step in a composite action under `.github/actions/` is not walked. None exists today.
- `re-raised` checks that the later step tests the outcome, not that its body exits non-zero.

Plain text parse, no YAML library: the runner image and the developer machine do not agree on one.
"""
import os
import re
import sys
import tempfile

CONTINUE = re.compile(r"^(\s*)continue-on-error:\s*([^#\s][^#]*?)?\s*(#.*)?$")
LABEL = re.compile(r"#\s*outcome:\s*(re-raised|reported|exception)\b\s*[,;:]?\s*(.*)$")
STEP_START = re.compile(r"^(\s*)-\s+\S")
KEY = re.compile(r"^(\s*)([A-Za-z_][\w-]*):")
TEE = re.compile(r"\|\s*(sudo\s+)?tee\b")
HARMLESS_HEAD = re.compile(r"^\s*(echo|printf)\b")


def indent(line):
    return len(line) - len(line.lstrip(" "))


def significant(line):
    s = line.strip()
    return s and not s.startswith("#")


def steps_of(lines):
    """[(start, end, dash_indent, job_start, job_end)] for every `- ` list item under a `steps:`."""
    out = []
    i = 0
    n = len(lines)
    while i < n:
        m = KEY.match(lines[i])
        if m and m.group(2) == "steps" and lines[i].rstrip().endswith(":"):
            key_indent = len(m.group(1))
            # The job is the nearest key above at a smaller indent.
            j = i - 1
            while j >= 0 and not (significant(lines[j]) and indent(lines[j]) < key_indent
                                  and KEY.match(lines[j])):
                j -= 1
            job_start = max(j, 0)
            job_indent = indent(lines[job_start])
            k = i + 1
            while k < n and (not significant(lines[k]) or indent(lines[k]) > job_indent):
                k += 1
            job_end = k
            # Items: dash lines inside (i, job_end) at the first dash indent found.
            dash = None
            starts = []
            for t in range(i + 1, job_end):
                sm = STEP_START.match(lines[t])
                if not sm or not significant(lines[t]):
                    continue
                if dash is None:
                    dash = len(sm.group(1))
                if len(sm.group(1)) == dash:
                    starts.append(t)
            for a, s in enumerate(starts):
                e = starts[a + 1] if a + 1 < len(starts) else job_end
                # Trailing comments belong to the next step, so trim them off this one.
                while e > s + 1 and not significant(lines[e - 1]):
                    e -= 1
                out.append((s, e, dash, job_start, job_end))
            i = job_end
            continue
        i += 1
    return out


def step_key(lines, s, e, dash, key):
    """The value of `key` at this step's own key level, or None."""
    for t in range(s, e):
        line = lines[t]
        text = line[dash + 2:] if t == s else line
        if t != s and indent(line) != dash + 2:
            continue
        m = re.match(r"\s*%s:\s*(.*?)\s*(#.*)?$" % re.escape(key), text)
        if m:
            return m.group(1)
    return None


def conditions(lines, start, end):
    """Every `if:` value in lines[start:end], with a folded (`>-`) value's continuation joined."""
    out = []
    for t in range(start, end):
        m = re.match(r"^(\s*)(-\s+)?if:\s*(.*)$", lines[t])
        if not m:
            continue
        own = len(m.group(1)) + len(m.group(2) or "")
        parts = [m.group(3)]
        u = t + 1
        while u < end and (not lines[u].strip() or indent(lines[u]) > own):
            parts.append(lines[u].strip())
            u += 1
        out.append(" ".join(parts))
    return out


def check_text(path, text):
    """(problems, swallows, tees): the findings and the two denominators for one workflow."""
    lines = text.split("\n")
    problems = []
    swallows = tees = 0
    steps = steps_of(lines)
    in_a_step = set()
    for s, e, dash, job_start, job_end in steps:
        for t in range(s, e):
            in_a_step.add(t)
        sid = step_key(lines, s, e, dash, "id")
        shell = step_key(lines, s, e, dash, "shell") or ""
        body = "\n".join(lines[s:e])
        for t in range(s, e):
            m = CONTINUE.match(lines[t])
            if not m or indent(lines[t]) != dash + 2 and t != s:
                continue
            value = (m.group(2) or "").strip()
            if value == "false":
                continue
            swallows += 1
            where = "%s:%d" % (path, t + 1)
            label = LABEL.search(m.group(3) or "")
            if not label:
                problems.append("%s: `continue-on-error: %s` with no `# outcome: <kind>` label on "
                                "the same line. Say where the failure goes: re-raised, reported, "
                                "or exception with a reason." % (where, value))
                continue
            kind, reason = label.group(1), label.group(2).strip()
            later = "\n".join(lines[e:job_end])
            if kind == "exception":
                if len(reason.split()) < 3:
                    problems.append("%s: `# outcome: exception` needs its reason on the line, in "
                                    "at least three words" % where)
                continue
            if not sid:
                problems.append("%s: `# outcome: %s` on a step with no `id:`, so nothing later "
                                "can read its outcome" % (where, kind))
                continue
            sidr = re.escape(sid)
            if re.search(r"steps\.%s\.conclusion" % sidr, later):
                problems.append("%s: a later step reads `steps.%s.conclusion`, which is `success` "
                                "for a failed step under continue-on-error; read `.outcome`"
                                % (where, sid))
                continue
            if kind == "re-raised":
                ok = any(re.search(r"steps\.%s\.outcome\s*(==\s*'failure'|!=\s*'success')"
                                   % sidr, cond) for cond in conditions(lines, e, job_end))
                if not ok:
                    problems.append("%s: labelled re-raised, but no later step in this job has an "
                                    "`if:` testing `steps.%s.outcome == 'failure'`" % (where, sid))
            elif kind == "reported":
                if not re.search(r"\$\{\{\s*steps\.%s\.outcome\s*\}\}" % sidr, later):
                    problems.append("%s: labelled reported, but no later step in this job reads "
                                    "`${{ steps.%s.outcome }}`" % (where, sid))
        if TEE.search(body):
            pipefail = shell.strip() == "bash" or re.search(r"set\s+-[a-z]*o\s+pipefail|set\s+-o\s+pipefail", body)
            for t in range(s, e):
                line = lines[t]
                if line.lstrip().startswith("#") or not TEE.search(line):
                    continue
                head = TEE.split(line)[0]
                head = re.sub(r"^\s*(-\s*)?run:\s*\|?\s*", "", head)
                if HARMLESS_HEAD.match(head):
                    continue
                tees += 1
                if not pipefail:
                    problems.append("%s:%d: a pipeline into `tee` without pipefail exits with "
                                    "`tee`'s status, which is success. Give the step `shell: bash` "
                                    "or `set -o pipefail`." % (path, t + 1))
    # A `continue-on-error` outside any step this parse found is a job-level one, or a parse that
    # has gone wrong. Either way it is not allowed to pass unread.
    for t, line in enumerate(lines):
        m = CONTINUE.match(line)
        if m and t not in in_a_step and (m.group(2) or "").strip() != "false":
            swallows += 1
            label = LABEL.search(m.group(3) or "")
            if not label or label.group(1) != "exception" or len(label.group(2).split()) < 3:
                problems.append("%s:%d: a job-level `continue-on-error` can only be "
                                "`# outcome: exception, <reason>`, since no later step can read "
                                "it" % (path, t + 1))
    return problems, swallows, tees


def check_tree(root):
    d = os.path.join(root, ".github", "workflows")
    # `os.listdir` raises when the directory moves, which is the loud failure notes/empty-selectors.md
    # relies on; the count below is the other half, for a directory that exists and is empty.
    names = sorted(f for f in os.listdir(d) if f.endswith((".yml", ".yaml")))
    if not names:
        print("workflow swallows: %s holds no workflow, so this examined nothing" % d, file=sys.stderr)
        return 1
    problems = []
    swallows = tees = 0
    for f in names:
        with open(os.path.join(d, f)) as fh:
            p, s, t = check_text(".github/workflows/" + f, fh.read())
        problems += p
        swallows += s
        tees += t
    for p in problems:
        print("  " + p, file=sys.stderr)
    if problems:
        print("workflow swallows: %d problem(s); see helpers/workflow_swallows.py and "
              "notes/denominators.md" % len(problems), file=sys.stderr)
        return 1
    print("workflow swallows: %d workflows, %d continue-on-error step(s) each labelled with where "
          "its failure goes, %d pipeline(s) into tee with pipefail" % (len(names), swallows, tees))
    return 0


# ---- fixtures ----------------------------------------------------------------------------------

HEAD = """name: x
on: push
jobs:
  j:
    runs-on: ubuntu-26.04
    steps:
      - uses: actions/checkout@v7
"""

CASES = [
    # (name, body appended to HEAD, expected problem count)
    ("an unlabelled swallow fails", """
      - name: sweep
        continue-on-error: true
        run: script/x
""", 1),
    ("false is not a swallow", """
      - name: sweep
        continue-on-error: false
        run: script/x
""", 0),
    ("an exception with a reason passes", """
      - name: sweep
        continue-on-error: true  # outcome: exception, a missing nightly is upstream's
        run: script/x
""", 0),
    ("an exception with no reason fails", """
      - name: sweep
        continue-on-error: true  # outcome: exception
        run: script/x
""", 1),
    ("re-raised with a later failure test passes", """
      - name: sweep
        id: sweep
        continue-on-error: true  # outcome: re-raised
        run: script/x
      - name: upload
        run: true
      - name: fail
        if: steps.sweep.outcome == 'failure'
        run: exit 1
""", 0),
    ("re-raised through a folded if passes", """
      - name: sweep
        id: sweep
        continue-on-error: true  # outcome: re-raised
        run: script/x
      - name: fail
        if: >-
          steps.other.outcome == 'failure' ||
          steps.sweep.outcome == 'failure'
        run: exit 1
""", 0),
    ("re-raised with no later test fails", """
      - name: sweep
        id: sweep
        continue-on-error: true  # outcome: re-raised
        run: script/x
      - name: gated on success only
        if: steps.sweep.outcome == 'success'
        run: true
""", 1),
    ("re-raised read through conclusion fails, because it can never fire", """
      - name: sweep
        id: sweep
        continue-on-error: true  # outcome: re-raised
        run: script/x
      - name: fail
        if: steps.sweep.conclusion == 'failure'
        run: exit 1
""", 1),
    ("re-raised with no id fails", """
      - name: sweep
        continue-on-error: true  # outcome: re-raised
        run: script/x
""", 1),
    ("re-raised in ANOTHER job does not count", """
      - name: sweep
        id: sweep
        continue-on-error: true  # outcome: re-raised
        run: script/x
  other:
    runs-on: ubuntu-26.04
    steps:
      - name: fail
        if: steps.sweep.outcome == 'failure'
        run: exit 1
""", 1),
    ("reported into a later step's value passes", """
      - name: build
        id: build
        continue-on-error: true  # outcome: reported
        run: cargo build
      - name: pr
        env:
          BUILD: ${{ steps.build.outcome }}
        run: helpers/pr.sh
""", 0),
    ("tee without pipefail fails", """
      - name: sweep
        run: |
          script/x | tee out.txt
""", 1),
    ("tee under shell: bash passes", """
      - name: sweep
        shell: bash
        run: |
          script/x | tee out.txt
""", 0),
    ("tee with set -o pipefail passes", """
      - name: sweep
        run: |
          set -o pipefail
          script/x 2>&1 | tee out.txt
""", 0),
    ("echo into sudo tee is exempt", """
      - name: udev
        run: |
          echo 'KERNEL=="kvm"' | sudo tee /etc/udev/rules.d/99.rules
""", 0),
    ("a job-level swallow must be an exception", """
  other:
    runs-on: ubuntu-26.04
    continue-on-error: true  # outcome: re-raised
    steps:
      - run: true
""", 1),
]


def selftest():
    failed = 0
    for name, body, want in CASES:
        problems, _, _ = check_text("fixture.yml", HEAD + body)
        if len(problems) != want:
            failed += 1
            print("workflow swallows selftest: %s: want %d problem(s), got %d: %s"
                  % (name, want, len(problems), problems), file=sys.stderr)
    # The empty directory is a case of its own: this check must not pass by checking nothing.
    with tempfile.TemporaryDirectory() as tmp:
        os.makedirs(os.path.join(tmp, ".github", "workflows"))
        saved = sys.stderr
        sys.stderr = open(os.devnull, "w")
        try:
            rc = check_tree(tmp)
        finally:
            sys.stderr.close()
            sys.stderr = saved
        if rc == 0:
            failed += 1
            print("workflow swallows selftest: an empty workflow directory passed", file=sys.stderr)
    if failed:
        return 1
    print("workflow swallows selftest: %d fixtures, each failing case fails" % (len(CASES) + 1))
    return 0


def main(argv):
    if argv and argv[0] == "--selftest":
        return selftest()
    if argv:
        print("usage: helpers/workflow_swallows.py [--selftest]", file=sys.stderr)
        return 2
    root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
    return check_tree(root)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
