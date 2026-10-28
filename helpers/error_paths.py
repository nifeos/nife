#!/usr/bin/env python3
"""Count the error paths in the tree, and which of them no host test reaches.

    script/coverage                                    # or download a CI run's coverage-report
    python3 helpers/error_paths.py target/llvm-cov/html

Run from the repository root. Prints a per-crate table for the host crates in the llvm-cov report,
with each error path marked reached or not, then a count of the error paths in code no coverage run
reaches (the kernel, the services, the boot-time crates; a report from `script/coverage --kernel`
moves the kernel into the first table). Writes one JSON record per path to
target/error-paths/. Milestone 745 (count the error paths no test reaches), provisional number;
notes/untested-error-paths.md is what the numbers mean.

Name: provisional, minted by milestone 745's lane on 2026-10-04 (UTC). A helper under `helpers/`,
which `script/names` puts out of its own scope, so its provenance is this paragraph. calef names
things; expect this to change.

**What an error path is**, found by reading each line with strings and comments blanked:

- Result family: a `?` (in a function not returning Option), an `Err(..)` built or returned, an
  `.ok_or(..)` with no `?` after it, an `Err(..)` arm or `let Ok(..) else`, a refusal status code
  used as a value (`EINVAL`, `REP_ERR`, `Refusal::Refused`), and an arm on one.
- Option family, kept apart because an absent value is often not an error: a `?` in a function
  returning Option, `return None`, and `let Some(..) else`.
- A cleanup path is any of these whose body calls something that releases (free, delete, revoke,
  unmap, discard, destroy, reclaim, ...).

**What reached means.** llvm-cov's HTML marks each region nobody executed `region red`; a `?` gets
a region of its own for its early return, so a red `?` is an error side no test took. An arm or a
let-else is unreached when the first code of its body is red. An `.ok_or(e)` evaluates `e` either
way, so it is unmeasurable on a reached line and counted as unmapped. Test code (`cfg(test)`,
`cfg(kani)`, `cfg(loom)`, the system-test features) is left out, and so is a Display `fmt`.

The classifier's error rate, measured by hand, is in the note. It is a source-level reading and not a
parser: a path it misses is not counted at all.
"""
import collections
import html as htmlmod
import json
import os
import re
import sys

RELEASE = re.compile(
    r"\b[a-z_]*(free|dealloc|release|revoke|delete|destroy|unmap|undo|rollback|unwind|reclaim|"
    r"retire|unreserve|discard|reap|teardown|tear_down|give_back|return_frame|put_back|"
    r"drop|forget|clear_slot|uninstall|unlink|remove)[a-z_]*\s*[(!]"
)
# A status code that says no: an errno-style constant, or a protocol's refusal word. Matched as a
# whole identifier, optionally path-qualified, so `ECAM` and `ENTRIES` stay out.
REFUSAL = re.compile(
    r"(?<![A-Za-z0-9_])(?:[a-z_][a-z0-9_]*::)*(?:"
    r"E(?:BADF|BADOP|INVAL|ISDIR|NODATA|NOSPC|NOTDIR|NOTEMPTY|NOTSUP|PERM|RANGE|ROFS|NOENT|EXIST|"
    r"MFILE|LOOP|CAP|IO|NOMEM|NOSYS|AGAIN|BUSY)"
    r"|[A-Z_]*(?:REFUSED|DENIED|MALFORMED|REJECTED|REP_ERR|REP_FAILED|NET_ERROR|BAD_REQUEST|"
    r"NOT_PERMITTED|IN_USE|UNKNOWN_OPERATION)[A-Z_]*"
    r"|[A-Z][A-Za-z]*::(?:Refused|Denied|NotPermitted|Rejected)"
    r")(?![A-Za-z0-9_])"
)


DESCRIBES = re.compile(r"^(eq|ne|fmt|explain|describe|as_str|message|sentence|label|to_str|name|text|"
                       r"why|reason|meaning|hash|cmp|partial_cmp|from_code|code|status_name)$")
ARMS = ("err_arm", "status_arm", "option_else")
OPTION_KINDS = ("question_option", "option_else", "none_return")


