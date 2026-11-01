"""Check every noise-shape survivor's site against its code, at the census's own commit.

Part 1 of the proposal design/roadmap/0954-a-survivor-accepted-once-is-not-accepted-forever.md,
alongside helpers/mutation_real_holes.py. The classes there come from the mutation text and the
triage row, and the proposal is explicit that shape is evidence, not proof: `replace | with ^` over
operands that share no bit is no test's kill, but text alone cannot say whether the operands share
a bit. This module reads each site's own line, at the census's own commit, and settles what that
line settles.

    python3 helpers/mutation_shape_sites.py check --census DATE --run ID --sha SHA \\
        [--out notes/project-metrics/mutation-shape-sites.csv] SHARD_DIR...
    python3 helpers/mutation_shape_sites.py stale          # script/lint: more than one census old
    python3 helpers/mutation_shape_sites.py --selftest     # fixtures only; script/lint

The input is the census's own shard output (missed.txt), the same files `script/mutation-census
--add` reads. Output: one row per noise-shape mutant (`replace | with ^`, `replace ^ with |`,
`replace << with >>`, `replace >> with <<`) with path, line, verdict and detail.

THE ARGUMENT. cargo-mutants swaps one operator token, so the question is whether that one swap
changes the line's value. Around the swap the line holds a chain of the same operator family
(`RCS | DPSLP | WOL`, `1 << 6`), because that is how register words and bit tests are written. The
chain, not the pair of adjacent operands, is the operand structure Rust parses, and the chain is what one
line plus the consts beside it can decide:

- an or-family chain (`|`, `^`) is swap-insensitive exactly when no two of its operands share a
  bit: every composition of `|` and `^` over a disjoint set is the same integer, so swapping one
  operator changes nothing;
- a shift-family chain (`<<`, `>>`) survives one swap only when the value feeding the swapped
  operator is zero or the shift amount right of it is zero: `x << 0`, `x >> 0`, `0 << n` and
  `0 >> n` are all `x` or `0`.

THE VERDICT says whether the line proves that argument:

- `literal` - every operand in the chain folds to an integer and the argument holds.
- `const` - the same, after folding operands that name a `const` defined in the same file
  (`pub const ALL: u64 = READ | WRITE;` folds its names, through any depth of other consts).
- `wide` - the chain folds, but the argument does not hold: `1 << 6` is not `1 >> 6`, and
  `0b1100 | 0b1000` shares a bit. This survivor is not noise by this argument, whatever its row
  says, and it is the check part 4's exclusion must answer.
- `site` - the line does not fold to integers: a field, a call, a name this file does not define,
  or an operand the word only ends (`a + MASK`, `x as u64`, a macro, a second line). Shape stays
  evidence; the row's reason and part 4's compiled assertion are what is left.

`check` writes the CSV and prints the per-crate summary. THE FILE IS NOT LINT GATED, deliberately:
its input is run artifacts, which are not in the tree, so `script/lint` cannot regenerate it from a
checkout. Its first line names the census, run and sha it read, the provenance form of the survivor
lists; each census's run regenerates it from its own artifacts.

BUGS

- One line of context is exactly one line. A line whose first or last token is the swapped
  operator is a continuation line: the operand on that side lives on the line above or below, so a
  chain on this line alone proves nothing, and the verdict is `site`. An operand written across
  lines, a macro invocation, or a const whose right-hand side ends on a later line reads as `site`
  the same way.
- A name that resolves through `use`, a `static`, an enum variant or another type's associated
  const reads as `site`. Only same-file `const NAME` definitions fold, and a name two same-file
  consts define refuses to fold rather than guessing which one.
- Parentheses are not parsed: `(A | B) | C` reads as `site` because the chain walk meets the
  bracket. The error is toward the verdict that sends a lane to read the code.
- A shift of a negative folded value reads `wide`: Python shifts a negative number in infinite
  two's complement, Rust wraps. The error is toward `wide`, which is the safe direction.
- The chain walk trusts the census's own column for which operator token was swapped.
"""

import argparse
import collections
import csv
import io
import pathlib
import re
import subprocess
import sys
import typing

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import mutation_inflow as mi  # noqa: E402

