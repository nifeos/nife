#!/usr/bin/env python3
"""crate_hardware_run.py: judge whether board transcripts are a hardware run of a jh7110_entropy version.

    crate_hardware_run.py judge <log>... [--nife-commit <sha>] [--tag-commit <sha>]
    crate_hardware_run.py --selftest

`judge` reads one or more console logs of nife booting on a VisionFive 2, splits them into boots at
the kernel's `nife on ` banner, and reads each boot's `self-test : trng` line. It prints one row per
condition of milestone 1035's definition and ends `PASS` or `FAIL, N problem(s)`, exiting 0 or 1:

1. Every boot names the same crate version V and commit C, and C is the commit of the tag `vV` on
   nifeos/jh7110_entropy. The version string alone does not settle it: the crate's `Cargo.toml`
   can carry a version whose tag points at another commit. With `--nife-commit`, that nife commit
   is on `origin/main` and its `Cargo.lock` resolves the crate to the same V and C.
2. At least two boots carry the line.
3. On each boot the line is `ok`: both 32-byte draws came back full and the second differs.
4. Each line's mode note says 256-bit.
5. No line is `FAILED`, which is where a persistent `LFSR_LOCKUP` lands.
6. The first draws of the boots differ from each other.

The tag's commit comes from `git ls-remote` against the crate's repository, so `judge` needs the
network; `--tag-commit` gives it instead, for a reader offline. The steps that produce the logs are in
design/roadmap/1035-a-crate-names-the-version-that-ran-on-hardware/bench-steps.md.

Name: provisional.
"""

import re
import subprocess
import sys
import tempfile
import os

CRATE = "jh7110_entropy"
CRATE_REPOSITORY = "https://github.com/nifeos/jh7110_entropy"
BANNER = re.compile(r"^nife on ")
TRNG = re.compile(r"self-test\s+: trng\s+(ok|FAILED|absent)\s+(.*)$")
STAT = re.compile(r"STAT 0x[0-9a-f]{8} \(([^)]*)\)")
DRAWS = re.compile(r"draws (\d+)/(\d+) bytes")
FIRST = re.compile(r"first ([0-9a-f]{8})\.\.")
DIFFER = re.compile(r"differ (true|false)")
IDENTITY = re.compile(r"crate " + CRATE + r" (\S+) at (\S+)$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")


def boots(texts):
    """Each boot's trng line, in order, across the logs given as text."""
    found = []
    for text in texts:
        current = None
        for raw in text.splitlines():
            line = raw.replace("\r", "").rstrip()
            if BANNER.match(line):
                current = len(found)
                found.append(None)
                continue
            m = TRNG.search(line)
            if m and current is not None and found[current] is None:
                found[current] = (m.group(1), m.group(2))
    return [b for b in found if b is not None]


def read_line(verdict, detail):
    """The fields of one trng line, None where the line does not carry one."""

    def get(pattern, group=1):
        m = pattern.search(detail)
        return m.group(group) if m else None

    return {
        "verdict": verdict,
        "mode": get(STAT),
        "draws": (get(DRAWS, 1), get(DRAWS, 2)),
        "first": get(FIRST),
        "differ": get(DIFFER),
        "version": get(IDENTITY, 1),
        "commit": get(IDENTITY, 2),
    }