def strip_code(line, state):
    """Blank out string literals and comments, keeping column positions. `state` carries an open
    block comment or string across lines."""
    out = []
    i = 0
    n = len(line)
    while i < n:
        c = line[i]
        if state["block"]:
            if line.startswith("*/", i):
                state["block"] -= 1
                out.append("  ")
                i += 2
                continue
            if line.startswith("/*", i):
                state["block"] += 1
                out.append("  ")
                i += 2
                continue
            out.append(" ")
            i += 1
            continue
        if state["str"]:
            if state["str"] == "raw":
                end = '"' + "#" * state["hashes"]
                if line.startswith(end, i):
                    state["str"] = None
                    out.append(" " * len(end))
                    i += len(end)
                    continue
                out.append(" ")
                i += 1
                continue
            if c == "\\":
                out.append("  ")
                i += 2
                continue
            if c == '"':
                state["str"] = None
            out.append(" ")
            i += 1
            continue
        if line.startswith("//", i):
            out.append(" " * (n - i))
            break
        if line.startswith("/*", i):
            state["block"] = 1
            out.append("  ")
            i += 2
            continue
        m = re.match(r'b?r(#*)"', line[i:])
        if m and (i == 0 or not (line[i - 1].isalnum() or line[i - 1] == "_")):
            state["str"] = "raw"
            state["hashes"] = len(m.group(1))
            out.append(" " * len(m.group(0)))
            i += len(m.group(0))
            continue
        if c == '"':
            state["str"] = "plain"
            out.append(" ")
            i += 1
            continue
        if c == "'":
            # char literal ('x', '\n', '\u{..}') vs lifetime ('a)
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'", line[i:])
            if m:
                out.append(" " * len(m.group(0)))
                i += len(m.group(0))
                continue
        out.append(c)
        i += 1
    return "".join(out)


def match_paren(s, open_idx):
    depth = 0
    for j in range(open_idx, len(s)):
        if s[j] == "(":
            depth += 1
        elif s[j] == ")":
            depth -= 1
            if depth == 0:
                return j
    return None


def candidates(code):
    """Yield (column, kind) for each error path visible on one comment-and-string-stripped line."""
    for m in re.finditer(r"\?", code):
        i = m.start()
        prev = code[i - 1] if i else " "
        if prev.isalnum() or prev in ")]}_>":
            if code[i + 1 : i + 6] == "Sized":
                continue
            yield i, "question"
    for m in re.finditer(r"(?<![A-Za-z0-9_:])Err\s*\(", code):
        i = m.start()
        before = code[:i].rstrip()
        close = match_paren(code, code.index("(", i))
        after = code[close + 1 :].lstrip() if close is not None else ""
        if re.search(r"\b(let|matches!\s*\([^,]*,)\s*$", before) or before.endswith("matches!("):
            if re.search(r"\bmatches!", before):
                continue  # a test of the value, not a path
            yield i, "err_arm"
        elif (after.startswith("=>") or after.startswith("|") or re.match(r"if\b", after)
              or re.match(r"[^=;{}(]*\)\s*(=>|if\b)", after)):
            yield i, "err_arm"
        elif before.endswith("|") and "=>" in after:
            yield i, "err_arm"
        elif re.search(r"\b(assert|assert_eq|assert_ne|debug_assert)!", before):
            continue
        else:
            yield i, "err_return"
    if not re.search(r"\b(const|static)\s+[A-Z_]+\s*:", code) and not re.search(
        r"(assert|assert_eq|assert_ne|matches)!", code
    ):
        for m in REFUSAL.finditer(code):
            i = m.start()
            if "Err(" in code[:i]:
                continue  # already counted as the Err it is wrapped in
            if "?" in code[m.end() :]:
                continue  # the error a `?` later on this line returns: that `?` is the path
            after = code[m.end() :].lstrip()
            before = code[:i].rstrip()
            if after.startswith("=>") or after.startswith("|") or before.endswith(("==", "!=", "|")):
                yield i, "status_arm"
            else:
                yield i, "refusal"
    for m in re.finditer(r"\blet\s+Some\s*\(", code):
        if not re.search(r"\b(if|while)\s+$", code[: m.start()]):
            yield m.start(), "option_else"  # let Some(x) = .. else { failure path }
    for m in re.finditer(r"\breturn\s+None\b", code):
        yield m.start(), "none_return"
    for m in re.finditer(r"\.ok_or(_else)?\s*\(", code):
        if "?" not in code[m.end():]:
            yield m.start(), "err_value"  # builds the Err a caller gets, with no `?` to see it
    for m in re.finditer(r"\blet\s+Ok\s*\(", code):
        if not re.search(r"\b(if|while)\s+$", code[: m.start()]):
            yield m.start(), "err_arm"  # let Ok(x) = .. else { error path }
    for m in re.finditer(r"\.is_err\(\)", code):
        if re.search(r"\b(if|while)\b", code[: m.start()]) and not re.search(
            r"(assert|matches)!", code[: m.start()]
        ):
            yield m.start(), "err_arm"


