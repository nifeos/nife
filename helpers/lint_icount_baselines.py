#!/usr/bin/env python3
# helpers/lint_icount_baselines.py: every bench/baseline-*.txt names the pinned nightly.
#
# Extracted from script/lint on 2026-09-30 so .githooks/pre-push can run exactly this check
# (script/lint --baselines): a pin that moves without its floors is the one gate failure that
# is about tree state rather than code shape, so no code-level hook could see it, and #1463's
# first push reached CI red on it. script/lint still runs this file in its full pass; there is
# one copy of the check and two doors into it.
import os
import pathlib
import re
import sys

if os.environ.get("NIFE_BUMP_IN_PROGRESS"):
    print("the icount baselines name the pinned nightly: SKIPPED (NIFE_BUMP_IN_PROGRESS; "
          "script/toolchain-bump is mid-bump, and CI on its pull request is the real gate)")
    sys.exit(0)

pin = None
for line in pathlib.Path("rust-toolchain.toml").read_text().splitlines():
    if line.lstrip().startswith("channel"):
        m = re.search(r'"([^"]+)"', line)
        if m:
            pin = m.group(1)
        break
if pin is None:
    print("lint: rust-toolchain.toml has no channel line to check the baselines against", file=sys.stderr)
    sys.exit(1)

STAMP = re.compile(r"^#\s*toolchain:\s*(\S+)\s*$")
bad, checked = [], 0
for path in sorted(pathlib.Path("bench").glob("baseline-*.txt")):
    checked += 1
    got = None
    for line in path.read_text().splitlines():
        m = STAMP.match(line)
        if m:
            got = m.group(1)
            break
    if got is None:
        bad.append(f"{path.as_posix()}: no '# toolchain: <channel>' line")
    elif got != pin:
        bad.append(f"{path.as_posix()}: recorded against {got}, but rust-toolchain.toml pins {pin}")

if bad:
    print("lint: the icount baselines do not name the pinned nightly:", file=sys.stderr)
    for b in bad:
        print(f"        {b}", file=sys.stderr)
    print(f"""
A compiler bump changes the instruction sequences these floors count, so the
numbers are revalued by a change no commit in this branch is responsible for.
The bump workflow's restamp job carries a floor across when it proves the
compiler moved no row by 0.5% or more (its table is in the pull request body).
Every floor named above is one it could not carry. Re-record it HERE, in the
branch that raises the pin, so the delta is seen by the person causing it:

    script/bench --save --why "nightly-<date>: <what the new compiler moved>"
    script/bench --riscv64 --save --why "..."
    script/bench --x86 --save --why "..."

--why is not optional: a floor records the reason it holds its value, beside the
number, because a commit message is read once on the day it is written (milestone
302). Say the same thing in the commit message. If a floor moved a lot, that is the
gate working: a nightly can make this kernel genuinely slower, and re-recording
without reading the delta is the one outcome this refuses to automate.""", file=sys.stderr)
    sys.exit(1)

# An empty glob here would print "0 file(s)" and pass (milestone 401): the selector is
# `bench/baseline-*.txt`, and renaming those files is a thing a benchmark milestone does.
if checked == 0:
    sys.exit("lint: bench/baseline-*.txt matched no file, so the toolchain-stamp check "
             "judged nothing. The baseline file naming moved; move the glob with it.")
print(f"the icount baselines name the pinned nightly: {checked} file(s) at {pin}")
