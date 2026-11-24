"""The freeze before the record leaves: no pull request changes a path that moves with the record.

Milestone 1018 (the design record moves to `nifeos/record` and is published as a website), step 6,
pull request 6 of its plan. `nifeos/record` is pushed from an extraction of this tree at the frozen
tip, and the removal pull request later checks that the record at basalt's pin equals this tree's
moving paths. A record edit that landed here after the extraction would make that check fail, or,
without the check, vanish at the removal. So from this freeze until the removal, no change to a
moving path lands in nife.

A path moves when `helpers/record_paths.py` selects it: its home in `packages/homes.toml` is the
record, or it is one of the record's own tools. The judgment reads the homes twice, at the base and
at the head, and refuses a changed path that moves on either side, so a pull request cannot re-sort
a path out of the record and edit it in one step, and cannot add a new file under a moving
directory.

    python3 helpers/record_freeze.py              # what script/lint runs; the context picks the base
    python3 helpers/record_freeze.py --base <rev> --enforce   # judge HEAD against <rev>, refusing
    python3 helpers/record_freeze.py --moves <path>...        # exit 0 when every path moves
    python3 helpers/record_freeze.py --notice     # print the notice for lanes
    python3 helpers/record_freeze.py --selftest   # fixtures and one scratch repository

# Where it refuses, and where it only warns

In a `pull_request` run the base is the merge base of HEAD with the base branch's current tip
(`helpers/scope-merge-base.sh`), and in a `merge_group` run it is the group's `base_sha`; both
refuse, and a base that does not resolve fails the run rather than passing it. Anywhere else (a
lane's own `script/lint`, the pre-push hook, a push to `main`) it judges HEAD against its merge base
with `origin/main` and only warns, so a lane keeps pushing a draft that holds a record edit.

# The one exception: the removal pull request

The removal pull request (pull request 10 of the plan) passes, and nothing else does. It passes
when both of these hold, and neither happens by accident:

- every moving path tracked at the base is gone at the head, and no moving path is tracked there;
- the diff adds `packages/moved.toml`, the file that names the anchor the record was cut from.

A partial removal, such as one directory deleted by mistake, keeps the first condition false and is
refused path by path. The removal pull request also lifts the freeze, by deleting this helper, its
`script/lint` section and the step in `metrics.yml` that reads it, and a run lints with the tree's
own `script/lint`. The exception is what lets that pull request land in either order: removal and
lift together, or the removal first with the freeze still in place.

Name: provisional, minted by the lane for milestone 1018's step 6 on 2026-11-24 (UTC). A python
module under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record. `--moves` and `--notice` are provisional with it.

BUGS

- While the freeze holds, `metrics.yml` leaves `notes/project-metrics.md` out of its pull request:
  its generated lines (the table of contents, the drift line, the week-note bullets) stand still,
  while the data under `notes/project-metrics/`, which stays in nife, keeps its daily row. The
  page catches up on the first run that writes it into `nifeos/record`, and the removal pull
  request is where that workflow switches.
- A ready `milestone/N-*` pull request must change its own roadmap block (`ready-status.yml`), and
  the block moves, so no milestone branch lands in nife while the freeze holds. Its lane keeps the
  draft and lands the block in the record.
"""

import json
import os
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import packages                                                             # noqa: E402
import record_paths                                                         # noqa: E402

# The file only the removal pull request adds (the plan's pull request 10).
MARKER = "packages/moved.toml"

NOTICE = """\
nife is frozen while the design record moves to nifeos/record (milestone 1018, step 6). No pull
request here may change a path that moves with the record: `python3 helpers/record_paths.py` lists
them, and `python3 helpers/record_freeze.py --moves <path>` answers for one. A lane holding such an
edit keeps its draft. Once nifeos/record exists, it lands the edit there, by pull request against
the record's main, and takes the edit off its nife branch, which can then land the rest. A
milestone branch lands its roadmap block in the record the same way."""


def git(args, root=".", stdin=None):
    return subprocess.run(["git"] + args, cwd=root, input=stdin, capture_output=True,
                          check=True).stdout


def tree_at(rev, root="."):
    """(tracked paths, {path: text} of what packages.evaluate reads) at a commit, from git alone."""
    tracked = [p for p in git(["ls-tree", "-r", "-z", "--name-only", rev], root).decode()
               .split("\0") if p]
    # The homes never depend on a Rust source's text, only on manifests and package files.
    want = [p for p in tracked if packages.wanted(p) and not p.endswith(".rs")]
    texts = {}
    if want:
        out = git(["cat-file", "--batch"], root,
                  "".join(f"{rev}:{p}\n" for p in want).encode())
        pos = 0
        for p in want:
            header_end = out.index(b"\n", pos)
            size = int(out[pos:header_end].split()[2])
            texts[p] = out[header_end + 1:header_end + 1 + size].decode("utf-8", "replace")
            pos = header_end + 1 + size + 1
    return tracked, texts


