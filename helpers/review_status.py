#!/usr/bin/env python3
"""helpers/review_status.py: the crates' outside reviews, one row per crate, read from the record.

    script/review-status              one row per review: done, open, waiting, at-move counts, open P0s
    script/review-status --open       every finding that is not Done, with its milestone and severity
    script/review-status --check      refuse a status line outside the closed set (script/lint runs it)
    script/review-status --selftest   run the parser and the check over fixtures, no tree

A crate review is a roadmap block whose title reads "`<crate>`: what an outside maintainer would fix
before ...", "`<crate>`: the outside review's findings land before ..." or "`<crate>` ...: the
release review's findings", and whose status is not SUPERSEDED. Each carries a `## Finding status`
section: optional opening prose, then one bullet per finding in the review's own order. A bullet
may wrap onto indented lines. Its shape is

    - <label>: <state><rest>

where <label> names the finding ("Item 3 (P1), 12-byte `reg`", "P1-3, the ring layout",
"Decision 2, `size_bytes`") and carries its severity, as `(P0)` or as the review's own `P0-1` id,
when the review gave one. <state> is one of the
closed set below, and the check holds each to what it needs.

    Done              the finding is fixed; the bullet cites the pull request that fixed it, as
                      `#N`, `[#N](...)` or `owner/repository#N`.
    Open              nobody has fixed it.
    At the move       the move commit does it (the crate's own repository files, its badges).
    At the release    the release commit does it (the manifest's `publish = false`, the changelog).
    Waiting on <what> it needs something else first: a ruling, a board run, another pull request.
    Will not fix      ruled out; the bullet gives the reason and the ruling date (ruled YYYY-MM-DD).

A block whose every finding is Done, At the move, At the release or Will not fix is BUILT;
otherwise it is PARTIAL. The check refuses a converted block whose `status:` says otherwise.

The totals are printed and never written into a document, because a derived total in a document goes
stale (ruled 2026-11-03 UTC).

Name: provisional, minted 2026-11-22 (UTC) by review-finding-status. The state words are provisional
too: `Will not fix` stands where the proposal wrote the contraction, and `At the release` is the
release commit's twin of `At the move`.
"""

import argparse
import os
import re
import sys

ROADMAP = "design/roadmap"

TITLE = re.compile(
    r"^# (\d+)\. `([^`]+)`[^:\n]*: (?:what an outside maintainer would fix before"
    r"|the outside review's findings land before|the release review's findings)",
    re.M,
)
STATUS = re.compile(r"^status:\s*(\S+)", re.M)
SECTION = re.compile(r"^## Finding status\n(.*?)(?=^## |\Z)", re.M | re.S)
SEVERITY = re.compile(r"\((P[0-9])\)|^(P[0-9])-\d+\b")
PULL_REQUEST = re.compile(r"(?:\b[\w.-]+/[\w.-]+)?#\d+\b")
RULED = re.compile(r"\bruled\b[^.]*\b\d{4}-\d{2}-\d{2}\b", re.I)

# The closed set, in the order the table prints it. A state is a prefix of the text after the label.
STATES = ("Done", "Open", "Waiting on", "At the move", "At the release", "Will not fix")
TERMINAL = {"Done", "At the move", "At the release", "Will not fix"}
STATE = re.compile(
    r"^(Done|Open|At the move|At the release|Will not fix)(?=[.,;: ]|$)"
    r"|^(Waiting on) \S"
)

# A review named here keeps its older wording until the pull request that converts it lands, and the
# check skips it. Each number sits on its own line, between separator lines, so that pull requests
# converting different reviews delete lines git merges in either order. The last one empties the set.
NOT_YET_CONVERTED = {
    #
    1048,
    #
    1049,
    #
    1050,
    #
    1057,
    #
    1058,
    #
    1059,
    #
    1060,
    #
    1061,
    #
    1071,
    #
    1072,
    #
    1073,
    #
    1074,
    #
    1075,
    #
    1076,
    #
    1077,
    #
    1079,
    #
    1081,
    #
    1082,
    #
    1083,
    #
    1084,
    #
    1085,
    #
    1086,
    #
    1087,
    #
    1088,
    #
    1089,
    #
    1092,
    #
    1094,
    #
    1095,
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
    #
}


def split_label(entry):
    """The label and the text after it: split at the first ': ' outside backticks."""
    ticks = 0
    for i, ch in enumerate(entry):
        if ch == "`":
            ticks ^= 1
        elif ch == ":" and not ticks and entry[i + 1:i + 2] == " ":
            return entry[:i], entry[i + 2:]
    return None, entry


def entries(section):
    """One string per bullet, its wrapped lines joined; prose before the first bullet is skipped."""
    out, current = [], None
    for line in section.splitlines():
        if line.startswith("- "):
            if current is not None:
                out.append(current)
            current = line[2:].strip()
        elif current is not None and line.startswith(" ") and line.strip():
            current += " " + line.strip()
        elif current is not None and not line.strip():
            out.append(current)
            current = None
        elif current is not None:
            out.append(current)
            current = None
            out.append(None)  # prose after a bullet: refused, the section is bullets after its prose
    if current is not None:
        out.append(current)
    return out