OUT = pathlib.Path("notes/project-metrics/mutation-shape-sites.csv")
# The mutations whose argument one line can decide; the site reads a chain of the swapped family.
NOISE_SHAPES = ("replace | with ^", "replace ^ with |", "replace << with >>", "replace >> with <<")
FAMILY = {"replace | with ^": ("|", "^"), "replace ^ with |": ("|", "^"),
          "replace << with >>": ("<<", ">>"), "replace >> with <<": ("<<", ">>")}
FIELDS = ["crate", "function", "mutation", "path", "line", "verdict", "detail"]
NAME = r"[A-Za-z_][A-Za-z0-9_]*"
CONST_DEF = re.compile(rf"(?:pub(?:\([a-z()]+\))?\s+)?const\s+({NAME})\s*:\s*[^=;]+=\s*([^;]+);")
# What may sit against a word: a path or field tail (`::`, `.`) means the word is not a whole name.
JOINED = ":."
# Tokens that end a chain cleanly, because they bind looser than any operator: `=` and the
# comparison and logic operators, and the punctuation of the surrounding syntax. `&` and `+` bind
# tighter and are parts of the operand, so they end nothing.
BOUND_L = frozenset("([{,;:=<>!?")
BOUND_R = frozenset(")]},;:<>?=")
Token = typing.NamedTuple("Token", [("kind", str), ("text", str), ("start", int), ("end", int)])


def tokenize(line):
    """Words, the four chain operators, and single other characters, with spans. `|=` keeps its `=`
    as a separate token so a compound assignment is never read as a chain operator."""
    out, i = [], 0
    while i < len(line):
        c = line[i]
        if c == " ":
            i += 1
        elif line[i:i + 2] in ("<<", ">>"):
            out.append(Token("op", line[i:i + 2], i, i + 2))
            i += 2
        elif c in "|^":
            out.append(Token("op", c, i, i + 1))
            i += 1
        elif c.isalnum() or c in "_'":
            j = i
            while j < len(line) and (line[j].isalnum() or line[j] in "_'"):
                j += 1
            out.append(Token("word", line[i:j], i, j))
            i = j
        else:
            out.append(Token("other", c, i, i + 1))
            i += 1
    return out


def fold(text, consts, seen=None):
    """The integer a const expression or operand folds to, or None.

    A small expression reader, not `eval`: the tree's own operators, integer literals of every
    form, parentheses, and names looked up in `consts`. Anything else (a path it cannot resolve, a
    call, a generic parameter) is None, and None means `site`, never a guess.
    """
    seen = frozenset() if seen is None else seen
    pos, tokens = 0, []
    while pos < len(text):
        m = re.match(r"\s*(?:(?P<shl><<)|(?P<shr>>>)|(?P<or>\|)|(?P<and>&)|(?P<xor>\^)|(?P<add>\+)|"
                     rf"(?P<sub>-)|(?P<open>\()|(?P<close>\))|(?P<num>0b[01_]+|0o[0-7_]+|"
                     rf"0x[0-9a-fA-F_]+|[0-9][0-9_]*)(?:[iu](?:8|16|32|64|size))?|(?P<name>{NAME}))",
                     text[pos:])
        if not m:
            if text[pos:].strip():
                return None
            break
        pos += m.end()
        kind = m.lastgroup
        if kind == "num":
            # Stripping the digit separators first keeps every match inside int()'s reach.
            tokens.append(int(m.group(kind).replace("_", ""), 0))
        elif kind == "name":
            name = m.group("name")
            if name in seen or name not in consts or consts[name] is None:
                return None
            value = fold(consts[name], consts, seen | {name})
            if value is None:
                return None
            tokens.append(value)
        else:
            tokens.append(kind)
    if not tokens:
        return None
    return _apply(tokens)


# Precedence, loosest to tightest: `|`, `^`, `&`, shifts, additive. Each level reads the levels
# below it, so one reader per level and the arithmetic itself at the bottom.
LEVELS = [("or", lambda a, b: a | b), ("xor", lambda a, b: a ^ b), ("and", lambda a, b: a & b),
          ("shl", lambda a, b: a << b), ("shr", lambda a, b: a >> b),
          ("add", lambda a, b: a + b), ("sub", lambda a, b: a - b)]