def working_tree(root="."):
    tracked, texts = packages.working_tree(root)
    return tracked, {p: t for p, t in texts.items() if not p.endswith(".rs")}


def moving(tracked, texts):
    """The tracked paths that move with the record, as helpers/record_paths.py selects them."""
    home = packages.evaluate(tracked, texts)["file_home"]
    return {p for p in tracked if record_paths.moves(home.get(p)) or p in record_paths.TOOLS}


def changed(base, root="."):
    """Every path the diff from `base` to HEAD adds, changes or deletes; a rename is both ends."""
    out = git(["diff", "--no-renames", "--name-only", "-z", base, "HEAD"], root).decode()
    return sorted({p for p in out.split("\0") if p})


def verdict(base_tracked, base_moving, head_tracked, head_moving, paths):
    """(refused paths, whether the removal exception let them through)."""
    touched = sorted(p for p in paths if p in base_moving or p in head_moving)
    if not touched:
        return [], False
    removal = (bool(base_moving) and not (base_moving & set(head_tracked)) and not head_moving
               and MARKER not in base_tracked and MARKER in head_tracked)
    if removal:
        return [], True
    return touched, False


def event_payload(env):
    path = env.get("GITHUB_EVENT_PATH", "")
    if not path:
        return {}
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def context(env, merge_base):
    """(mode, base or None, what the base is) for this run. `merge_base(ref, fallback)` resolves
    a pull request's base, as helpers/scope-merge-base.sh does."""
    event = env.get("GITHUB_EVENT_NAME", "")
    if event == "pull_request":
        fallback = event_payload(env).get("pull_request", {}).get("base", {}).get("sha", "")
        ref = env.get("GITHUB_BASE_REF", "")
        return "enforce", merge_base(ref, fallback), f"the merge base with {ref or fallback}"
    if event == "merge_group":
        base = event_payload(env).get("merge_group", {}).get("base_sha", "")
        return "enforce", base or None, "the merge group's base"
    return "warn", merge_base("main", ""), "the merge base with origin/main"


def scope_merge_base(ref, fallback):
    run = subprocess.run(["sh", os.path.join(HERE, "scope-merge-base.sh"), ref, fallback],
                         capture_output=True, text=True)
    base = run.stdout.strip()
    return base if run.returncode == 0 and base else None


def judge(mode, base, what, root="."):
    if base is None:
        if mode == "enforce":
            print(f"record_freeze: {what} does not resolve to a commit, so the freeze cannot judge "
                  "this change; the run fails rather than passing it unread", file=sys.stderr)
            return 1
        print(f"record_freeze: {what} does not resolve; nothing judged", file=sys.stderr)
        return 0
    base_tracked, base_texts = tree_at(base, root)
    head_tracked, head_texts = working_tree(root)
    base_moving = moving(base_tracked, base_texts)
    head_moving = moving(head_tracked, head_texts)
    paths = changed(base, root)
    refused, removal = verdict(set(base_tracked), base_moving, set(head_tracked), head_moving, paths)
    if removal:
        print(f"record_freeze: the removal pull request: all {len(base_moving)} moving paths are "
              f"gone and {MARKER} is added, so the freeze lets it through")
        return 0
    if not refused:
        print(f"record_freeze: {len(paths)} changed path(s) against {what}, none of the "
              f"{len(base_moving | head_moving)} that move with the record")
        return 0
    verb = "refuses" if mode == "enforce" else "would refuse, in a pull request,"
    print(f"record_freeze: the freeze {verb} {len(refused)} changed path(s) that move with the "
          f"record ({what}):", file=sys.stderr)
    for p in refused[:40]:
        print(f"  {p}", file=sys.stderr)
    if len(refused) > 40:
        print(f"  and {len(refused) - 40} more", file=sys.stderr)
    print("", file=sys.stderr)
    print(NOTICE, file=sys.stderr)
    return 1 if mode == "enforce" else 0