def parse_block(text):
    """(number, crate, status, findings, problems) for a review block, or None for any other block.

    Each finding is a dict: label, state, severity, text."""
    title = TITLE.search(text)
    if not title:
        return None
    status = STATUS.search(text)
    status = status.group(1) if status else ""
    if status == "SUPERSEDED":
        return None
    number, crate = int(title.group(1)), title.group(2)
    findings, problems = [], []
    sections = SECTION.findall(text)
    if len(sections) != 1:
        problems.append(f"carries {len(sections)} `## Finding status` sections, not one")
        return number, crate, status, findings, problems
    for entry in entries(sections[0]):
        if entry is None:
            problems.append("prose after a bullet in `## Finding status`; every finding is a bullet")
            continue
        label, rest = split_label(entry)
        short = entry[:70]
        if label is None:
            problems.append(f"no `<label>: <state>` in: {short}")
            continue
        state = STATE.match(rest)
        if not state:
            problems.append(f"state outside {', '.join(STATES)}: {short}")
            continue
        state = state.group(1) or state.group(2)
        severity = SEVERITY.search(label)
        severity = (severity.group(1) or severity.group(2)) if severity else ""
        if severity and severity not in ("P0", "P1", "P2"):
            problems.append(f"severity {severity} is not P0, P1 or P2: {short}")
        if state == "Done" and not PULL_REQUEST.search(entry):
            problems.append(f"Done cites no pull request: {short}")
        if state == "Will not fix" and not RULED.search(entry):
            problems.append(f"Will not fix gives no ruling date (ruled YYYY-MM-DD): {short}")
        findings.append({"label": label, "state": state, "severity": severity, "text": rest})
    if not findings and not problems:
        problems.append("`## Finding status` lists no finding")
    if findings and not problems:
        built = all(f["state"] in TERMINAL for f in findings)
        want = "BUILT" if built else "PARTIAL"
        if status != want:
            problems.append(f"status is {status}, but its findings make it {want}")
    return number, crate, status, findings, problems


def read_reviews(root):
    reviews = []
    directory = os.path.join(root, ROADMAP)
    for name in sorted(os.listdir(directory)):
        if not name.endswith(".md"):
            continue
        with open(os.path.join(directory, name), encoding="utf-8") as handle:
            parsed = parse_block(handle.read())
        if parsed:
            reviews.append((name,) + parsed)
    return reviews


def table(reviews):
    rows = [("milestone", "crate", "done", "open", "waiting", "at move", "at release",
             "will not fix", "open P0s")]
    for _, number, crate, _, findings, _ in reviews:
        count = {s: sum(1 for f in findings if f["state"] == s) for s in STATES}
        p0 = [short_label(f["label"]) for f in findings
              if f["severity"] == "P0" and f["state"] not in TERMINAL]
        rows.append((str(number), crate, str(count["Done"]), str(count["Open"]),
                     str(count["Waiting on"]), str(count["At the move"]),
                     str(count["At the release"]), str(count["Will not fix"]),
                     "; ".join(p0) or "-"))
    widths = [max(len(r[i]) for r in rows) for i in range(len(rows[0]) - 1)]
    return "\n".join("  ".join(c.ljust(w) for c, w in zip(r, widths)) + "  " + r[-1]
                     for r in rows)


def short_label(label):
    return label.split(",")[0].strip()


def open_list(reviews):
    lines = []
    for _, number, crate, _, findings, _ in reviews:
        for f in findings:
            if f["state"] == "Done":
                continue
            what = f["text"].split(". ")[0] if f["state"] == "Waiting on" else f["state"]
            lines.append(f"{number}  {crate}  {f['severity'] or '-'}  {f['label']}: {what}")
    return "\n".join(lines)


def check(reviews):
    problems = []
    for name, number, _, _, _, found in reviews:
        if number in NOT_YET_CONVERTED:
            continue
        problems += [f"{ROADMAP}/{name}: {p}" for p in found]
    return problems