def _apply(tokens):
    """Fold a token list under LEVELS' precedence, or None when it is not a well-formed expression."""
    if len(tokens) == 1 and isinstance(tokens[0], int):
        return tokens[0]

    def parenthesized():
        if tokens[0] != "open" or tokens[-1] != "close":
            return None
        depth = 0
        for i, t in enumerate(tokens):
            if t == "open":
                depth += 1
            elif t == "close":
                depth -= 1
                if depth == 0 and i != len(tokens) - 1:
                    return None
        return _apply(tokens[1:-1]) if depth == 0 else None

    def level(i):
        op, fn = LEVELS[i]
        for j in range(1, len(tokens) - 1, 2):
            if tokens[j] == op:
                left, right = _apply(tokens[:j]), _apply(tokens[j + 1:])
                return None if left is None or right is None else fn(left, right)
        return None

    if tokens[0] == "sub" and len(tokens) == 2 and isinstance(tokens[1], int):
        return -tokens[1]
    got = parenthesized() if tokens else None
    if got is not None:
        return got
    for i in range(len(LEVELS)):
        got = level(i)
        if got is not None:
            return got
    return None


def consts_of(source):
    """name -> right-hand-side text, for the same-file consts. A name two consts define maps to
    None, which `fold` refuses: the checker does not guess between definitions."""
    out = {}
    for line in source.splitlines():
        m = CONST_DEF.match(line.strip())
        if m:
            out[m.group(1)] = None if m.group(1) in out else m.group(2)
    return out


def chain(line, toks, col, family):
    """The words of the swapped operator's chain, nearest-first per side, or None.

    Expanding outward from the swapped token, the line must alternate word, family operator, word;
    anything tighter (`a + MASK`, `& FLAG`, a path or field tail) ends the read, and only the
    looser tokens of BOUND_L and BOUND_R may close it. `|=` and friends are not chain operators:
    the `=` after them fails the read.
    """
    idx = next((i for i, t in enumerate(toks)
                if t.kind == "op" and t.text in family and t.start <= col - 1 < t.end), None)
    if idx is None:
        return None

    def compound(t):
        return t.end < len(line) and line[t.end] == "="

    def side(step):
        seq, i = [], idx
        while True:
            i += step
            if not 0 <= i < len(toks):
                return seq
            t = toks[i]
            if t.kind == "word":
                if (t.start > 0 and line[t.start - 1] in JOINED) \
                        or (t.end < len(line) and line[t.end] in JOINED):
                    return None
                seq.append(t)
            elif t.kind == "op" and t.text in family and not compound(t):
                seq.append(t)
            elif t.kind == "other" and t.text in (BOUND_L if step < 0 else BOUND_R):
                return seq
            else:
                return None

    left, right = side(-1), side(1)
    if left is None or right is None or not left and not right:
        return None
    merged = sorted((*left, toks[idx], *right), key=lambda t: t.start)
    # An edge token that is itself the operator means the expression walks off the line: the
    # swapped operator's operand on that side is a continuation of the line above or below, and a
    # chain that cannot see it proves nothing. A line that begins `| C` or ends `A |` is that case.
    if merged[0].kind == "op" or merged[-1].kind == "op":
        return None
    return merged, merged.index(toks[idx])