class FileModel:
    """A file's lines with, per character, whether llvm-cov saw it unreached. `reached` is None
    for an unmeasured file."""

    def __init__(self, path, lines, line_state=None, red=None):
        self.path = path
        self.lines = lines
        self.line_state = line_state  # list of 'covered'|'uncovered'|'skipped'
        self.red = red  # list of sets of columns inside a red region


ROW = re.compile(
    r"<tr><td class='line-number'><a name='L(\d+)'[^>]*><pre>\d+</pre></a></td>"
    r"<td class='(covered-line|uncovered-line|skipped-line)'>(?:<pre>[^<]*</pre>)?</td>"
    r"<td class='code'><pre>(.*?)</pre></td></tr>"
)
TAG = re.compile(r"<(/?)(span|div)([^>]*)>|&[a-z#0-9]+;|[^<&]+", re.S)


def parse_html(path):
    raw = open(path, encoding="utf-8").read()
    lines, states, reds = [], [], []
    for m in ROW.finditer(raw):
        state = m.group(2).split("-")[0]
        text = []
        red = set()
        stack = []  # each entry: 'red' | 'tip' | 'other'
        for t in TAG.finditer(m.group(3)):
            tok = t.group(0)
            if t.group(2):
                if t.group(1):
                    if stack:
                        stack.pop()
                else:
                    cls = t.group(3)
                    if "red" in cls:
                        stack.append("red")
                    elif "tooltip-content" in cls:
                        stack.append("tip")
                    else:
                        stack.append("other")
                continue
            if "tip" in stack:
                continue  # the execution count annotation, not source text
            s = htmlmod.unescape(tok) if tok.startswith("&") else tok
            for ch in s:
                if "red" in stack:
                    red.add(len(text))
                text.append(ch)
        lines.append("".join(text))
        states.append(state)
        reds.append(red)
    return lines, states, reds


def test_ranges(stripped):
    """Line indices inside #[cfg(test)] items (inline test modules)."""
    skip = set()
    i = 0
    n = len(stripped)
    while i < n:
        m = re.search(r"#\[cfg\((.*)\)\]", stripped[i])
        if m and "not(" not in m.group(1) and re.search(
            r"\b(test|kani|loom)\b|system_tests|soak_test|\bbench\b", m.group(1)
        ):
            depth = 0
            started = False
            j = i
            while j < n:
                skip.add(j)
                for ch in stripped[j]:
                    if ch == "{":
                        depth += 1
                        started = True
                    elif ch == "}":
                        depth -= 1
                if started and depth <= 0:
                    break
                if not started and j > i and stripped[j].rstrip().endswith(";"):
                    break
                j += 1
            i = j + 1
            continue
        i += 1
    return skip


