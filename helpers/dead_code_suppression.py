#!/usr/bin/env python3
"""No module-wide dead-code suppression, anywhere: the sweep script/lint runs, in one interpreter.

    helpers/dead_code_suppression.py            # every tracked .rs file outside vendor/ and patches/
    helpers/dead_code_suppression.py --selftest

Milestone 41 (dead code: triage the suppressions, and un-blindfold the gate) and §38 (a suppression
is scoped to an item and carries a reason, or it does not ship) are the rule and script/lint's section "module-wide dead-code
suppression" is where it is explained. An inner attribute (`#![...]`) applies to the whole enclosing
module or crate, so one that names `dead_code` or any `unused` lint is a blanket and never a
decision about an item. The attribute is read bracket-matched, because `rustfmt` splits a long
`cfg_attr` across lines: from the line that opens `#![` until its parentheses balance.

One exemption, and it is not dead code: an attribute whose whole list is
`allow(unused_crate_dependencies)`. That lint is about the manifest and fires per target, and a bin
target can only silence it crate-wide (milestone 609 (the system tests leave the kernel crate)).

Exit 0 when nothing is found, 1 with one `path:line: attribute` per hit on stdout, 3 when no .rs
file was read at all (a wrong filter must not read as a pass; milestone 401 (a gate that selects the set it judges can pass by
checking nothing)).

Milestone 980 (script/lint stops paying per-item costs), part 4. This replaces one `awk` process per
file (1,631 of them) and keeps its verdict: the same start rule, the same parenthesis count (every
`(` and `)` on each line, strings included), the same two patterns, and a restart when a second `#![`
opens before the first balances. A hit now carries its path on every line, where the awk printed it
on the first.

Name: provisional, minted 2026-10-31 (UTC) by lane/980-dead-code-one-interpreter.
"""

import re
import subprocess
import sys

# POSIX [[:space:]], which is what the awk it replaces matched: not Python's wider \s.
SPACE = " \t\n\r\f\v"
START = re.compile(rf"^[{SPACE}]*#!\[")
BLANKET = re.compile(r"dead_code|unused")
EXEMPT = re.compile(rf"^[{SPACE}]*#!\[allow\(unused_crate_dependencies\)\][{SPACE}]*$")


def hits(text):
    """`[(line, attribute)]` for each module-wide suppression in one file's text."""
    found, collecting, buf, start, opens, closes = [], False, "", 0, 0, 0
    for number, line in enumerate(text.split("\n"), 1):
        if START.match(line):
            collecting, buf, start, opens, closes = True, "", number, 0, 0
        if collecting:
            buf += " " + line
            opens += line.count("(")
            closes += line.count(")")
            if opens <= closes:
                if BLANKET.search(buf) and not EXEMPT.match(buf):
                    found.append((start, buf.lstrip(SPACE)))
                collecting = False
    return found


def tracked():
    out = subprocess.run(["git", "ls-files", "-z", "*.rs"], capture_output=True, check=True).stdout
    names = [n.decode("utf-8", "surrogateescape") for n in out.split(b"\0") if n]
    return [n for n in names if not n.startswith(("vendor/", "patches/"))]


def sweep(read=None):
    """`(files read, ["path:line: attribute", ...])` over the tracked tree."""
    read = read or (lambda p: open(p, encoding="utf-8", errors="surrogateescape").read())
    seen, lines = 0, []
    for path in tracked():
        seen += 1
        lines += [f"{path}:{n}: {attr}" for n, attr in hits(read(path))]
    return seen, lines


def selftest():
    cases = [
        ("blanket dead_code", "#![allow(dead_code)]\nfn f() {}\n", [(1, "#![allow(dead_code)]")]),
        ("any unused lint", "  #![allow(unused_variables)]\n", [(1, "#![allow(unused_variables)]")]),
        ("split cfg_attr",
         "fn a() {}\n#![cfg_attr(\n    target_arch = \"riscv64\",\n    allow(dead_code)\n)]\n",
         [(2, '#![cfg_attr(     target_arch = "riscv64",     allow(dead_code) )]')]),
        ("the one exemption", "#![allow(unused_crate_dependencies)]\n", []),
        ("exemption with company",
         "#![allow(unused_crate_dependencies, dead_code)]\n",
         [(1, "#![allow(unused_crate_dependencies, dead_code)]")]),
        ("per-item allow", "#[allow(dead_code)]\nfn f() {}\n", []),
        ("an inner attribute about something else", "#![no_std]\n#![deny(missing_docs)]\n", []),
        ("restart on a second opening",
         "#![cfg_attr(\n#![allow(dead_code)]\n", [(2, "#![allow(dead_code)]")]),
        ("never closed", "#![allow(dead_code\nfn f() {}\n", []),
        ("two hits", "#![allow(dead_code)]\n\n#![allow(unused)]\n",
         [(1, "#![allow(dead_code)]"), (3, "#![allow(unused)]")]),
    ]
    wrong = [name for name, text, want in cases if hits(text) != want]
    if wrong:
        print(f"dead-code-suppression selftest: wrong verdict on {wrong}", file=sys.stderr)
        return 1
    print(f"dead-code-suppression selftest: {len(cases)} fixtures, each judged as expected")
    return 0


def main(argv):
    if argv[1:] == ["--selftest"]:
        return selftest()
    if argv[1:]:
        print("usage: dead_code_suppression.py [--selftest]", file=sys.stderr)
        return 2
    seen, lines = sweep()
    if seen == 0:
        return 3
    if lines:
        sys.stdout.buffer.write(("\n".join(lines) + "\n").encode("utf-8", "surrogateescape"))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
