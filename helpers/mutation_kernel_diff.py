"""The diff-scoped kernel check's glue: filter the diff's mutants to the corpus, route each file to
its architecture, split the shards so every job stays under 20 minutes, and read each shard's
verdicts into the triage ledger's vocabulary.

Part 3 of roadmap 955 (the kernel and its programs are mutated under QEMU): the advisory half of
option B, which calef ruled on 2026-10-27 (UTC). The workflow is
.github/workflows/kernel-mutants-diff.yml; it lists the pull request's mutants with
`cargo mutants --no-config --list --diff --in-diff`, hands the listing to `shards` here, and runs
each shard's patches with `script/falsifications --mutants --patches`, the driver part 1 built.
`verdict` reads that driver's TSV and writes the shard's step summary.

    python3 helpers/mutation_kernel_diff.py shards --listing LISTING [--listing ...] \\
        --toml .cargo/mutants.toml [--roots kernel,components] --shard-dir DIR [--summary FILE]
    python3 helpers/mutation_kernel_diff.py verdict --tsv TSV --arch A --index K --total N \\
        [--count C] [--triage CSV] [--renames CSV] [--strict] [--summary FILE]
    python3 helpers/mutation_kernel_diff.py --selftest      # fixtures, no git, no cargo; script/lint

`shards` prints one `matrix=<json>` line on stdout for the workflow's GITHUB_OUTPUT and its human
report on stderr; `verdict` prints its summary on stdout and appends it to `--summary`. Both exit
0 unless the input is broken (exit 2, a broken run is never a pass, the diff gate's rule). The
check runs advisory: a survivor never fails it. `verdict --strict` is the shape the check takes
if the block's four-green-weeks promotion makes it required, and it is the only path that exits 1.

THE CORPUS IS THE TOML'S OWN INSTRUCTION, and a config reading cannot do it. The kernel run lists
with `--no-config` because `.cargo/mutants.toml`'s `kernel/**` entry, which keeps the kernel out
of the host census, would make a config-reading run list nothing; the toml's kernel section says
so and is addressed to this run: take the `exclude_globs` entries that name a path under
`<root>/src/` (the section's own entries: benches, boot self-tests, harness files) plus the
`**/`-prefixed proof-file entries, and treat a matched file as outside the corpus. The
crate-whole entries and other crates' entries do not participate. `exclude_re` applies whole: its
patterns match module paths (`proofs::`, `tests::`), which no corpus wants.

THE SHARD MATH is the block's, with a stated reserve. A shard's job pays its baseline first (both
image builds included: 380 s on aarch64, 785 s on x86_64, part 1's measurement), so the mutant
phase gets JOB_SECONDS minus the baseline minus SETUP_RESERVE_SECONDS for everything a job does
around the driver (checkout, toolchain, cargo-mutants, the QEMU cache restore, bootstrap). A
larger diff takes more shards, never a longer job: each shard holds at most SHARD_MUTANTS of its
architecture, and the driver's own `--budget` stops it cleanly if mutants run slower than
measured. riscv64 has no measurement and borrows x86_64's numbers (both boots run under TCG on
the arm pool); the plan summary says so.

THE ROUTE. An `arch/` file runs on its own architecture, or its mutant survives for want of a leg
(the driver's own BUGS list says why). Everything else runs on aarch64, which part 1 measured 24%
cheaper per mutant.

THE KEY AND THE LEDGER are the rule of milestone 85 (mutation testing over the host crates) and
the key of milestone 924 (a survivor on a pull request's own lines cannot merge untriaged),
imported rather than restated: a survivor
keys on (crate, function, mutation) through the rename chain, and only an `equivalent` or `gap`
row clears one, because this check, like the gate, has just watched the mutant live.

Name: provisional, minted 2026-10-28 for milestone 955 (mutation testing reaches the kernel and
its programs under QEMU, a diff and a slice at a time), part 3; an architect names things.
"""

import argparse
import collections
import fnmatch
import json
import pathlib
import re
import sys
import tomllib

from mutation_inflow import load_renames, parse_mutant, read_triage
from mutation_diff_gate import decide

