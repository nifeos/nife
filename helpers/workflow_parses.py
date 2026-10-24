#!/usr/bin/env python3
"""Every file under .github/workflows/ parses.

    helpers/workflow_parses.py              # script/lint: fail naming the file and the parser's error
    helpers/workflow_parses.py --selftest   # fixtures, no tree; script/lint runs it first

Milestone 861 (every workflow file parses). The file name is provisional; naming is an
architect's.

# WHY

A comment line that loses its `#` prefix to a line-wrap slip is invisible to every other gate:
the pin and swallow checks match their regexes on the lines they care about and wave the rest
through, and the pre-push hook waves the file through with them. GitHub does not: a workflow file
that does not parse never starts a run, and for ci.yml and verify.yml that is every pull request
waiting on a required check nothing can post, the failure shape notes/repo-hardening.md records.
This check is the parse those regex gates never did.

# THE PARSER

`ruby -ryaml`, Ruby's stdlib YAML (Psych), one process over the whole directory. The tree's other
workflow checks parse by hand because, as helpers/workflow_swallows.py's header records, no YAML
library was present on both the runner image and the developer machine. Ruby's stdlib YAML is
present on both: macOS ships /usr/bin/ruby, and the ubuntu-24.04 runner images (amd64, and arm64,
which is what ci.yml's jobs run on) list Ruby 3.2.3, both read from actions/runner-images on
2026-10-15. A real parser is the honest answer, so there is no structural fallback in this file
and no weaker parse anywhere beside it.

`YAML.parse_file` builds Psych's node tree and evaluates nothing, so no tag or alias in a
workflow executes anything, and Psych::SyntaxError names the line and column this check prints.

# BUGS

- This proves each file is YAML, not that it is a valid workflow. GitHub's schema is not
  validated: a misspelled key that stays well-formed (`runs-o:`) parses, and remains the pin,
  swallow and budget checks' business.
- Psych is YAML 1.1 and GitHub parses with its own, so the gate and GitHub can disagree in either
  direction in principle. The incident shape (a comment line that lost its `#`) is rejected by
  both; a file this passes and GitHub rejects is the residue.
- A machine without `ruby` on PATH fails loudly naming that instead of degrading to a weaker
  parse, so the gate is down there until ruby is installed (`script/bootstrap` installs it). That
  is deliberate: a quiet degradation is the failure this check exists to end.
- A lost `#` inside a `run: |` block edits the script's text and stays valid YAML. Nothing that
  reads structure can see it.
- Only `.github/workflows/` is walked. `.github/actions/` (composite actions) holds nothing
  today; if one appears, its files do not parse through this check.
"""
import os
import subprocess
import sys
import tempfile

WORKFLOWS = os.path.join(".github", "workflows")

# One ruby process for the directory: it rescues each file's error, so every broken file is named
# rather than only the first, and exits non-zero when any failed.
PARSE = """\
$failed = false
ARGV.each do |path|
  begin
    YAML.parse_file(path)
  rescue Psych::SyntaxError => e
    warn "#{path}: #{e.message}"
    $failed = true
  end
end
exit($failed ? 1 : 0)
"""


def parse(cwd, rels):
    """(failures, note): ruby's verdict over the directory's files. It runs with `cwd` so the
    names it prints are the tree's own. `note` is set only when the gate could not run at all."""
    try:
        run = subprocess.run(["ruby", "-ryaml", "-e", PARSE, "--"] + rels,
                             capture_output=True, text=True, cwd=cwd)
    except FileNotFoundError:
        return [], ("`ruby` is not on PATH. The parser is `ruby -ryaml` "
                    "(helpers/workflow_parses.py); run script/bootstrap (it installs it), do not skip the gate")
    if run.returncode not in (0, 1):
        return [], (run.stderr.strip() or "ruby exited %d with no error text" % run.returncode)
    return [line for line in run.stderr.splitlines() if line.strip()], None


def check_tree(root):
    d = WORKFLOWS
    # `os.listdir` raises when the directory moves, which is loud; the count below is the other
    # half, for a directory that exists and is empty (design/roadmap/0401-a-gate-that-selects-
    # the-set-it-judges.md).
    names = sorted(f for f in os.listdir(os.path.join(root, d)) if f.endswith((".yml", ".yaml")))
    if not names:
        print("workflow parses: %s holds no workflow, so this examined nothing" % d, file=sys.stderr)
        return 1
    failures, note = parse(root, [os.path.join(d, f) for f in names])
    if note:
        print("workflow parses: " + note, file=sys.stderr)
        return 1
    if failures:
        for line in failures:
            print("  " + line, file=sys.stderr)
        print("workflow parses: %d of %d file(s) do not parse. GitHub rejects such a file and "
              "never starts its run, which leaves a required check nothing can post; see "
              "helpers/workflow_parses.py." % (len(failures), len(names)), file=sys.stderr)
        return 1
    print("workflow parses: %d file(s), every one parses under `ruby -ryaml`" % len(names))
    return 0


# ---- fixtures -----------------------------------------------------------------------------------

HEAD = """\
name: x
on: push
jobs:
  gate:
    runs-on: ubuntu-24.04
    steps:
"""

CASES = [
    # (name, steps appended to HEAD, whether the file must parse)
    ("the incident: a comment line that lost its #", """
      - name: budget
        if: always()
        runs even when a step failed, so the budget check always posts
        run: python3 helpers/job-budget.py check
""", False),
    ("a tab in the indentation", """
      - name: budget
\trun: python3 helpers/job-budget.py check
""", False),
    ("an ordinary workflow", """
      - uses: actions/checkout@v7
      - run: true
""", True),
]


def run_quiet(root):
    """check_tree's exit code with its output discarded, over a throwaway tree."""
    saved = sys.stderr
    sys.stderr = open(os.devnull, "w")
    try:
        return check_tree(root)
    finally:
        sys.stderr.close()
        sys.stderr = saved


def selftest():
    failed = 0
    for name, body, want in CASES:
        with tempfile.TemporaryDirectory() as tmp:
            d = os.path.join(tmp, WORKFLOWS)
            os.makedirs(d)
            with open(os.path.join(d, "fixture.yml"), "w") as fh:
                fh.write(HEAD + body)
            rc = run_quiet(tmp)
        if rc != (0 if want else 1):
            failed += 1
            print("workflow parses selftest: %s: want %s, got exit %d"
                  % (name, "pass" if want else "fail", rc), file=sys.stderr)
    # The empty directory is a case of its own: this check must not pass by checking nothing
    # (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md).
    with tempfile.TemporaryDirectory() as tmp:
        os.makedirs(os.path.join(tmp, WORKFLOWS))
        if run_quiet(tmp) == 0:
            failed += 1
            print("workflow parses selftest: an empty workflow directory passed", file=sys.stderr)
    if failed:
        return 1
    print("workflow parses selftest: %d fixtures, each failing case fails" % (len(CASES) + 1))
    return 0


def main(argv):
    if argv and argv[0] == "--selftest":
        return selftest()
    if argv:
        print("usage: helpers/workflow_parses.py [--selftest]", file=sys.stderr)
        return 2
    root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
    return check_tree(root)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