def analyse(path, lines, states=None, reds=None):
    st = {"block": 0, "str": None, "hashes": 0}
    stripped = [strip_code(l, st) for l in lines]
    skip = test_ranges(stripped)
    # Enclosing function by brace depth.
    fn_at = [None] * len(lines)
    fn_start = [None] * len(lines)
    stack = []
    depth = 0
    pending = None
    for i, s in enumerate(stripped):
        m = re.search(r"\bfn\s+([A-Za-z0-9_]+)", s)
        if m:
            pending = [m.group(1), "", i]
        for pos, ch in enumerate(s):
            if pending is not None and not (pending[2] == i and pos < m.start()):
                pending[1] += ch
            if ch == "{":
                depth += 1
                if pending:
                    stack.append((depth, pending[0], return_kind(pending[1]), pending[2]))
                    pending = None
            elif ch == "}":
                if stack and stack[-1][0] == depth:
                    stack.pop()
                depth -= 1
            elif ch == ";" and pending:
                pending = None  # a declaration without a body
        if pending is not None:
            pending[1] += " "
        fn_at[i] = (stack[-1][1], stack[-1][2]) if stack else (None, None)
        fn_start[i] = stack[-1][3] if stack else None
        if m and stack and stack[-1][1] == m.group(1):
            fn_at[i] = (stack[-1][1], stack[-1][2])
    out = []
    for i, s in enumerate(stripped):
        if i in skip:
            continue
        found = sorted(candidates(s))
        arm_cols = [c for c, k in found if k in ("err_arm", "status_arm")]
        # `Err(e) => Err(e)` is one path, the arm; the value it rebuilds is not a second one.
        found = [(c, k) for c, k in found if not (k in ("err_return", "refusal") and any(a < c for a in arm_cols))]
        for col, kind in found:
            fn, ret = fn_at[i] if fn_at[i] else (None, None)
            if fn == "fmt":
                continue  # a Display impl's fmt::Error plumbing: not a path anyone handles
            if kind in ("refusal", "status_arm") and fn and DESCRIBES.match(fn):
                continue  # naming or comparing an error code, not taking a path
            if kind == "question" and ret == "option":
                kind = "question_option"
            rec = {"file": path, "line": i + 1, "col": col + 1, "kind": kind, "fn": fn,
                   "text": lines[i].strip()[:160]}
            if states is not None:
                rec["reached"] = reached(i, col, kind, s, stripped, states, reds)
            if kind in ARMS:
                body, rec["end"] = arm_body(stripped, i, col)
            elif kind in ("err_return", "refusal", "none_return") and s[:col].rstrip().endswith("=>"):
                body = s[col:]  # a match arm whose whole body is the error value
            elif kind in ("err_return", "refusal", "none_return"):
                body = block_before(stripped, i, col)
            else:
                body = ""
            rec["releases"] = bool(RELEASE.search(body))
            out.append(rec)
    # One path, one count: a `return Err(..)` inside an `Err(e) =>` arm or a let-else is that
    # arm's body, and so is an option let-else whose body returns an Err. Keep the outer arm when
    # it is an error arm; keep the inner Err when the outer is an Option let-else, since the Err
    # is the error the path produces.
    spans = [(r["line"], r["end"], r) for r in out if r["kind"] in ARMS and "end" in r]
    drop = set()
    for a_lo, a_hi, a in spans:
        inner = [r for r in out if r is not a and a_lo <= r["line"] <= a_hi
                 and r["kind"] in ("err_return", "refusal", "none_return")
                 and (r["line"], r["col"]) > (a["line"], a["col"])]
        if not inner:
            continue
        errs = [r for r in inner if r["kind"] != "none_return"]
        if a["kind"] == "option_else" and errs:
            drop.add(id(a))
            drop.update(id(r) for r in inner if r["kind"] == "none_return")
        else:
            drop.update(id(r) for r in inner)
    return [r for r in out if id(r) not in drop]


def return_kind(sig):
    """'option' when a signature's return type is Option. The parameter list is skipped by paren
    matching, and a where clause is cut, so a closure bound's `-> Option` is not mistaken for it."""
    sig = re.split(r"\bwhere\b", sig)[0]
    angle = 0
    k = 0
    while k < len(sig):
        if sig.startswith("->", k):
            k += 2
            continue
        if sig[k] == "<":
            angle += 1
        elif sig[k] == ">":
            angle -= 1
        elif sig[k] == "(" and angle == 0:
            close = match_paren(sig, k)
            if close is None:
                return "other"
            return "option" if re.match(r"\s*->\s*Option\b", sig[close + 1 :]) else "other"
        k += 1
    return "other"