def selftest():
    import contextlib
    import io
    import tempfile

    def quiet_judge(*args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return judge(*args)
    # The verdict, on sets.
    base_t = {"design/a.md", "notes/b.md", "script/x", "script/roadmap"}
    base_m = {"design/a.md", "notes/b.md", "script/roadmap"}
    assert verdict(base_t, base_m, base_t, base_m, ["script/x"]) == ([], False)
    assert verdict(base_t, base_m, base_t, base_m, ["script/x", "notes/b.md"]) == (["notes/b.md"],
                                                                                  False)
    # A new file under a moving directory moves at the head, though the base never tracked it.
    head_t = base_t | {"design/new.md"}
    assert verdict(base_t, base_m, head_t, base_m | {"design/new.md"}, ["design/new.md"]) == \
        (["design/new.md"], False)
    # Re-sorted out of the record and edited at once: it moves at the base, so it is refused.
    assert verdict(base_t, base_m, base_t, base_m - {"notes/b.md"},
                   ["packages/homes.toml", "notes/b.md"]) == (["notes/b.md"], False)
    # The removal: every moving path gone, the marker added.
    gone = ["design/a.md", "notes/b.md", "script/roadmap", MARKER]
    removed_t = {"script/x", MARKER}
    assert verdict(base_t, base_m, removed_t, set(), gone) == ([], True)
    # Without the marker, or with one moving path left, or with the marker already at the base.
    assert verdict(base_t, base_m, removed_t - {MARKER}, set(), gone[:-1])[0] == gone[:-1]
    partial_t = {"script/x", MARKER, "notes/b.md"}
    assert verdict(base_t, base_m, partial_t, {"notes/b.md"}, ["design/a.md", MARKER])[0] == \
        ["design/a.md"]
    assert verdict(base_t | {MARKER}, base_m, removed_t, set(), gone)[0] == gone[:-1]
    # The marker alone, with no moving path touched, is an ordinary change.
    assert verdict(base_t, base_m, base_t | {MARKER}, base_m, [MARKER]) == ([], False)

    # The context: which base each event judges against, and which events refuse.
    with tempfile.TemporaryDirectory() as tmp:
        event = os.path.join(tmp, "event.json")
        with open(event, "w", encoding="utf-8") as f:
            json.dump({"pull_request": {"base": {"sha": "f" * 40}},
                       "merge_group": {"base_sha": "e" * 40}}, f)
        asked = []
        mb = lambda ref, fb: asked.append((ref, fb)) or "m" * 40
        pr = context({"GITHUB_EVENT_NAME": "pull_request", "GITHUB_BASE_REF": "main",
                      "GITHUB_EVENT_PATH": event}, mb)
        assert pr[:2] == ("enforce", "m" * 40) and asked[-1] == ("main", "f" * 40), (pr, asked)
        group = context({"GITHUB_EVENT_NAME": "merge_group", "GITHUB_EVENT_PATH": event}, mb)
        assert group[:2] == ("enforce", "e" * 40), group
        assert context({"GITHUB_EVENT_NAME": "merge_group"}, mb)[:2] == ("enforce", None)
        assert context({"GITHUB_EVENT_NAME": "push"}, mb)[0] == "warn"
        assert context({}, mb)[0] == "warn" and asked[-1] == ("main", "")
        # An unresolved base fails an enforcing run and passes a warning one.
        assert quiet_judge("enforce", None, "the merge group's base", tmp) == 1
        assert quiet_judge("warn", None, "the merge base with origin/main", tmp) == 0

    # One scratch repository: the homes read from git at the base and from the work tree at HEAD.
    with tempfile.TemporaryDirectory() as tmp:
        def put(path, text):
            os.makedirs(os.path.dirname(os.path.join(tmp, path)) or tmp, exist_ok=True)
            with open(os.path.join(tmp, path), "w", encoding="utf-8") as f:
                f.write(text)
        homes = ('[[home]]\nstatus = "provisional"\nrepo = "record"\npaths = ["design/"]\n\n'
                 '[[home]]\nstatus = "undecided"\nreason = "code"\npaths = ["src/", "packages/"]\n')
        put(packages.HOMES_FILE, homes)
        put("design/a.md", "a\n")
        put("src/x.txt", "x\n")
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid",
                   GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.invalid")
        sh = lambda *a: subprocess.run(["git"] + list(a), cwd=tmp, env=env, check=True,
                                       capture_output=True)
        sh("init", "-q")
        sh("add", "-A")
        sh("commit", "-q", "-m", "base")
        base = git(["rev-parse", "HEAD"], tmp).decode().strip()
        tracked, texts = tree_at(base, tmp)
        assert moving(tracked, texts) == {"design/a.md"}, moving(tracked, texts)
        put("src/x.txt", "y\n")
        sh("commit", "-q", "-am", "code")
        assert changed(base, tmp) == ["src/x.txt"]
        assert quiet_judge("enforce", base, "base", tmp) == 0
        put("design/a.md", "b\n")
        sh("commit", "-q", "-am", "record edit")
        assert quiet_judge("enforce", base, "base", tmp) == 1
        assert quiet_judge("warn", base, "base", tmp) == 0
        sh("rm", "-q", "design/a.md")
        put(MARKER, "anchor = \"0\"\n")
        sh("add", "-A")
        sh("commit", "-q", "-m", "removal")
        assert quiet_judge("enforce", base, "base", tmp) == 0
    print("record_freeze: selftest passed")


def main(argv):
    if argv == ["--selftest"]:
        selftest()
        return 0
    if argv == ["--notice"]:
        print(NOTICE)
        return 0
    if argv[:1] == ["--moves"] and len(argv) > 1:
        tracked, texts = working_tree()
        moves = moving(tracked, texts)
        return 0 if all(p in moves for p in argv[1:]) else 1
    if argv == []:
        mode, base, what = context(os.environ, scope_merge_base)
        return judge(mode, base, what)
    if len(argv) == 3 and argv[0] == "--base" and argv[2] == "--enforce":
        return judge("enforce", argv[1], f"base {argv[1]}")
    print(__doc__.split("\n\n")[3], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