def verdict(source, path, line_no, col, mutation):
    """(verdict, detail) for one site: the census's own line of the file, the operator's column."""
    lines = source.splitlines()
    if not 1 <= line_no <= len(lines):
        return "site", f"line {line_no} is past the end of {path}"
    line = lines[line_no - 1]
    family = FAMILY[mutation]
    got = chain(line, tokenize(line), col, family)
    if got is None:
        return "site", line.strip()[:120]
    tokens, at = got
    consts = consts_of(source)
    values = [fold(t.text, consts) for t in tokens if t.kind == "word"]
    if any(v is None for v in values):
        return "site", line.strip()[:120]
    named = any(t.kind == "word" and t.text in consts for t in tokens)
    kind = "const" if named else "literal"
    shown = " ".join(f"{v} {t.text}" for v, t in zip(values, [t for t in tokens if t.kind == "op"])) \
        + f" {values[-1]}"
    if family == ("|", "^"):
        for i, a in enumerate(values):
            shared = next((b for b in values[i + 1:] if a & b), None)
            if shared is not None:
                return "wide", f"{shown}: {a:#x} and {shared:#x} share a bit"
        return kind, f"{shown} are pairwise disjoint"
    # A shift chain: the swap moves one operator from `x << y` to `x >> y` or back, which is the
    # identity exactly when the value feeding it or the amount right of it is zero. The chain
    # alternates value, operator, value, so the mutated operator at tokens[at] is the `at // 2`-th
    # operator of the sequence.
    op_texts = [t.text for t in tokens if t.kind == "op"]
    j = (at - 1) // 2
    feed = values[0]
    for i in range(j):
        feed = feed << values[i + 1] if op_texts[i] == "<<" else feed >> values[i + 1]
    amount = values[j + 1]
    if feed == 0 or amount == 0:
        return kind, f"a zero feed or amount: {feed} and {amount}"
    return "wide", f"{feed} shifted by {amount} changes value"