def block_before(stripped, i, col):
    """The statements of the innermost block that ends in this error return, up to it: where a
    cleanup before `return Err(..)` would be written."""
    text = [stripped[i][:col]]
    depth = 0
    for ch in reversed(stripped[i][:col]):
        if ch == "}":
            depth += 1
        elif ch == "{":
            if depth == 0:
                return "".join(text)
            depth -= 1
    for k in range(i - 1, max(-1, i - 15), -1):
        seg = stripped[k]
        for j in range(len(seg) - 1, -1, -1):
            ch = seg[j]
            if ch == "}":
                depth += 1
            elif ch == "{":
                if depth == 0:
                    text.append(seg[j:])
                    return " ".join(reversed(text))
                depth -= 1
        text.append(seg)
    return " ".join(reversed(text))


def arm_body(stripped, i, col):
    """The text of the error arm or let-else block starting at (i, col)."""
    s = stripped[i]
    j = s.find("=>", col)
    if j < 0:
        j = s.find("else", col)
    if j < 0:
        j = s.find("{", col)
    if j < 0:
        return s[col:], i
    rest = s[j:]
    text = []
    depth = 0
    started = False
    for k in range(i, min(len(stripped), i + 40)):
        seg = rest if k == i else stripped[k]
        for ch in seg:
            text.append(ch)
            if ch == "{":
                depth += 1
                started = True
            elif ch == "}":
                depth -= 1
                if started and depth <= 0:
                    return "".join(text), k
            elif ch == "," and not started and depth == 0:
                return "".join(text), k
        text.append(" ")
        if not started and k > i + 1:
            break
    return "".join(text), k


def reached(i, col, kind, s, stripped, states, reds):
    """False when llvm-cov says the error side of this path never ran; True when it ran; None
    when the line carries no coverage mapping at all."""
    if states[i] == "uncovered":
        return False
    if kind in ("question", "question_option"):
        if col in reds[i]:
            return False
        return None if states[i] == "skipped" else True
    if kind == "err_value":
        # `.ok_or(e)` evaluates `e` either way, so only `.ok_or_else(|| ..)`'s closure can show
        # whether the None side ran; plain ok_or is unmeasurable on a reached line.
        if "ok_or_else" in s[col : col + 12]:
            close = match_paren(s, s.index("(", col))
            span = range(s.index("(", col) + 1, close if close else len(s))
            return not any(k in reds[i] for k in span if s[k].strip())
        return None
    if kind in ("err_return", "refusal", "none_return"):
        if col in reds[i]:
            return False
        if states[i] == "skipped":
            # a continuation line: inherit the nearest mapped line above
            k = i - 1
            while k >= 0 and states[k] == "skipped":
                k -= 1
            if k >= 0 and states[k] == "uncovered":
                return False
            return None if k < 0 else True
        return True
    # An arm: the pattern is not a region, the body is. Look at the first code after `=>`, or
    # after the `else` of a let-else, or the first mapped line below.
    j = s.find("=>", col)
    if j < 0:
        j = s.find("else", col)
    if j < 0:
        j = s.find("{", col)
    if j >= 0:
        for k in range(j + 2, len(s)):
            if s[k].strip() and s[k] not in "{":
                return not (k in reds[i])
    for k in range(i + 1, min(len(states), i + 6)):
        if not stripped[k].strip() or stripped[k].strip() in ("{",):
            continue
        if states[k] == "uncovered":
            return False
        first = len(stripped[k]) - len(stripped[k].lstrip())
        if first in reds[k]:
            return False
        if states[k] == "covered":
            return True
    return None


# What script/coverage leaves out because it cannot run on the host, plus the programs outside the
# workspace that ship. Test programs (system_tests, fixtures, the exercisers, socket_test_client, the
# soak) are not product code and are left out of both populations. Keep in step with script/coverage.
UNMEASURED = [
    "kernel/src", "components/src", "crates/user_mode_runtime", "crates/supervision_contract",
    "crates/hot_swap_contract", "crates/system_initializer", "crates/boot_ladder",
    "crates/capability_witness_contract", "redoxfs_server", "cryptography_provider", "entropy_backend",
]
NOT_PRODUCT = ("components/src/socket_test_client.rs", "kernel/src/soak.rs")
RESULT_KINDS = ("question", "err_return", "err_value", "err_arm", "refusal", "status_arm")


