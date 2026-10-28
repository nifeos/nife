"""The host-exclusion wiring gate: every consumer CALLS the derivation, and nothing else lists it.

Milestone 393 (four files each record which crates cannot compile for the host, and one of them derives it) put the set in
helpers/host-excluded-crates.sh. script/lint's "the host pass excludes exactly the bare-metal
crates" section checks the derivation against `cargo metadata`; this helper checks the other half,
that nothing bypasses it.

    python3 helpers/host_exclusion_wiring.py --check     # what script/lint runs
    python3 helpers/host_exclusion_wiring.py --selftest  # the traps, as fixtures

Two checks, each closing a hole the first version of the gate had:

1. **The call, not the name.** A consumer passed the first version if its text merely CONTAINED
   "helpers/host-excluded-crates.sh", and a comment contains it. Replacing the call in
   script/mutation with a hand list and leaving the comment kept the gate green. Now comments are
   stripped first (full-line and trailing, quote-aware) and the actual invocation is required:
   `$(helpers/host-excluded-crates.sh` in the shell consumers, `.arg("helpers/host-excluded-crates.sh")`
   in xtask. script/coverage makes two calls (names, and `paths` for the floor).
2. **A repo-wide ban on the literals.** `--exclude <one of the derived crates>` anywhere outside the
   helper is a hand copy growing back. script/lint's lint-ratchet pass kept one for the whole life
   of the "four copies collapsed to one" claim, because the marker count only saw the passes that
   used the marker. The ban reads every tracked or untracked-unignored text file outside
   helpers/host-excluded-crates.sh, notes/ and design/ (prose may name the old list), and fails
   naming file:line. It also matches the split forms xtask would use (`"--exclude", "user_mode_runtime"`,
   including across a line break) and `--exclude=name`.

A ban that matched nothing would pass by checking nothing (milestone 401 (a gate that selects the set it judges can pass by checking nothing)), so `--check` fails if it
scanned fewer than `MIN_SCANNED` files or did not scan the consumers, and the selftest proves each
form fires.

Every literal this file would match is assembled from parts, so the ban does not hit its own source.

Name: provisional, minted by the lane that fixed the wiring gate of milestone 393 (four files each record which crates cannot compile for the host, and one of them derives it).
"""
import re
import subprocess
import sys

EMITTER = "helpers/host-excluded-crates.sh"
FLAG = "--" + "exclude"
BANNED = ["user_mode_runtime", "hot_swap_contract", "supervision_contract", "system_initializer"]
BAN_RE = re.compile(
    re.escape(FLAG) + r"""[\s"',=]+(?:%s)\b""" % "|".join(BANNED))
BAN_SKIP_PREFIXES = ("notes/", "design/", "vendor/")
BAN_SKIP_FILES = (EMITTER, "helpers/host_exclusion_wiring.py")
MIN_SCANNED = 500

# The marker for script/lint's `cargo clippy --workspace` passes: the judgment excludes, then the
# derived flags. Assembled so a count over script/lint does not see this file.
CLIPPY_MARKER = " ".join([FLAG, "kernel", FLAG, "system_tests", "$bare_excludes"])
CLIPPY_PASSES = 3  # the two -D warnings passes and the lint-ratchet pass


def strip_comments(text, line_prefix, quotes):
    """Drop `line_prefix` comments (full-line and trailing) outside quoted strings."""
    out = []
    for line in text.split("\n"):
        quote = None
        cut = len(line)
        i = 0
        while i < len(line):
            c = line[i]
            if quote:
                if c == "\\" and quote == '"':
                    i += 1
                elif c == quote:
                    quote = None
            elif c in quotes:
                quote = c
            elif line.startswith(line_prefix, i) and (
                    line_prefix != "#" or i == 0 or line[i - 1] in " \t;"):
                cut = i
                break
            i += 1
        out.append(line[:cut])
    return "\n".join(out)


def sh_code(text):
    return strip_comments(text, "#", "\"'")


def rs_code(text):
    return strip_comments(text, "//", '"')


CALL_NAMES = re.escape("$(" + EMITTER + ")")
CALL_PATHS = re.escape("$(" + EMITTER + " paths)")
XTASK_CALL = re.escape('.arg("' + EMITTER + '")')


def wiring_problems(files):
    """`files` maps path to text. Returns a list of problem strings."""
    bad = []

    def need(path, label, pattern, code, count=1):
        n = len(re.findall(pattern, code))
        if n < count:
            bad.append(f"{path}: {label} (found {n}, want {count}); {EMITTER} must be CALLED in code, "
                       "not named in a comment")

    lint = sh_code(files["script/lint"])
    marker = lint.count(CLIPPY_MARKER)
    if marker != CLIPPY_PASSES:
        bad.append(f"script/lint: {marker} cargo clippy --workspace pass(es) build their {FLAG} flags "
                   f"from $bare_excludes, want {CLIPPY_PASSES} (the two -D warnings passes and the "
                   f"lint-ratchet pass); each must follow `{FLAG} kernel {FLAG} system_tests "
                   "$bare_excludes`")
    need("script/lint", "bare_crates=$(...) derivations", re.escape("bare_crates=") + CALL_NAMES,
         lint, CLIPPY_PASSES)
    cov = sh_code(files["script/coverage"])
    need("script/coverage", "the run's names call", CALL_NAMES, cov)
    need("script/coverage", "the floor's paths call", CALL_PATHS, cov)
    need("script/mutation", "the cargo-mutants paths call", CALL_PATHS, sh_code(files["script/mutation"]))
    xtask = "\n".join(rs_code(t) for p, t in sorted(files.items()) if p.startswith("xtask/src/"))
    need("xtask/src/", "the Command invocation", XTASK_CALL, xtask)
    return bad