def selftest():
    def block(status, body, title="`demo`: what an outside maintainer would fix before it leaves"):
        return f"---\nstatus: {status}\n---\n# 9001. {title}\n\n## Finding status\n\n{body}\n## Index row\n"

    good = ("One line per finding.\n\n"
            "- Item 1 (P0), a red check: Done, [#12](https://example.invalid/12).\n"
            "- Item 2 (P1), `a: b`: Open. Nobody has\n  fixed it.\n"
            "- Item 3 (P2): Waiting on a board run.\n"
            "- Item 4: At the move.\n"
            "- Item 5: At the release, the manifest.\n"
            "- Decision 1: Will not fix, ruled 2026-11-14 (UTC): the reason.\n"
            "- Item 6: Done in nifeos/other#3.\n"
            "- P1-7, an id: Open.\n")
    cases = [
        ("the closed set parses", block("PARTIAL", good), 0),
        ("all terminal and BUILT", block("BUILT", "- Item 1: Done, #1.\n- Item 2: At the move.\n"), 0),
        ("all terminal but PARTIAL", block("PARTIAL", "- Item 1: Done, #1.\n"), 1),
        ("open but BUILT", block("BUILT", "- Item 1: Open.\n"), 1),
        ("a state outside the set", block("PARTIAL", "- Item 1: Partly done, #1.\n"), 1),
        ("Done without a pull request", block("PARTIAL", "- Item 1: Done.\n- Item 2: Open.\n"), 1),
        ("Will not fix without a date", block("PARTIAL", "- Item 1: Will not fix.\n- Item 2: Open.\n"), 1),
        ("Waiting on nothing", block("PARTIAL", "- Item 1: Waiting on.\n"), 1),
        ("a bad severity", block("PARTIAL", "- Item 1 (P4): Open.\n"), 1),
        ("prose after a bullet", block("PARTIAL", "- Item 1: Open.\nA note.\n"), 1),
        ("no bullet at all", block("PARTIAL", "Nothing.\n"), 1),
        ("no label", block("PARTIAL", "- Open.\n"), 1),
    ]
    failed = 0
    for name, text, want in cases:
        parsed = parse_block(text)
        got = 1 if parsed is None or parsed[4] else 0
        if got != want:
            failed += 1
            print(f"selftest FAIL: {name}: wanted {want}, got {got}: {parsed and parsed[4]}",
                  file=sys.stderr)
    parsed = parse_block(block("PARTIAL", good))
    states = [f["state"] for f in parsed[3]]
    if states != ["Done", "Open", "Waiting on", "At the move", "At the release", "Will not fix",
                  "Done", "Open"]:
        failed += 1
        print(f"selftest FAIL: states read as {states}", file=sys.stderr)
    if [f["severity"] for f in parsed[3]][:3] + [parsed[3][-1]["severity"]] != ["P0", "P1", "P2",
                                                                                "P1"]:
        failed += 1
        print("selftest FAIL: severities misread", file=sys.stderr)
    if parsed[3][1]["label"] != "Item 2 (P1), `a: b`":
        failed += 1
        print(f"selftest FAIL: label read as {parsed[3][1]['label']}", file=sys.stderr)
    reviews = [("9001-demo.md",) + parsed]
    out = table(reviews)
    if "Item 1" in out or out.splitlines()[1].split() != ["9001", "demo", "2", "2", "1", "1", "1",
                                                          "1", "-"]:
        failed += 1
        print(f"selftest FAIL: table:\n{out}", file=sys.stderr)
    open_p0 = table([("9001-demo.md",) + parse_block(block("PARTIAL", "- Item 7 (P0), x: Open.\n"))])
    if not open_p0.rstrip().endswith("Item 7 (P0)"):
        failed += 1
        print(f"selftest FAIL: open P0 not listed:\n{open_p0}", file=sys.stderr)
    listed = open_list(reviews)
    if "Item 1" in listed or "Waiting on a board run" not in listed or listed.count("\n") != 5:
        failed += 1
        print(f"selftest FAIL: --open:\n{listed}", file=sys.stderr)
    superseded = block("SUPERSEDED", "- Item 1: Partly.\n")
    if parse_block(superseded) is not None:
        failed += 1
        print("selftest FAIL: a SUPERSEDED review was read", file=sys.stderr)
    other = block("PARTIAL", "- x\n", title="`demo`: a milestone that is no review")
    if parse_block(other) is not None:
        failed += 1
        print("selftest FAIL: a block that is no review was read", file=sys.stderr)
    if failed:
        return 1
    print(f"review-status selftest: {len(cases) + 8} cases pass")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--open", action="store_true", help="list every finding that is not Done")
    group.add_argument("--check", action="store_true", help="refuse a line outside the closed set")
    group.add_argument("--selftest", action="store_true", help="run over fixtures, no tree")
    args = parser.parse_args()
    if args.selftest:
        return selftest()
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    reviews = read_reviews(root)
    if args.check:
        problems = check(reviews)
        for problem in problems:
            print(f"review-status: {problem}", file=sys.stderr)
        if problems:
            print("review-status: the states are Done (with its pull request), Open, Waiting on "
                  "<what>, At the move, At the release and Will not fix (with ruled YYYY-MM-DD); "
                  "helpers/review_status.py says more", file=sys.stderr)
        return 1 if problems else 0
    converted = [r for r in reviews if r[1] not in NOT_YET_CONVERTED]
    print(open_list(converted) if args.open else table(converted))
    return 0


if __name__ == "__main__":
    sys.exit(main())