def measured(report):
    pages = []
    for dirpath, _, files in os.walk(report):
        pages += [os.path.join(dirpath, f) for f in files if f.endswith(".rs.html")]
    # The report nests each page under the absolute path it was built at (a runner's, or a
    # worktree's); the common prefix of them all is that build's repository root.
    root = os.path.commonpath([os.path.dirname(p) for p in pages])
    while not os.path.basename(root) in ("",) and any(
        os.path.relpath(p, root).split("/")[0] == "src" for p in pages
    ):
        root = os.path.dirname(root)
    for p in sorted(pages):
        src = os.path.relpath(p, root)[: -len(".html")]
        lines, states, reds = parse_html(p)
        yield from analyse(src, lines, states, reds)


def unmeasured():
    for d in UNMEASURED:
        for dirpath, dirs, files in os.walk(d):
            dirs[:] = sorted(x for x in dirs if x not in ("target", "tests", "benches", "examples"))
            for f in sorted(files):
                rel = os.path.join(dirpath, f)
                if f.endswith(".rs") and rel not in NOT_PRODUCT:
                    lines = open(rel, encoding="utf-8", errors="replace").read().split("\n")
                    yield from analyse(rel, lines)


def unit(path):
    p = path.split("/")
    if p[0] == "crates":
        return p[1]
    if p[0] == "kernel":
        return "kernel (arch)" if len(p) > 2 and p[2] == "arch" else "kernel"
    return p[0]


def table(rows, reached):
    d = collections.defaultdict(collections.Counter)
    for r in rows:
        fam = "R" if r["kind"] in RESULT_KINDS else "O"
        for key in (unit(r["file"]), "TOTAL"):
            d[key][fam] += 1
            d[key]["kind:" + r["kind"]] += 1
            if r.get("reached") is False:
                d[key][fam + "u"] += 1
                d[key]["unreached:" + r["kind"]] += 1
            elif "reached" in r and r["reached"] is None:
                d[key]["unmapped"] += 1
    order = sorted(d.items(), key=lambda kv: (kv[0] != "TOTAL", -kv[1]["Ru"], -kv[1]["R"], kv[0]))
    if reached:
        print("| crate | Result paths | not reached | Option paths | not reached | unmapped |")
        print("|---|---:|---:|---:|---:|---:|")
        for k, v in order:
            print(f"| {k} | {v['R']} | {v['Ru']} | {v['O']} | {v['Ou']} | {v['unmapped']} |")
    else:
        print("| code | Result paths | Option paths |")
        print("|---|---:|---:|")
        for k, v in order:
            print(f"| {k} | {v['R']} | {v['O']} |")
    print()
    print("| kind | paths" + (" | not reached |" if reached else " |"))
    print("|---|---:" + ("|---:|" if reached else "|"))
    t = d["TOTAL"]
    for k in RESULT_KINDS + OPTION_KINDS:
        print(f"| {k} | {t['kind:' + k]}" + (f" | {t['unreached:' + k]} |" if reached else " |"))
    print()


def main():
    if len(sys.argv) != 2 or sys.argv[1].startswith("-"):
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    out = "target/error-paths"
    os.makedirs(out, exist_ok=True)
    m = list(measured(sys.argv[1]))
    # A report from an instrumented kernel image (`script/coverage --kernel`, milestone 781 (measure kernel and service coverage under QEMU))
    # measures files this list also names. A file in the report is measured, and counts once.
    seen = {r["file"] for r in m}
    u = [r for r in unmeasured() if r["file"] not in seen]
    for name, rows in (("measured", m), ("unmeasured", u)):
        with open(os.path.join(out, name + ".jsonl"), "w") as f:
            for r in rows:
                f.write(json.dumps(r) + "\n")
    print("## Measured: the host crates in the coverage report\n")
    table(m, True)
    print("## Unmeasured: code no coverage run reaches\n")
    table(u, False)
    cleanup = [r for r in m + u if r["releases"]]
    print(f"cleanup paths (an error path whose body releases something): {len(cleanup)}, "
          f"of which unreached: {sum(1 for r in cleanup if r.get('reached') is False)}, "
          f"unmeasured: {sum(1 for r in cleanup if 'reached' not in r)}")
    print(f"per-path records: {out}/measured.jsonl, {out}/unmeasured.jsonl")


if __name__ == "__main__":
    main()