# The block's bound: sharded so each job stays under 20 minutes. Everything below derives from it
# and from part 1's measurement (.github/workflows/kernel-mutants-cost.yml, run 40587528953,
# 2026-10-27 (UTC), the block's Cost section).
JOB_SECONDS = 20 * 60
# Checkout, toolchain, cargo-mutants, the QEMU cache restore and bootstrap: the work a shard job
# does around the driver. Part 1's cost job finished its 8-mutant run inside a 25-minute bound
# with about this much setup, and rust-cache sharing the mutation gate's pool only shortens it.
SETUP_RESERVE_SECONDS = 180
# riscv64 is estimated: no measurement exists, so it borrows x86_64's, the other TCG-on-arm leg.
BASELINE_SECONDS = {"aarch64": 380, "riscv64": 785, "x86_64": 785}
PER_MUTANT_SECONDS = {"aarch64": 81, "riscv64": 106, "x86_64": 106}
BUDGET_SECONDS = {a: JOB_SECONDS - BASELINE_SECONDS[a] - SETUP_RESERVE_SECONDS
                  for a in BASELINE_SECONDS}
SHARD_MUTANTS = {a: BUDGET_SECONDS[a] // PER_MUTANT_SECONDS[a] for a in BASELINE_SECONDS}
# Milestone 741's first scaled run queued 40 jobs on the shared runners and was cancelled; the
# census that shares this pool budgets 10 runner-hours a night. A diff whose mutants need more
# than MAX_SHARDS shards is skipped with that stated, never squeezed into longer jobs.
MAX_SHARDS = 10
ARCH_ROUTE = ("aarch64", "riscv64", "x86_64")
TRIAGE_PATH = "notes/project-metrics/mutation-triage.csv"
RENAMES_PATH = "notes/project-metrics/mutation-census-renames.csv"
HEADER = re.compile(r"^(\S+\.rs):(\d+):(\d+): (.*)$")


def route(path):
    """The architecture a mutant of `path` boots on: an `arch/` file's own, aarch64 otherwise."""
    m = re.match(r"^(?:kernel|components)/src/arch/([^/]+)/", path)
    if m and m.group(1) in ARCH_ROUTE:
        return m.group(1)
    return "aarch64"


def corpus_globs(toml_path, roots):
    """The `exclude_globs` entries a kernel-and-components run consumes, per the toml's own
    instruction: the entries naming a path under a root's `src/` (the kernel section) and the
    `**/`-prefixed proof-file entries. The crate-whole entries (`kernel/**`, `xtask/**`), which
    keep these packages out of the host census, do not participate: applied whole they would end
    the run before it began."""
    entries = tomllib.loads(pathlib.Path(toml_path).read_text()).get("exclude_globs", [])
    return [g for g in entries
            if g.startswith("**/") or any(g.startswith(r + "/src/") for r in roots)]


def corpus_regexes(toml_path):
    """`exclude_re` applies whole: its patterns match module paths, which no corpus wants."""
    return tomllib.loads(pathlib.Path(toml_path).read_text()).get("exclude_re", [])


class Block:
    """One listed mutant: its header fields and its raw text, blank line included, so a shard file
    re-serializes to exactly what the driver's patch parser reads."""

    def __init__(self, file, line, name, raw):
        self.file, self.line, self.name, self.raw = file, line, name, raw


def parse_blocks(text):
    """Split `cargo mutants --list --diff` output into blocks, the split the driver's
    `mutant_patches` does, keeping each block's raw lines. A diff body line starts with a space,
    `+`, `-` or `@@`, so a truly empty line ends a block."""
    out, cur = [], None
    for line in text.splitlines():
        if cur is None:
            m = HEADER.match(line)
            if m:
                cur = {"file": m.group(1), "line": int(m.group(2)), "name": m.group(4),
                       "lines": [line]}
            continue
        if line == "":
            cur["lines"].append("")
            out.append(Block(cur["file"], cur["line"], cur["name"], "\n".join(cur["lines"]) + "\n"))
            cur = None
        else:
            cur["lines"].append(line)
    if cur:
        out.append(Block(cur["file"], cur["line"], cur["name"], "\n".join(cur["lines"]) + "\n"))
    return out


def under_roots(path, roots):
    return any(path == r or path.startswith(r + "/") for r in roots)


def excluded(block, globs, regexes):
    """True when the file is outside the corpus: a glob from the toml's own instruction matches the
    path, or an `exclude_re` pattern matches the mutant's module path."""
    if any(fnmatch.fnmatch(block.file, g) for g in globs):
        return True
    return any(re.search(r, block.name) for r in regexes)


def plan(listing_paths, toml_path, roots):
    """(kept blocks by arch, dropped count, listed count). The order inside an architecture is the
    listing's, so a shard's slice is stable across a rerun on the same tree."""
    globs, regexes = corpus_globs(toml_path, roots), corpus_regexes(toml_path)
    blocks, dropped = [], 0
    for p in listing_paths:
        blocks.extend(parse_blocks(pathlib.Path(p).read_text()))
    listed = len(blocks)
    kept = collections.OrderedDict()
    for b in blocks:
        if not under_roots(b.file, roots) or excluded(b, globs, regexes):
            dropped += 1
            continue
        kept.setdefault(route(b.file), []).append(b)
    return kept, dropped, listed


def split(kept):
    """Contiguous shards, at most SHARD_MUTANTS of one architecture each, listing order kept."""
    shards = []
    for arch, blocks in kept.items():
        n = SHARD_MUTANTS[arch]
        for i in range(0, len(blocks), n):
            shards.append((arch, blocks[i:i + n]))
    return shards


def matrix_entries(shards):
    return [{"index": k + 1, "total": len(shards), "arch": arch, "file": "shard-%02d.txt" % (k + 1),
             "count": len(blocks), "limit": SHARD_MUTANTS[arch], "budget": BUDGET_SECONDS[arch],
             "noop": False}
            for k, (arch, blocks) in enumerate(shards)]


def cmd_shards(args):
    roots = [r for r in args.roots.split(",") if r]
    try:
        kept, dropped, listed = plan(args.listing, args.toml, roots)
    except (OSError, ValueError) as e:
        print(f"mutation_kernel_diff: {e}", file=sys.stderr)
        return 2
    shards = split(kept)
    total = sum(len(b) for b in kept.values())
    counts = {a: len(b) for a, b in kept.items()}
    out = ["### Kernel mutants (advisory): the plan", ""]
    if total == 0:
        outside = " and ".join(f"`{r}`" for r in roots)
        out += [
            f"The diff's listing holds {listed} mutant(s) and the corpus keeps none of them: the"
            f" changed lines sit outside {outside}, or in files `.cargo/mutants.toml`'s kernel"
            " section excludes (benches, boot self-tests, harness files).",
            "",
            "Nothing is sharded and nothing boots. This pass is not a mutation result.",
        ]
    elif len(shards) > MAX_SHARDS:
        out += [
            f"The corpus keeps {total} mutant(s)"
            f" ({', '.join(f'{a} {n}' for a, n in counts.items())}), which needs {len(shards)}"
            f" shards, over this run's cap of {MAX_SHARDS}. A lane once queued 40 mutant jobs on"
            " the shared runners and the run was cancelled (milestone 741 (does a standing proof"
            " notice a regression)), so a larger diff is"
            " skipped, never squeezed into longer jobs.",
            "",
            "Nothing is sharded and nothing boots. The rolling census (roadmap 955, part 4)"
            " mutates a slice of every file on its rotation; split the pull request to bring the"
            " diff under the cap.",
        ]
    else:
        d = pathlib.Path(args.shard_dir)
        d.mkdir(parents=True, exist_ok=True)
        for entry, (_arch, blocks) in zip(matrix_entries(shards), shards):
            (d / entry["file"]).write_text("".join(b.raw for b in blocks))
        routing = ", ".join(f"{a} {counts[a]}" for a in ARCH_ROUTE if counts.get(a))
        outside = " and ".join(f"`{r}`" for r in roots)
        out += [
            f"The diff's listing holds {listed} mutant(s); the corpus keeps {total} ({dropped}"
            f" dropped: outside {outside}, or files the toml's kernel section excludes: benches,"
            " boot self-tests, harness files).",
            "",
            f"Routing: {routing}. An `arch/` file boots on its own architecture; everything else"
            " boots on aarch64, the cheaper leg in part 1's measurement.",
            "",
            f"{len(shards)} shard(s), at most "
            + ", ".join(f"{SHARD_MUTANTS[a]} mutants on {a}" for a in ARCH_ROUTE)
            + ". A shard's job pays its baseline first (380 s on aarch64, 785 s on x86_64 and,"
            " estimated, riscv64: both image builds included) plus a 180 s setup reserve, inside"
            " the job's 20-minute bound, at 81 s a mutant on aarch64 and 106 s on x86_64 and"
            " riscv64 (part 1's measurement). A larger diff takes more shards, never a longer"
            " job.",
        ]
    text = "\n".join(out) + "\n"
    print(text, file=sys.stderr, end="" if text.endswith("\n") else "\n")
    if args.summary:
        with open(args.summary, "a") as fh:
            fh.write(text)
    allowed = shards if len(shards) <= MAX_SHARDS else []
    # A matrix that expands to zero combinations turns the whole run red (observed, run
    # 40640527340: both jobs green, the run failed, no check-run failed), so a run with nothing to
    # shard sends one sentinel entry instead, and the shard job turns it into an honest green
    # "nothing to mutate" on the cheap runner.
    if allowed:
        entries = matrix_entries(allowed)
    else:
        entries = [{"index": 0, "total": 0, "arch": "none", "file": "", "count": 0, "limit": 0,
                    "budget": 0, "noop": True}]
    print("matrix=" + json.dumps({"include": entries}))
    return 0


VERDICT_KIND = {"survived": "missed"}
VERDICT_KIND.update({f"timeout-{t}": "timeout" for t in ("kernel", "system")})


def read_tsv(path):
    """The driver's `--out` table as (mutant, verdict) pairs. A missing file is a broken run."""
    p = pathlib.Path(path)
    if not p.exists():
        raise FileNotFoundError(f"{path}: no verdict table; the driver did not finish, and a"
                                " broken run is never a pass")
    rows = []
    for i, raw in enumerate(p.read_text().splitlines()[1:], start=2):
        if not raw:
            continue
        parts = raw.split("\t")
        if len(parts) != 4:
            raise ValueError(f"{path}:{i}: expected 4 tab-separated fields, got {len(parts)}")
        rows.append((parts[0], parts[1]))
    return rows


def cmd_verdict(args):
    try:
        rows = read_tsv(args.tsv)
        chain = load_renames(args.renames)
        triage = read_triage(args.triage, chain)
        survivors = []
        for name, verdict in rows:
            kind = VERDICT_KIND.get(verdict)
            if kind:
                survivors.append((parse_mutant(name), kind))
        cleared, untriaged, _keys = decide(survivors, triage, chain)
    except (OSError, ValueError) as e:
        print(f"mutation_kernel_diff: {e}", file=sys.stderr)
        return 2
    kinds = collections.Counter(v for _n, v in rows)
    ran = ", ".join(f"{v} {n}" for v, n in sorted(kinds.items())) or "no mutant ran"
    head = (f"### Kernel mutants (advisory), shard {args.index} of {args.total} on {args.arch}: "
            + (f"{len(untriaged)} survivor key(s) untriaged" if untriaged else "clean"))
    body = [
        "",
        f"{args.count if args.count is not None else len(rows)} mutant(s) in the shard,"
        f" {len(rows)} run ({ran}).",
    ]
    if args.count is not None and len(rows) < args.count:
        body += [
            "",
            f"The shard stopped {args.count - len(rows)} mutant(s) short of its plan: the budget"
            " fires when mutants run slower than part 1 measured. The shard's slice is stable, so"
            " a rerun of this workflow finishes what is missing.",
        ]
    if untriaged:
        body += [
            "",
            f"{cleared} key(s) cleared by the ledger; {len(untriaged)} did not clear, and under"
            f" milestone 85's rule each needs a row in {TRIAGE_PATH} (`equivalent` or `gap`) or a"
            " test that kills it. A timeout is untriaged the same as a survivor (milestone 924's"
            " rule, and this run watched the mutant live, so a `killed` row never clears one).",
            "",
            "This check is advisory (roadmap 955, part 3): it does not block the merge, and the"
            " rolling census revisits every file.",
            "",
            "| crate | function | mutation | kinds | what is missing |",
            "|---|---|---|---|---|",
        ]
        for (crate, function, mutation), kinds_seen, why in untriaged:
            missing = {
                "none": f"a row in {TRIAGE_PATH} (`equivalent` or `gap`), or a test that kills it",
                "killed": "this run saw the mutant live, so its `killed` row is false here: kill"
                          " it with a test, or write the `equivalent` or `gap` row",
                "stale": "its row is unchanged, and this diff changed the function it describes:"
                         " read the row against the new code and update it, or kill the mutant"
                         " with a test",
            }[why]
            body.append(f"| {crate} | `{function}` | `{mutation}` |"
                        f" {', '.join(sorted(kinds_seen))} | {missing} |")
        unviable = sum(1 for _n, v in rows if v == "unviable")
        stale = sum(1 for _n, v in rows if v == "stale")
        if unviable or stale:
            body += ["", f"Also {unviable} unviable (the mutant does not compile) and {stale}"
                         " stale (the patch no longer applied), which are verdicts on the diff's"
                         " context, not on the tests."]
    else:
        body += [
            "",
            "No survivor and no timeout, so there is nothing to triage. This pass is a mutation"
            " result: the mutants ran under QEMU in two tiers, the kernel suite first, the system"
            " image only for what survived it (roadmap 955, part 3).",
        ]
    text = "\n".join([head] + body) + "\n"
    print(text, end="" if text.endswith("\n") else "\n")
    if args.summary:
        with open(args.summary, "a") as fh:
            fh.write(text)
    if args.strict and untriaged:
        return 1
    return 0


def selftest():
    def listing(*blocks):
        return "\n".join("\n".join((f"{f}:{l}:5: {n}", f"--- {f}", f"+++ {n}", "@@ -1,2 +1,2 @@",
                                    "-a", "+b", "")) for f, l, n in blocks) + "\n"

    import tempfile
    with tempfile.TemporaryDirectory(prefix="nife955-selftest-") as tmp:
        root = pathlib.Path(tmp)
        toml = root / "mutants.toml"
        toml.write_text(
            'exclude_globs = [\n'
            '  "kernel/**",\n'          # the host census's crate-whole entry: must NOT end the run
            '  "kernel/src/bench/**",\n'
            '  "kernel/src/self_test.rs",\n'
            '  "**/src/**/proofs.rs",\n'
            ']\n'
            'exclude_re = ["tests::"]\n'
        )
        roots = ["kernel", "components"]

        # The corpus rule: the kernel section's entries exclude their files, the crate-whole entry
        # excludes nothing, the proof glob and the module-path regex do their work.
        raw = root / "listing.txt"
        raw.write_text(listing(
            ("kernel/src/revoke.rs", 9, "replace == with != in Revoke::revoke"),
            ("kernel/src/bench/meters.rs", 3, "replace + with - in Meters::add"),
            ("kernel/src/self_test.rs", 7, "replace < with <= in check"),
            ("kernel/src/cap/proofs.rs", 4, "replace | with ^ in subset"),
            ("kernel/src/glob.rs", 2, "replace == with != in tests::helper"),
            ("kernel/src/virtio.rs", 5, "replace * with / in Wire::step"),
        ))
        kept, dropped, listed = plan([str(raw)], str(toml), roots)
        assert [b.file for b in kept["aarch64"]] == ["kernel/src/revoke.rs", "kernel/src/virtio.rs"]
        assert dropped == 4 and listed == 6, (dropped, listed)

        # The route: an arch file boots on its own architecture, everything else on aarch64.
        assert route("kernel/src/arch/x86_64/discovery.rs") == "x86_64"
        assert route("kernel/src/arch/riscv64/trap.rs") == "riscv64"
        assert route("kernel/src/arch/aarch64/semihosting.rs") == "aarch64"
        assert route("kernel/src/sched/rendezvous.rs") == "aarch64"
        assert route("components/src/init.rs") == "aarch64"

        # The shard math holds the block's numbers: the budget is the job minus baseline minus
        # reserve, the count fits the budget at the measured cost, and one more does not.
        assert BUDGET_SECONDS["aarch64"] == 1200 - 380 - 180
        assert BUDGET_SECONDS["x86_64"] == 1200 - 785 - 180
        assert SHARD_MUTANTS["aarch64"] == 7 and SHARD_MUTANTS["x86_64"] == 2, SHARD_MUTANTS
        for a, n in SHARD_MUTANTS.items():
            assert n * PER_MUTANT_SECONDS[a] <= BUDGET_SECONDS[a] < (n + 1) * PER_MUTANT_SECONDS[a]

        # Nine aarch64 mutants take two shards of 7 and 2, and the matrix carries each shard's
        # budget and limit.
        nine = root / "nine.txt"
        nine.write_text(listing(*[("kernel/src/sched/thing.rs", i + 2,
                                   f"replace == with != in f{i}") for i in range(9)]))
        kept, _d, _l = plan([str(nine)], str(toml), roots)
        shards = split(kept)
        assert [len(b) for _a, b in shards] == [7, 2], [len(b) for _a, b in shards]
        entries = matrix_entries(shards)
        assert entries[0]["arch"] == "aarch64" and entries[1]["count"] == 2
        assert entries[0]["budget"] == BUDGET_SECONDS["aarch64"] and entries[0]["limit"] == 7

        tri = root / "triage.csv"
        tri.write_text(
            "crate,function,mutation,disposition,reason,kind,verified,home\n"
            "kernel,Revoke::revoke,replace == with !=,equivalent,both arms refuse,algebraic,2026-10-28,\n"
        )
        renames = str(root / "absent.csv")
        tsv = root / "shard.tsv"
        tsv.write_text(
            "mutant\tverdict\tkernel_s\tsystem_s\n"
            "kernel/src/revoke.rs:9:5: replace == with != in Revoke::revoke\tsurvived\t16\t126\n"
            "kernel/src/wire.rs:3:5: replace == with != in Wire::step\tcaught-kernel\t17\t\n"
            "kernel/src/wire.rs:8:5: replace < with <= in Wire::walk\ttimeout-system\t18\t420\n"
            "kernel/src/wire.rs:12:5: replace + with - in Wire::add\tunviable\t4\t\n"
        )

        def verdict(**over):
            args = argparse.Namespace(tsv=str(tsv), arch="aarch64", index=1, total=2, count=4,
                                      triage=str(tri), renames=renames, strict=False, summary=None)
            for k, val in over.items():
                setattr(args, k, val)
            return _capture(cmd_verdict, args)

        # Advisory: a survivor and a timeout are reported, the equivalent row clears its key, and
        # the exit is 0. The timeout renders in the same table, as 924's gate renders one.
        code, text = verdict()
        assert code == 0, text
        assert "shard 1 of 2 on aarch64: 1 survivor key(s) untriaged" in text, text
        assert "1 key(s) cleared by the ledger" in text, text
        assert "| kernel | `Revoke::revoke`" not in text, text  # the equivalent row cleared it
        assert "| kernel | `Wire::walk` | `replace < with <=` | timeout |" in text, text
        assert "advisory (roadmap 955, part 3)" in text and "does not block the merge" in text, text
        # --strict, the required shape, fails on the same table.
        assert verdict(strict=True)[0] == 1
        # A killed row never clears a survivor this run just watched live.
        tri.write_text(tri.read_text()
                       + "kernel,Wire::walk,replace < with <=,killed,a census saw it die,,,\n")
        code, text = verdict()
        assert code == 0 and "`killed` row is false here" in text, text
        tri.write_text(tri.read_text().replace(
            "kernel,Wire::walk,replace < with <=,killed,a census saw it die,,,\n",
            "kernel,Wire::walk,replace < with <=,gap,guards a real hang,,,\n"))
        code, text = verdict()
        assert code == 0 and "clean" in text and "nothing to triage" in text, text
        # A budget-cut shard says so; a missing table is a broken run, exit 2, and a malformed
        # row is the same.
        code, text = verdict(count=7)
        assert code == 0 and "stopped 3 mutant(s) short" in text, text
        missing = str(root / "absent.tsv")
        assert verdict(tsv=missing, count=1)[0] == 2
        bad = root / "bad.tsv"
        bad.write_text("mutant\tverdict\tkernel_s\tsystem_s\nonly-one-field\n")
        assert verdict(tsv=str(bad), count=1)[0] == 2

        # The shards command: an empty corpus and an over-cap diff both send the one sentinel
        # entry (a matrix that expands to zero combinations turns the run red), and say what the
        # pass is. Both exit 0. The matrix line goes to stdout, the human report to stderr and
        # the summary file.
        other = root / "other.txt"
        other.write_text(listing(("crates/other/src/lib.rs", 3, "replace == with != in f")))
        out = root / "out.txt"
        code, stdout, _err = _capture_shards(cmd_shards, argparse.Namespace(
            listing=[str(other)], toml=str(toml), roots="kernel,components",
            shard_dir=str(root / "s"), summary=str(out)))
        sentinel = json.loads(stdout.removeprefix("matrix="))
        assert code == 0 and sentinel["include"][0]["noop"] is True, stdout
        assert "not a mutation result" in out.read_text(), out.read_text()
        big = root / "big.txt"
        big.write_text(listing(*[("kernel/src/sched/thing.rs", i + 2,
                                  f"replace == with != in f{i}") for i in range(100)]))
        out2 = root / "out2.txt"
        code, stdout, _err = _capture_shards(cmd_shards, argparse.Namespace(
            listing=[str(big)], toml=str(toml), roots="kernel,components",
            shard_dir=str(root / "s2"), summary=str(out2)))
        sentinel = json.loads(stdout.removeprefix("matrix="))
        assert code == 0 and sentinel["include"][0]["noop"] is True, stdout
        assert "cap of 10" in out2.read_text(), out2.read_text()
        # A real plan writes its shard files, sends real entries, and each file re-parses to its
        # own blocks in order.
        out3 = root / "out3.txt"
        shard_dir = root / "s3"
        code, stdout, _err = _capture_shards(cmd_shards, argparse.Namespace(
            listing=[str(nine)], toml=str(toml), roots="kernel,components",
            shard_dir=str(shard_dir), summary=str(out3)))
        assert code == 0 and '"noop": false' in stdout, stdout
        with _driver_parser() as mutant_patches:
            pairs = mutant_patches((shard_dir / "shard-01.txt").read_text(), None)
            assert [n for n, _p in pairs] == [f"kernel/src/sched/thing.rs:{i + 2}:5:"
                                              f" replace == with != in f{i}" for i in range(7)]

    print("mutation_kernel_diff selftest: ok")
    return 0


def _driver_parser():
    """The driver's patch parser as a context manager, exec'd from script/falsifications's source
    (the script dispatches on sys.argv at import, so it cannot be imported whole)."""
    import contextlib
    src = (pathlib.Path(__file__).parent.parent / "script" / "falsifications").read_text()
    ns = {"re": re}
    start = src.index("MUTANT_HEADER = ")
    end = src.index("def mutants_tier")
    exec(compile(src[start:end], "falsifications-patch-parsing", "exec"), ns)

    @contextlib.contextmanager
    def ctx():
        yield ns["mutant_patches"]
    return ctx()


def _capture_shards(fn, args):
    """cmd_shards' three surfaces at once: the return code, stdout (the matrix line) and stderr
    (the human report)."""
    import io
    from contextlib import redirect_stderr, redirect_stdout
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        code = fn(args)
    return code, out.getvalue(), err.getvalue()


def _capture(fn, args):
    import io
    from contextlib import redirect_stdout
    buf = io.StringIO()
    with redirect_stdout(buf):
        code = fn(args)
    return code, buf.getvalue()


def _capture_err(fn, args):
    import io
    from contextlib import redirect_stderr
    buf = io.StringIO()
    with redirect_stderr(buf):
        return fn(args)


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("shards", help="filter, route and split a listing into shard patch files")
    s.add_argument("--listing", action="append", required=True)
    s.add_argument("--toml", default=".cargo/mutants.toml")
    s.add_argument("--roots", default="kernel,components")
    s.add_argument("--shard-dir", default="shards")
    s.add_argument("--summary")
    v = sub.add_parser("verdict", help="read a shard's TSV and write its advisory summary")
    v.add_argument("--tsv", required=True)
    v.add_argument("--arch", required=True)
    v.add_argument("--index", type=int, required=True)
    v.add_argument("--total", type=int, required=True)
    v.add_argument("--count", type=int)
    v.add_argument("--triage", default=TRIAGE_PATH)
    v.add_argument("--renames", default=RENAMES_PATH)
    v.add_argument("--strict", action="store_true")
    v.add_argument("--summary")
    args = ap.parse_args(argv)
    return cmd_shards(args) if args.cmd == "shards" else cmd_verdict(args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