def ban_hits(files):
    """`files` maps path to text. Returns [(path, line, text)] for each banned literal."""
    hits = []
    for path, text in sorted(files.items()):
        if path in BAN_SKIP_FILES or path.startswith(BAN_SKIP_PREFIXES):
            continue
        for m in BAN_RE.finditer(text):
            line = text.count("\n", 0, m.start()) + 1
            hits.append((path, line, " ".join(m.group(0).split())))
    return hits


def tree_files():
    names = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        capture_output=True, check=True).stdout.decode().split("\0")
    files = {}
    for n in names:
        if not n:
            continue
        try:
            with open(n, encoding="utf-8") as f:
                files[n] = f.read()
        except (UnicodeDecodeError, OSError):
            continue
    return files


def check():
    files = tree_files()
    problems = []
    for needed in ("script/lint", "script/coverage", "script/mutation", "xtask/src/suite.rs"):
        if needed not in files:
            problems.append(f"{needed} was not read, so the wiring check would have checked nothing")
    if len(files) < MIN_SCANNED:
        problems.append(f"only {len(files)} text files scanned (want at least {MIN_SCANNED}); the ban "
                        "would pass by looking at nothing (milestone 401 (a gate that selects the set it judges can pass by checking nothing))")
    if problems:
        sys.exit("lint: host-exclusion wiring gate cannot run:\n  " + "\n  ".join(problems))
    problems = wiring_problems(files)
    hits = ban_hits(files)
    if hits:
        problems.append("a hand copy of the host-excluded crate set (derive it with " + EMITTER + "):")
        problems += [f"  {p}:{n}: {t}" for p, n, t in hits]
    if problems:
        sys.exit("lint: the host-exclusion derivation is not wired to every consumer:\n  "
                 + "\n  ".join(problems))
    print(f"host-exclusion wiring: every consumer calls {EMITTER}, no hand copy in {len(files)} files")


def selftest():
    call = "$(" + EMITTER + ")"
    callp = "$(" + EMITTER + " paths)"
    marker = CLIPPY_MARKER
    good = {
        "script/lint": ("bare_crates=" + call + "\n" + marker + " x\n") * CLIPPY_PASSES,
        "script/coverage": "a=" + call + "\nb=" + callp + "\n",
        "script/mutation": "p=" + callp + "\n",
        "xtask/src/suite.rs": '.arg("' + EMITTER + '")\n',
    }
    assert wiring_problems(good) == [], wiring_problems(good)

    def broke(path, text):
        files = dict(good)
        files[path] = text
        return wiring_problems(files)

    # A hand list with the helper named only in a comment, full-line and trailing.
    assert broke("script/mutation", "# " + EMITTER + "\np=\"a b\"\n"), "comment must not satisfy"
    assert broke("script/mutation", "p=\"a b\" # " + callp + "\n"), "trailing comment must not satisfy"
    assert broke("script/coverage", "a=" + call + "\n# b=" + callp + "\n"), "coverage needs both calls"
    assert broke("script/coverage", "b=" + callp + "\n"), "coverage needs the names call"
    assert broke("xtask/src/suite.rs", '// .arg("' + EMITTER + '")\n'), "xtask comment"
    assert broke("script/lint", ("bare_crates=" + call + "\n" + marker + "\n") * (CLIPPY_PASSES - 1))
    # A quoted '#' is not a comment.
    assert not broke("script/mutation", 'echo "#"; p=' + callp + "\n")

    def hits(text, path="script/x"):
        return ban_hits({path: text})

    for form in (FLAG + " user_mode_runtime", FLAG + "=hot_swap_contract",
                 '"' + FLAG + '", "supervision_contract"',
                 '"' + FLAG + '",\n    "system_initializer"'):
        assert hits("a\n" + form + "\n"), "ban must fire on " + form
    assert hits("a\n" + '"' + FLAG + '",\n    "system_initializer"')[0][1] == 2
    assert not hits(FLAG + " kernel " + FLAG + " xtask")
    assert not hits(FLAG + " user_mode_runtime_extra"), "a longer crate name is not a hit"
    for skipped in (EMITTER, "notes/a.md", "design/b.md"):
        assert not hits(FLAG + " user_mode_runtime", skipped), skipped
    print("host-exclusion wiring selftest: ok")


if __name__ == "__main__":
    if sys.argv[1:] == ["--check"]:
        check()
    elif sys.argv[1:] == ["--selftest"]:
        selftest()
    else:
        sys.exit(__doc__)