def tag_commit(version):
    """The commit the crate's tag `v<version>` names, peeled, or None."""
    out = subprocess.run(
        ["git", "ls-remote", CRATE_REPOSITORY, f"refs/tags/v{version}", f"refs/tags/v{version}^{{}}"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout
    refs = dict(reversed(line.split("\t")) for line in out.splitlines() if "\t" in line)
    return refs.get(f"refs/tags/v{version}^{{}}") or refs.get(f"refs/tags/v{version}")


def nife_lock(nife_commit, git=("git",)):
    """(on main, version, commit) of the crate in nife_commit's Cargo.lock."""
    on_main = (
        subprocess.run(
            [*git, "merge-base", "--is-ancestor", nife_commit, "origin/main"],
            capture_output=True,
            check=False,
        ).returncode
        == 0
    )
    lock = subprocess.run(
        [*git, "show", f"{nife_commit}:Cargo.lock"], capture_output=True, text=True, check=False
    ).stdout
    for block in lock.split("[[package]]"):
        fields = dict(
            re.findall(r'^(name|version|source) = "([^"]*)"$', block, flags=re.MULTILINE)
        )
        if fields.get("name") == CRATE:
            commit = fields.get("source", "").rpartition("#")[2]
            return on_main, fields.get("version"), commit
    return on_main, None, None


def judge(texts, tag=None, resolve_tag=tag_commit, nife=None):
    """The rows and the problem count for these logs. `nife` is nife_lock's answer, or None."""
    rows = []
    problems = 0

    def row(ok, what):
        nonlocal problems
        rows.append(f"{'PASS' if ok else 'FAIL'}  {what}")
        if not ok:
            problems += 1

    lines = [read_line(*b) for b in boots(texts)]
    for i, line in enumerate(lines, 1):
        rows.append(
            f"boot {i}: {line['verdict']}, first {line['first']}, mode {line['mode']!r}, "
            f"crate {line['version']} at {line['commit']}"
        )

    identities = {(line["version"], line["commit"]) for line in lines}
    version, commit = next(iter(identities)) if len(identities) == 1 else (None, None)
    pinned = version is not None and commit is not None and COMMIT.match(commit) is not None
    row(pinned, f"1. every boot names one crate version and commit: {sorted(identities, key=str)}")
    if pinned:
        expected = tag or resolve_tag(version)
        row(expected == commit, f"1. the tag v{version} names {expected}, the boots ran {commit}")
    if nife is not None:
        on_main, lock_version, lock_commit = nife
        row(on_main, "1. the nife commit is on origin/main")
        row(
            (lock_version, lock_commit) == (version, commit),
            f"1. the nife commit's Cargo.lock resolves {CRATE} {lock_version} at {lock_commit}",
        )
    row(len(lines) >= 2, f"2. {len(lines)} boot(s) carry a trng line, and the run needs two")
    row(
        bool(lines) and all(
            line["verdict"] == "ok" and line["draws"] == ("32", "32") and line["differ"] == "true"
            for line in lines
        ),
        "3. every trng line is ok, with two full 32-byte draws that differ",
    )
    row(
        bool(lines) and all((line["mode"] or "").startswith("256-bit") for line in lines),
        "4. every mode note says 256-bit",
    )
    row(all(line["verdict"] != "FAILED" for line in lines), "5. no trng line is FAILED")
    firsts = [line["first"] for line in lines]
    row(
        len(firsts) >= 2 and None not in firsts and len(set(firsts)) == len(firsts),
        f"6. the first draws differ across boots: {firsts}",
    )
    return rows, problems


def selftest():
    """What this file promises, on synthetic logs."""
    commit = "cfdbeb78e4f63eb251cd11b5dcf1521d4ed0ca06"
    other = "53b022b55b42a6dfc7eb670c438f0d3463da8db5"

    def trng(first, verdict="ok", mode="256-bit: all eight RAND words are the answer", c=commit, v="0.2.0"):
        return (
            f"  self-test       : trng       {verdict:<6}  JH7110 TRNG at 0x1600c000 (tree says disabled): "
            f"report 0x524e475550, STAT 0x00040308 ({mode}), draws 32/32 bytes, first {first}.., "
            f"differ true; bring-up 551 us, 64 bytes in 68 us (937728 bytes/s); crate {CRATE} {v} at {c}\r"
        )

    def log(*lines):
        out = []
        for line in lines:
            out += ["nife on RISC-V (rv64, S-mode, Sv39)", line]
        return "\n".join(out)

    def problems(texts, tag=commit, nife=None):
        return judge(texts, tag=tag, nife=nife)[1]

    good = log(trng("93a34471"), trng("f6c2c13e"))
    assert problems([good]) == 0, judge([good], tag=commit)[0]
    # Two logs of one boot each are two boots.
    assert problems([log(trng("93a34471")), log(trng("f6c2c13e"))]) == 0
    # One boot is not a run, and has no second first draw to differ from.
    assert problems([log(trng("93a34471"))]) == 2
    # The same first draw across boots fails condition 6.
    assert problems([log(trng("93a34471"), trng("93a34471"))]) == 1
    # A 128-bit mode note fails condition 4.
    assert problems([log(trng("93a34471"), trng("f6c2c13e", mode="128-BIT: only RAND0..3"))]) == 1
    # A FAILED line fails conditions 3 and 5.
    assert problems([log(trng("93a34471"), trng("f6c2c13e", verdict="FAILED"))]) == 2
    # A tag that names another commit fails condition 1, whatever the version string says.
    assert problems([good], tag=other) == 1
    # Boots from two builds are not one run.
    assert problems([log(trng("93a34471"), trng("f6c2c13e", c=other))]) >= 1
    # A kernel built without the identity names `unknown`, which no tag matches.
    assert problems([log(trng("93a34471", c="unknown"), trng("f6c2c13e", c="unknown"))]) >= 1
    # A line before any banner belongs to no boot.
    assert problems([trng("93a34471") + "\n" + good]) == 0
    # The nife commit must be on main and lock the same crate commit.
    assert problems([good], nife=(True, "0.2.0", commit)) == 0
    assert problems([good], nife=(False, "0.2.0", commit)) == 1
    assert problems([good], nife=(True, "0.2.0", other)) == 1
    # nife_lock reads a Cargo.lock out of a git commit.
    with tempfile.TemporaryDirectory() as tmp:
        git = ("git", "-C", tmp)
        env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}
        subprocess.run([*git, "init", "-q"], check=True, env=env)
        with open(os.path.join(tmp, "Cargo.lock"), "w") as f:
            f.write(
                f'[[package]]\nname = "{CRATE}"\nversion = "0.2.0"\n'
                f'source = "git+{CRATE_REPOSITORY}?rev={commit}#{commit}"\n'
            )
        subprocess.run([*git, "add", "Cargo.lock"], check=True, env=env)
        subprocess.run([*git, "commit", "-q", "-m", "lock"], check=True, env=env)
        on_main, version, locked = nife_lock("HEAD", git=git)
        assert (version, locked) == ("0.2.0", commit), (version, locked)
        assert on_main is False
    print("crate_hardware_run.py: selftest passed")


def main(argv):
    if argv[1:] == ["--selftest"]:
        selftest()
        return 0
    if len(argv) < 3 or argv[1] != "judge":
        print(__doc__, file=sys.stderr)
        return 2
    args = argv[2:]
    tag = nife_commit = None
    logs = []
    while args:
        arg = args.pop(0)
        if arg == "--tag-commit" and args:
            tag = args.pop(0)
        elif arg == "--nife-commit" and args:
            nife_commit = args.pop(0)
        else:
            logs.append(arg)
    texts = []
    for path in logs:
        with open(path, "rb") as f:
            texts.append(f.read().decode("latin-1"))
    nife = nife_lock(nife_commit) if nife_commit else None
    rows, problems = judge(texts, tag=tag, nife=nife)
    print("\n".join(rows))
    print("PASS" if problems == 0 else f"FAIL, {problems} problem(s)")
    return 0 if problems == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