def git_show(sha, path, root="."):
    """The file at the census's commit, or None when that revision does not hold it."""
    r = subprocess.run(["git", "show", f"{sha}:{path}"], cwd=root, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def check(census, run, sha, dirs, out=None, root=".", reader=git_show):
    """Verdicts for every noise-shape mutant of the shard directories. Returns
    (per-crate Counter, the CSV text); `out` writes the CSV under `root`."""
    root = pathlib.Path(root)
    chain_renames = mi.load_renames(root / "notes/project-metrics/mutation-census-renames.csv")
    sites = {}
    for d in dirs:
        p = pathlib.Path(d) / "missed.txt"
        if not p.exists():
            continue
        for raw in p.read_text().splitlines():
            if not raw.strip():
                continue
            crate, function, mutation, path, line_no = mi.parse_mutant(raw)
            if mutation in NOISE_SHAPES:
                col = int(re.match(r"[^:]+:(\d+):(\d+):", raw).group(2))
                sites[(path, line_no, col)] = (mi.resolve(crate, chain_renames), function, mutation)
    files, rows = {}, []
    for (path, line_no, col), (crate, function, mutation) in sorted(sites.items()):
        if path not in files:
            files[path] = reader(sha, path, root)
        source = files[path]
        if source is None:
            got, detail = "site", f"{path} is not in this revision"
        else:
            got, detail = verdict(source, path, line_no, col, mutation)
        rows.append([crate, function, mutation, path, line_no, got, detail])
    buf = io.StringIO()
    buf.write(f"# census={census} run={run} sha={sha}\n")
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(FIELDS)
    w.writerows(rows)
    text = buf.getvalue()
    if out:
        target = root / out
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
    per_crate = collections.Counter()
    for crate, _f, _m, _p, _l, got, _d in rows:
        per_crate[crate] += 1
        per_crate[f"{crate}:{got}"] += 1
    return per_crate, text


def summary(per_crate):
    """The per-crate lines `check` prints: noise sites, then the count of each verdict."""
    out = []
    for crate in sorted({k.split(":")[0] for k in per_crate}):
        parts = " ".join(f"{v}={per_crate[f'{crate}:{v}']}"
                         for v in ("const", "literal", "wide", "site") if per_crate[f"{crate}:{v}"])
        out.append(f"  {crate:55s} {per_crate[crate]:4d}  {parts}")
    return out


def behind(shape_text, census_text):
    """How many recorded censuses are newer than the one the shape-sites text was generated from.
    The first line names it (`# census=DATE ...`); the census record's first column lists the rest.
    A file with no such line counts as behind every census."""
    m = re.match(r"# census=(\S+)", shape_text)
    mine = m.group(1) if m else ""
    dates = {r.split(",")[0] for r in census_text.splitlines()[1:] if r.strip()}
    return sum(1 for d in dates if d > mine)


def stale(root="."):
    """Lint: the committed file may trail the census record by the one record pull request that is
    still open, and no more. Returns an error line, or None."""
    root = pathlib.Path(root)
    shape = (root / OUT).read_text()
    n = behind(shape, (root / "notes/project-metrics/mutation-census.csv").read_text())
    if n > 1:
        return (f"{OUT} is {n} censuses behind notes/project-metrics/mutation-census.csv; "
                f"the record job regenerates it with each census (mutation.yml), so a census "
                f"record that did not carry it needs `python3 helpers/mutation_shape_sites.py "
                f"check` on that census's shards")
    return None


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    if argv[:1] == ["stale"]:
        err = stale()
        if err:
            print(f"mutation_shape_sites: {err}", file=sys.stderr)
            return 1
        return 0
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check")
    c.add_argument("--census", required=True)
    c.add_argument("--run", required=True)
    c.add_argument("--sha", required=True)
    c.add_argument("--out")
    c.add_argument("shards", nargs="+")
    args = ap.parse_args(argv)
    per_crate, _text = check(args.census, args.run, args.sha, args.shards, args.out)
    print(f"mutation_shape_sites: census {args.census}, noise-shape sites per crate "
          f"(literal+const = the argument holds on the line; wide = it does not; "
          f"site = the line does not fold):")
    for line in summary(per_crate):
        print(line)
    return 0


def selftest():
    src = "\n".join([
        "pub const READ: u64 = 1 << 0;",
        "pub const WRITE: u64 = 1 << 1;",
        "pub const ALL: u64 = READ | WRITE;",
        "pub const MIX: u64 = READ | 0b1000;",
        "pub const PAIR: u64 = 0b1100 | 0b1000;",
        "pub const CHAIN: u64 = 0b1100 | 0b1000 | 0b0011;",
        "const NEG: i64 = -4;",
        "const SHIFTED: u64 = 1 << 6;",
        "const Z: u64 = 0b00;",
        "const TWICE: u64 = Z << 6;",
    ])
    consts = consts_of(src)
    assert consts["ALL"] == "READ | WRITE" and consts["NEG"] == "-4", consts
    assert fold("READ", consts) == 1 and fold("ALL", consts) == 3 and fold("MIX", consts) == 9, consts
    assert fold("PAIR", consts) == 0b1100 and fold("CHAIN", consts) == 0b1111, consts
    assert fold("NEG", consts) == -4 and fold("TWICE", consts) == 0
    assert fold("(READ | WRITE) << 2", consts) == 12
    assert fold("self.0 | 1", consts) is None and fold("SIX", consts) is None
    assert fold("", consts) is None
    # a name two consts define refuses to fold
    twice = consts_of(src + "\nconst READ: u64 = 5;\n")
    assert twice["READ"] is None and fold("READ", twice) is None

    v = lambda n, c, m: verdict(src, "p", n, c, m)  # noqa: E731
    assert v(3, 27, "replace | with ^") == ("const", "1 | 2 are pairwise disjoint")
    assert v(4, 27, "replace | with ^") == ("const", "1 | 8 are pairwise disjoint")
    assert v(5, 30, "replace | with ^") == ("wide", "12 | 8: 0xc and 0x8 share a bit")
    # a chain reads whole: three operands joined by the family, one swap anywhere in it
    assert v(6, 31, "replace | with ^") == ("wide", "12 | 8 | 3: 0xc and 0x8 share a bit")
    assert v(6, 40, "replace | with ^") == ("wide", "12 | 8 | 3: 0xc and 0x8 share a bit")
    assert v(1, 25, "replace << with >>")[0] == "literal"   # 1 << 0: a zero amount
    assert v(8, 24, "replace << with >>") == ("wide", "1 shifted by 6 changes value")
    assert v(10, 22, "replace << with >>") == ("const", "a zero feed or amount: 0 and 6")
    # no operator at the column, past the end, a field call, a path tail
    assert v(3, 3, "replace | with ^")[0] == "site"
    assert v(99, 3, "replace | with ^")[0] == "site"
    assert verdict("fn f(x: u64) -> u64 { x.bits() | 1 }", "p", 1, 32, "replace | with ^")[0] == "site"
    assert verdict("fn f(x: u64) -> u64 { a::B | 1 }", "p", 1, 30, "replace | with ^")[0] == "site"
    # an operand the word only ends: the chain walk refuses, never folds a part
    assert verdict("fn g() -> u64 { x as MASK | 1 }", "p", 1, 27, "replace | with ^")[0] == "site"
    assert verdict("fn h(x: u64) -> u64 { MASK + 1 | x }", "p", 1, 29, "replace | with ^")[0] == "site"
    assert verdict("fn i(x: u64) -> u64 { -MASK | x }", "p", 1, 29, "replace | with ^")[0] == "site"
    assert verdict("fn f(x: u64) -> u64 { a::B | 1 }", "p", 1, 30, "replace | with ^")[0] == "site"
    # a name no const in the file defines is a site, and a compound assignment is not a chain
    assert verdict("fn k(x: u64) -> u64 { x | MASK; }", "p", 1, 26, "replace | with ^")[0] == "site"
    assert verdict("fn l(x: u64) { x |= 0b1; }", "p", 1, 15, "replace | with ^")[0] == "site"
    # parentheses are not parsed (BUGS)
    assert verdict("fn m(x: u64) -> u64 { (READ | WRITE) | x }", "p", 1, 35,
                   "replace | with ^")[0] == "site"
    # a brace closes the chain; the folded literals decide
    assert verdict("fn j() -> u64 { 0x_FFF0 | 1 }", "p", 1, 25,
                   "replace | with ^") == ("literal", "65520 | 1 are pairwise disjoint")

    # a continuation line: the operator opens or closes the line, so an operand lives on the line
    # above or below, and no one-line chain can see it. `READ` folds, which is the trap.
    cont = "pub const READ: u64 = 1;\nconst OR: u64 = READ\n    | READ;\n"
    assert verdict(cont, "p", 3, 5, "replace | with ^") == ("site", "| READ;")
    cont2 = "pub const READ: u64 = 1;\nconst OR: u64 = READ |\n    READ;\n"
    assert verdict(cont2, "p", 2, 25, "replace | with ^") == ("site", "const OR: u64 = READ |")
    # end to end over a fixture shard: the verdicts, and the CSV's provenance line
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)
        shard = root / "shard"
        shard.mkdir()
        (shard / "missed.txt").write_text(
            "crates/c/src/lib.rs:3:20: replace | with ^\n"
            "crates/c/src/lib.rs:4:18: replace << with >>\n"
            "crates/c/src/other.rs:1:30: replace < with <= in f\n")
        (root / "crates/c/src").mkdir(parents=True)
        (root / "crates/c/src/lib.rs").write_text(
            "const A: u64 = 0b01;\nconst B: u64 = 0b10;\n"
            "const ALL: u64 = A | B;\nconst Z: u64 = 1 << 0;\n")
        read = lambda sha, path, r: (root / path).read_text() if (root / path).exists() else None
        per_crate, text = check("2026-10-26", "1", "deadbeef", [shard], root=root, reader=read)
        assert text.startswith("# census=2026-10-26 run=1 sha=deadbeef\n"), text
        rows = list(csv.DictReader(text.splitlines()[1:]))
        assert [r["verdict"] for r in rows] == ["const", "literal"], rows
        assert per_crate["c"] == 2 and per_crate["c:literal"] == 1 and per_crate["c:const"] == 1
        assert summary(per_crate) == ["  c                                                          2  const=1 literal=1"], \
            summary(per_crate)
        # a file the revision does not hold is a `site`, not a crash
        (shard / "missed.txt").write_text("crates/c/src/gone.rs:1:27: replace | with ^\n")
        per_crate, text = check("2026-10-26", "1", "deadbeef", [shard], root=root, reader=read)
        assert list(csv.DictReader(text.splitlines()[1:]))[0]["verdict"] == "site"
    # staleness: one census behind is the open record pull request; two is a missed regeneration
    hdr = "census,run,crate\n"
    cen = hdr + "2026-10-12,1,a\n2026-10-19,2,a\n2026-10-26,3,a\n2026-10-26,3,b\n"
    assert behind("# census=2026-10-26 run=3 sha=x\n", cen) == 0
    assert behind("# census=2026-10-19 run=2 sha=x\n", cen) == 1
    assert behind("# census=2026-10-12 run=1 sha=x\n", cen) == 2
    assert behind("crate,function\n", cen) == 3
    print("mutation_shape_sites: selftest ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
