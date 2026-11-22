#!/usr/bin/env python3
"""A boot-line head is spelled once, in `serial_console`'s heads module; every other site uses it.

    helpers/boot_line_heads.py              # script/lint: fail on a head spelled by hand
    helpers/boot_line_heads.py --selftest   # fixtures, no tree; script/lint runs it first

Name provisional; naming is an architect's. A python module under `helpers/`, outside
`script/names`' scope.

# WHY

The kernel, the self-test program and the shell print their boot lines through
`boot_ladder::Heads`, and every reader matches through the same value. A gate that spells
`"nife capability shell"` by hand instead passes every unit test after a wording change and then
fails as a timeout, which reads like a hang. `boot_ladder` cannot see its consumers, so this check
is nife's.

# WHAT IT READS

The heads come from `crates/serial_console/src/heads.rs`: the string literals of
`the_heads_print_the_bytes_the_logs_hold`, the test that pins nife's heads to the bytes stored logs
hold. That test is the one place a head is spelled out, so this check copies nothing.

A head counts when a quote opens on it (`"` or `'`, so an f-string and a raw string count too),
or opens on it after a `^`, as a regular expression anchors it. The
check skips itself, Markdown, `design/` and `notes/`, a file named `tests.rs`, anything under a `tests/`
or `fixtures/` directory, comment lines, and everything in a Rust file from its first
`#[cfg(test)]` on, since a test spells the bytes it expects on purpose. A site that must keep a
copy, such as a script in a language that cannot link the crate, says so on the line above:

    # boot-line-head: copy, <reason>

What this cannot see: a head assembled from pieces, a head in a file git does not track, or a
Rust test module that is not at the end of its file.
"""
import re
import subprocess
import sys

HEADS_FILE = "crates/serial_console/src/heads.rs"
SELF = "helpers/boot_line_heads.py"
HEADS_TEST = "fn the_heads_print_the_bytes_the_logs_hold"
HEAD_COUNT = 7
EXEMPT = re.compile(r"boot-line-head:\s*copy,\s*\S")
COMMENT = re.compile(r"^\s*(//|#(?!\[)|\*|/\*)")
CFG_TEST = re.compile(r"^\s*#\[cfg\(test\)\]")
LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')


def heads_from(source):
    """The heads the pinning test spells, in order; refuses a test that is missing or changed shape."""
    start = source.find(HEADS_TEST)
    if start < 0:
        raise SystemExit("boot_line_heads: %s has no `%s`" % (HEADS_FILE, HEADS_TEST))
    body = source[start:]
    end = body.find("\n}")
    body = body[: end if end >= 0 else len(body)]
    heads = [s for s in LITERAL.findall(body) if s.startswith("nife")]
    if len(heads) != HEAD_COUNT:
        raise SystemExit(
            "boot_line_heads: `%s` spells %d heads, not %d; update HEAD_COUNT with the heads"
            % (HEADS_TEST, len(heads), HEAD_COUNT)
        )
    return heads


def skipped(path):
    parts = path.split("/")
    return (
        path in (HEADS_FILE, SELF)
        or path.endswith(".md")
        or parts[0] in ("design", "notes")
        or parts[-1] == "tests.rs"
        or "tests" in parts[:-1]
        or "fixtures" in parts[:-1]
    )


def refusals(path, text, heads):
    """(line_number, head, line) for each head this file spells by hand."""
    pattern = re.compile("[\"'](?:\\^)?(%s)" % "|".join(re.escape(h) for h in heads))
    out = []
    previous = ""
    for number, line in enumerate(text.splitlines(), 1):
        if path.endswith(".rs") and CFG_TEST.match(line):
            break
        hit = pattern.search(line)
        if hit and not COMMENT.match(line) and not EXEMPT.search(previous) and not EXEMPT.search(line):
            out.append((number, hit.group(1), line.strip()))
        previous = line
    return out


def check_tree():
    with open(HEADS_FILE, encoding="utf-8") as f:
        heads = heads_from(f.read())
    files = subprocess.run(["git", "ls-files", "-z"], check=True, capture_output=True).stdout
    bad = []
    for path in files.decode().split("\0"):
        if not path or skipped(path):
            continue
        try:
            with open(path, encoding="utf-8") as f:
                text = f.read()
        except (UnicodeDecodeError, FileNotFoundError, IsADirectoryError):
            continue
        for number, head, line in refusals(path, text, heads):
            bad.append("%s:%d: spells the head %r by hand: %s" % (path, number, head, line))
    if bad:
        print("\n".join(bad), file=sys.stderr)
        print(
            "lint: use serial_console::heads::HEADS (or boot_ladder::Heads) instead, or mark a copy"
            " that cannot link the crate with `boot-line-head: copy, <reason>` on the line above",
            file=sys.stderr,
        )
        sys.exit(1)


def selftest():
    pinning = (
        "pub const HEADS: Heads = Heads::new(\"nife\");\n"
        + HEADS_TEST
        + "() {\n    assert_eq!(printed, [\n"
        + "".join(
            '        "%s",\n' % h
            for h in (
                "nife on ",
                "nife machine: ",
                "nife self-test: ",
                "nife self-test subject: thread ",
                "nife: the capability core runs on ",
                "nife capability shell",
                "nife screen: held for the host; ",
            )
        )
        + "    ]);\n}\n"
    )
    heads = heads_from(pinning)
    assert heads[5] == "nife capability shell", heads
    try:
        heads_from(pinning.replace('        "nife machine: ",\n', ""))
        raise AssertionError("a pinning test that lost a head must refuse")
    except SystemExit:
        pass
    refused = [
        ("xtask/src/install.rs", 'let banner = "nife capability shell";\n'),
        ("script/rehearsal", 'MARKER = "nife self-test: "\n'),
        ("helpers/run.py", 'out = f"nife on {arch}"\n'),
        ("helpers/run.py", 'BANNER = re.compile(r"^nife on ")\n'),
        ("xtask/src/x.rs", "fn f() {}\n#[cfg(tests)]\nlet a = \"nife machine: \";\n"),
    ]
    passed = [
        ("xtask/src/install.rs", "let banner = HEADS.shell_banner();\n"),
        ("xtask/src/x.rs", 'fn f() {}\n#[cfg(test)]\nmod tests { const L: &str = "nife on x"; }\n'),
        ("xtask/src/x.rs", '/// let log = "nife on aarch64";\n'),
        ("script/rehearsal", '# the head is "nife self-test: "\n'),
        ("script/rehearsal", '# boot-line-head: copy, Python cannot link the crate\nM = "nife self-test: "\n'),
        ("script/board-image", 'description = "nife on the TH1520"; /* boot-line-head: copy, a tree title */\n'),
        ("xtask/src/x.rs", 'let s = "the nife on-call";\n'),
        ("helpers/run.py", 'reads the kernel `nife on ` banner\n'),
    ]
    for path, text in refused:
        assert refusals(path, text, heads), (path, text)
    for path, text in passed:
        assert not refusals(path, text, heads), (path, text)
    for path in (SELF, "README.md", "notes/x.txt", "crates/a/src/tests.rs", "crates/a/tests/t.rs", "fixtures/src/f.rs", HEADS_FILE):
        assert skipped(path), path
    assert not skipped("xtask/src/stick.rs")
    print("boot_line_heads: selftest ok (%d refusals, %d passes)" % (len(refused), len(passed)))


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        selftest()
    else:
        check_tree()
