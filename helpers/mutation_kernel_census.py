"""The rolling census's glue: keep a rotation state, cut each night's two jobs out of the slice
list, read the wall clock into the driver's budget, and render the verdicts in the triage ledger's
vocabulary.

Part 4 of roadmap 955 (the kernel and its programs are mutated under QEMU): the rolling census,
which calef ruled on 2026-10-27 (UTC) at 60 runner-hours a week. The workflow is
.github/workflows/kernel-mutants-census.yml; each night it lists the whole corpus with
`cargo mutants --no-config --list --diff` (no `--in-diff`: the census's listing is the slice, not a
diff), hands it to `plan` here with the previous night's state, and runs the two jobs' windows with
`script/falsifications --mutants --patches`, the driver part 1 built and part 3's diff check
already feeds. `verdict` reads a job's TSV; `state` fuses the night's TSVs into the next state;
`record` appends the night's rows to the census record; `window` turns the wall clock into the
driver's `--budget`.

    python3 helpers/mutation_kernel_census.py plan --listing LISTING \\
        --toml .cargo/mutants.toml [--state state.json] [--shard-dir DIR] [--summary FILE] \\
        [--only SLICE] [--limit N]
    python3 helpers/mutation_kernel_census.py verdict --tsv TSV --slice S --arch A --slot K \\
        [--count C] [--triage CSV] [--renames CSV] [--summary FILE]
    python3 helpers/mutation_kernel_census.py window --arch A [--now S] [--stop S] [--override S]
    python3 helpers/mutation_kernel_census.py state --matrix JSON --material JSON --tsvs DIR \\
        --date D --run R --out state.json
    python3 helpers/mutation_kernel_census.py record --matrix JSON --tsvs DIR --date D --run R \\
        --csv PATH [--summary FILE]
    python3 helpers/mutation_kernel_census.py --selftest     # fixtures, no git, no cargo; script/lint

`plan` prints one `matrix=<json>` line on stdout for the workflow's GITHUB_OUTPUT and its human
report on stderr and `--summary`; it also writes `state-material.json`, the pruned state the night's
`state` job fuses its TSVs into. Both `verdict` and `plan` exit 0 unless the input is broken (exit
2, a broken run is never a pass, the diff gate's rule). The census runs advisory, always: a
survivor never fails it, and there is no `--strict`.

THE SLICE IS THE UNIT, and it is what the diff check's shard math cannot be. A diff check is
stateless: its listing is the pull request's own lines, its shards are counts of the diff's
mutants, and a rerun starts over. The census's listing is every mutant in the corpus, one rotation
is weeks of nights, and each night must resume where the last one stopped, so the state has to
survive between runs. It travels in artifacts: each night's `state` job uploads `census-state`, the
next night's `plan` job downloads the newest successful run's copy (same branch, so a proof
dispatch never touches the scheduled rotation), and a lost or expired artifact begins a fresh
rotation rather than a wrong one. Covered sets hold whole mutant names, so a shifted line re-runs
its mutant instead of hiding behind the old name; the diff check mutates every line a pull request
touches anyway, so the drift a name set double-runs is work that check already did.

THE WINDOW IS THE WALL CLOCK, not the shard math. A window's size exists only to keep the two
concurrent jobs' windows disjoint: it is the night's five hours less the setup, the baseline and a
margin, at part 1's measured per-mutant cost. The real budget is computed in the job, seconds
before the driver runs: `window` reads the clock, stops by 13:00 UTC whatever time the night began,
and refuses to start when even one mutant cannot fit (a late night is a short night, never an
overrun into the working day). A job whose window held mutants and produced no verdict row is a
broken run (`verdict` exits 2): zero is loud.

THE CORPUS FILTER, THE PATCH PARSER, THE VERDICT MACHINERY AND PART 1'S MEASURED NUMBERS are
imports from helpers/mutation_kernel_diff.py, not restatements: the toml's own instruction consumed
in code, `parse_blocks`, `read_tsv`, `decide` and the untriaged table the two checks share, and the
baseline and per-mutant seconds the cost workflow measured. What is census-specific here is only
what the diff check has no shape for: the slice table, the rotation state, the wall clock and the
record. The verdicts speak the ledger's language whole: a survivor keys on (crate, function,
mutation) under milestone 85 (mutation testing over the host crates)'s rule, and what clears one is
milestone 924 (a survivor on a pull request's own lines cannot merge untriaged)'s rule, an
`equivalent` or `gap` row or a test that kills it.

THE SLICE LIST is the block's order, "in order of what a wrong line costs": the confinement core
first, then `arch/` per architecture, then the programs' wiring, then `components` against the
system tests. Two slices sit past it and complete the coverage the block's "every kernel module"
demands: `kernel-rest` (every kernel file the named slices do not name) and `kernel-needing-crates`
(the crates the host census cannot reach: system_initializer, virtio, user_mode_runtime and the
contract trio). Both are provisional, like every name this lane mints.

THE RECORD is notes/project-metrics/kernel-mutation-census.csv, one row per slice a night worked,
appended through a pull request the way the host census records its own (a record other people
quote is never pushed to main unreviewed). Artifacts expire in about 90 days; the record does not.
The rotation check the block's "what holds the gain" asks for reads the newest row per slice.

Name: provisional, minted 2026-10-28 for milestone 955 (mutation testing reaches the kernel and its
programs under QEMU, a diff and a slice at a time), part 4; an architect names things.
"""

import argparse
import collections
import fnmatch
import json
import pathlib
import re
import sys
from datetime import datetime, timezone

from mutation_kernel_diff import (
    ARCH_ROUTE,
    BASELINE_SECONDS,
    PER_MUTANT_SECONDS,
    RENAMES_PATH,
    TRIAGE_PATH,
    VERDICT_KIND,
    corpus_globs,
    corpus_regexes,
    excluded,
    parse_blocks,
    read_tsv,
    under_roots,
    untriaged_table,
)
from mutation_inflow import load_renames, parse_mutant, read_triage
from mutation_diff_gate import decide

# The night the block set on 2026-10-27 (UTC): cron `0 8 * * 0,2-6` starts the window, and every
# job stops by 13:00 UTC, which is inside 00:00-06:00 Pacific under both forms of daylight time.
NIGHT_CRON = "0 8 * * 0,2-6"
STOP_HOUR_UTC = 13
# The nominal span, for the window size only. The real budget is read off the clock in the job.
WINDOW_SECONDS = 5 * 3600
# Checkout, toolchain, rust-cache, the QEMU cache restore, bootstrap: what a job does around the
# driver. The window size is an upper bound for disjointness, so this errs high (part 3's shard
# reserve of 180 s assumed warm caches; a census night may not have them).
SETUP_SECONDS = 600
# The budget's safety: one in-flight mutant at the slowest observed tier (about 220 s, part 1)
# plus the verdict step and the artifact upload, all of which land after the driver stops.
MARGIN_SECONDS = 900
# A window that cannot pay its baseline, one mutant and this much slack is skipped, not started:
# starting it would pay the baseline and run nothing, which `verdict` would have to call broken.
MIN_RUN_SECONDS = 300
# The block's number: two jobs a night, ten runner-hours at five hours each.
JOBS_PER_NIGHT = 2
# The block's estimate for a mutant the kernel tier cannot see (it is not in that image), so the
# mutant always pays the second tier: about 130 s of system boot on top of the kernel tier. The
# budget is the real bound; this only sizes the window.
SYSTEM_TIER_ADDER = 150

Slice = collections.namedtuple("Slice", "name globs arch cost")

# The block's order, "in order of what a wrong line costs". `cost` names the per-mutant model:
# "kernel" mutants mostly die in the kernel tier; "system" slices (components and the crates that
# only the system image builds) pay both tiers on nearly every mutant.
SLICES = (
    Slice("syscall", ("kernel/src/syscall.rs", "kernel/src/syscall/**"), "aarch64", "kernel"),
    Slice("revoke", ("kernel/src/revoke.rs",), "aarch64", "kernel"),
    Slice("cap", ("kernel/src/capability_object.rs",), "aarch64", "kernel"),
    Slice("memory_region", ("kernel/src/memory_region.rs",), "aarch64", "kernel"),
    Slice("sched", ("kernel/src/sched.rs", "kernel/src/sched/**"), "aarch64", "kernel"),
    Slice("arch-x86_64", ("kernel/src/arch/x86_64/**",), "x86_64", "kernel"),
    Slice("arch-riscv64", ("kernel/src/arch/riscv64/**",), "riscv64", "kernel"),
    Slice("arch-aarch64", ("kernel/src/arch/aarch64/**",), "aarch64", "kernel"),
    Slice("user", ("kernel/src/user.rs", "kernel/src/user/**"), "aarch64", "kernel"),
    # The block's last named slice: every mutant here pays the system tests, the proposal's own
    # reason it sits last.
    Slice("components", ("components/**",), "aarch64", "system"),
    # Past the block's list, and completing it: every kernel file the named slices do not name
    # (the arch modules the ISA directories do not hold, the drivers, the memory glue), then the
    # crates the host census cannot reach. Both provisional.
    Slice("kernel-rest", ("kernel/src/**",), "aarch64", "kernel"),
    Slice("kernel-needing-crates",
          ("crates/system_initializer/**", "crates/virtio/**", "crates/user_mode_runtime/**",
           "crates/hot_swap_contract/**", "crates/supervision_protocol/**",
           "crates/memory_region_heap/**"),
          "aarch64", "system"),
)
SLICE_ORDER = tuple(s.name for s in SLICES)
SLICE_TABLE = {s.name: s for s in SLICES}

# The packages the corpus keeps, which the proposal's own census priced: 18,518 mutants. A path
# under none of these roots is dropped before the slices are consulted.
ROOTS = ("kernel", "components", "crates/system_initializer", "crates/virtio",
         "crates/user_mode_runtime", "crates/hot_swap_contract", "crates/supervision_protocol",
         "crates/memory_region_heap")

RECORD_PATH = "notes/project-metrics/kernel-mutation-census.csv"
RECORD_HEADER = "date,run,rotation,slice,arch,mutants,survived,caught,unviable,stale,timeout\n"


# One rotation: the time one pass of the whole corpus takes at 60 runner-hours a week. The block's
# cost table prices a full rotation at 562 runner-hours, about 9.5 weeks, so 67 days.
ROTATION_DAYS = 67


def per_mutant(arch, cost):
    """Seconds one mutant of this slice costs: part 1's measured number, plus the block's system
    estimate for a slice the kernel tier cannot see."""
    base = PER_MUTANT_SECONDS[arch]
    return base + SYSTEM_TIER_ADDER if cost == "system" else base


def cap(arch, cost):
    """The window size: the night's span less setup, baseline and margin, in mutants. It keeps two
    jobs' windows disjoint; the wall clock, read in the job, is what actually stops them."""
    span = WINDOW_SECONDS - SETUP_SECONDS - BASELINE_SECONDS[arch] - MARGIN_SECONDS
    return span // per_mutant(arch, cost)


def slice_of(path):
    """The slice a file belongs to: the first slice whose globs match, so the named core wins over
    `kernel-rest`'s catch-all. None when no slice matches, which is a defect in the slice table."""
    for s in SLICES:
        if any(fnmatch.fnmatch(path, g) for g in s.globs):
            return s.name
    return None


def fresh_state():
    return {"rotation": 1, "night": None, "run": None, "covered": {}}


def load_state(path):
    """The previous night's `census-state`, or a fresh rotation when none was found. A state that
    exists but does not parse is a broken run, never a silent restart."""
    if path is None or not pathlib.Path(path).exists():
        return fresh_state(), False
    state = json.loads(pathlib.Path(path).read_text())
    if not isinstance(state.get("rotation"), int) or state["rotation"] < 1:
        raise ValueError(f"{path}: rotation must be a whole number, got {state.get('rotation')!r}")
    if not isinstance(state.get("covered"), dict):
        raise ValueError(f"{path}: covered must hold one name list per slice")
    return state, True


def cmd_plan(args):
    try:
        blocks = parse_blocks(pathlib.Path(args.listing).read_text())
        globs = corpus_globs(args.toml, ROOTS)
        regexes = corpus_regexes(args.toml)
        state, found = load_state(args.state)
        if args.only and args.only not in SLICE_TABLE:
            raise ValueError(f"unknown slice {args.only!r}; the slice list is {', '.join(SLICE_ORDER)}")
    except (OSError, ValueError) as e:
        print(f"mutation_kernel_census: {e}", file=sys.stderr)
        return 2

    kept = collections.OrderedDict()
    for b in blocks:
        if not under_roots(b.file, ROOTS) or excluded(b, globs, regexes):
            continue
        name = slice_of(b.file)
        if name is None:
            print(f"mutation_kernel_census: no slice holds {b.file}: the slice table and ROOTS"
                  " disagree", file=sys.stderr)
            return 2
        kept.setdefault(name, []).append(b)
    if not kept:
        print("mutation_kernel_census: the listing holds no corpus mutant; cargo-mutants or"
              " .cargo/mutants.toml broke", file=sys.stderr)
        return 2

    # The state's covered sets are pruned to the names today's listing still holds: a name the
    # listing no longer holds names a mutant that no longer exists, and keeping it would rot the
    # state one entry per tree edit. The names are the blocks' full header lines, the same
    # identity the driver's TSV rows carry.
    listed = {s: {b.full for b in bs} for s, bs in kept.items()}
    covered = {s: [n for n in state["covered"].get(s, []) if n in listed[s]] for s in listed}
    remaining = {s: [b for b in kept[s] if b.full not in set(covered[s])] for s in kept}
    rotation = state["rotation"]
    if sum(len(v) for v in remaining.values()) == 0:
        # Every mutant in the corpus is covered by this rotation: it is complete, and tonight
        # begins the next one from the front. A lost state artifact lands here too, honestly.
        rotation += 1
        covered = {s: [] for s in kept}
        remaining = {s: list(bs) for s, bs in kept.items()}

    def cap_for(s):
        c = cap(SLICE_TABLE[s].arch, SLICE_TABLE[s].cost)
        return min(c, args.limit) if args.limit else c

    unfinished = [s for s in SLICE_ORDER if remaining.get(s)]
    if args.only:
        unfinished = [args.only] if remaining.get(args.only) else []

    # Slot 1 takes the front-most unfinished slice; slot 2 takes the next one, or, when only one
    # slice is left, the stretch of the same slice behind slot 1's window. One slice per job, the
    # block's "two jobs a night, each taking a slice from a slice list".
    jobs = []
    if unfinished:
        s1 = unfinished[0]
        jobs.append((1, s1, remaining[s1][:cap_for(s1)]))
        if len(unfinished) > 1:
            s2 = unfinished[1]
            w2 = remaining[s2][:cap_for(s2)]
        else:
            s2 = s1
            c1 = cap_for(s1)
            w2 = remaining[s1][c1:c1 + c1]
        if w2:
            jobs.append((2, s2, w2))

    d = pathlib.Path(args.shard_dir)
    d.mkdir(parents=True, exist_ok=True)
    for stale in d.glob("job-*.txt"):
        stale.unlink()
    for slot, _s, w in jobs:
        (d / f"job-{slot}.txt").write_text("".join(b.raw for b in w))
    material = {"rotation": rotation, "covered": covered}
    (d / "state-material.json").write_text(json.dumps(material, indent=1) + "\n")

    entries = []
    for slot in range(1, JOBS_PER_NIGHT + 1):
        job = next((j for j in jobs if j[0] == slot), None)
        if job is None:
            entries.append({"slot": slot, "slice": "", "arch": "none", "file": "", "count": 0,
                            "rotation": rotation, "noop": True})
        else:
            _slot, s, w = job
            entries.append({"slot": slot, "slice": s, "arch": SLICE_TABLE[s].arch,
                            "file": f"job-{slot}.txt", "count": len(w), "rotation": rotation,
                            "noop": False})

    corpus = sum(len(bs) for bs in kept.values())
    done = corpus - sum(len(v) for v in remaining.values())
    out = [f"### Kernel mutants (rolling census): the plan for rotation {rotation}", ""]
    if found:
        out.append(f"State from night {state.get('night') or 'unknown'}"
                   f" (run {state.get('run') or 'unknown'}): the corpus holds {corpus} mutant(s),"
                   f" {done} covered this rotation, {sum(len(v) for v in remaining.values())}"
                   " remaining.")
    else:
        out.append(f"No state found (first night, or the artifact expired): the corpus holds"
                   f" {corpus} mutant(s), and this night begins rotation {rotation} from the"
                   " front.")
    if rotation != state["rotation"]:
        out.append(f"The previous rotation covered everything it listed, so it is complete and"
                   f" tonight begins rotation {rotation}.")
    out += [
        "",
        "| slice | arch | corpus | covered | remaining | tonight |",
        "|---|---|---|---|---|---|",
    ]
    for s in SLICES:
        if s.name not in kept:
            continue
        tonight = ""
        for slot, name, w in jobs:
            if name == s.name:
                tonight = (tonight + "; " if tonight else "") + f"slot {slot}: {len(w)}"
        out.append(f"| {s.name} | {s.arch} | {len(kept[s.name])} |"
                   f" {len(kept[s.name]) - len(remaining[s.name])} | {len(remaining[s.name])} |"
                   f" {tonight} |")
    if jobs:
        pieces = [f"slot {slot} takes {len(w)} of {s}'s remaining on {SLICE_TABLE[s].arch}"
                  for slot, s, w in jobs]
        out += ["", "Tonight: " + "; ".join(pieces) + "."]
    else:
        out += ["", "Nothing to run: every window is empty under the current state."]
    out += [
        "",
        f"A window's size is the {WINDOW_SECONDS // 3600}-hour night less {SETUP_SECONDS} s of"
        f" setup, the baseline ({BASELINE_SECONDS['aarch64']} s on aarch64,"
        f" {BASELINE_SECONDS['x86_64']} s on x86_64 and, estimated, riscv64) and a"
        f" {MARGIN_SECONDS} s margin, at {PER_MUTANT_SECONDS['aarch64']} s a mutant on aarch64 and"
        f" {PER_MUTANT_SECONDS['x86_64']} s on x86_64 and riscv64 (part 1's measurement), or that"
        f" plus a {SYSTEM_TIER_ADDER} s estimate for a slice the kernel tier cannot see"
        " (components and the kernel-needing crates). The real budget is the wall clock: each job"
        f" reads it seconds before the driver runs and stops cleanly by {STOP_HOUR_UTC:02d}:00 UTC,"
        " whatever time the night began, so a late night is a short night.",
        "",
        f"Schedule: cron `{NIGHT_CRON}`, two jobs a night, Monday free for the host census. The"
        f" record is {RECORD_PATH}, one row per slice a night worked.",
    ]
    text = "\n".join(out) + "\n"
    print(text, file=sys.stderr, end="" if text.endswith("\n") else "\n")
    if args.summary:
        with open(args.summary, "a") as fh:
            fh.write(text)
    print("matrix=" + json.dumps({"include": entries}))
    return 0


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
        print(f"mutation_kernel_census: {e}", file=sys.stderr)
        return 2
    if args.count and not rows:
        print(f"mutation_kernel_census: the window held {args.count} mutant(s) and the driver"
              " produced no verdict row; a window that runs nothing is a broken run, never a pass",
              file=sys.stderr)
        return 2
    kinds = collections.Counter(v for _n, v in rows)
    ran = ", ".join(f"{v} {n}" for v, n in sorted(kinds.items())) or "no mutant ran"
    head = (f"### Kernel census (advisory), slice {args.slice} on {args.arch}, job {args.slot}"
            f" of {JOBS_PER_NIGHT}: "
            + (f"{len(untriaged)} survivor key(s) untriaged" if untriaged else "clean"))
    body = [
        "",
        f"{args.count if args.count is not None else len(rows)} mutant(s) in the window,"
        f" {len(rows)} run ({ran}).",
    ]
    if args.count is not None and len(rows) < args.count:
        body += [
            "",
            f"The job stopped {args.count - len(rows)} mutant(s) short of its window: the wall"
            " clock reached its stop. The slice resumes from here on the next night, and the"
            " census record shows what tonight got.",
        ]
    if untriaged:
        body += [
            "",
            f"{cleared} key(s) cleared by the ledger; {len(untriaged)} did not clear, and under"
            f" milestone 85's rule each needs a row in {TRIAGE_PATH} (`equivalent` or `gap`) or a"
            " test that kills it. A timeout is untriaged the same as a survivor (milestone 924's"
            " rule, and this run watched the mutant live, so a `killed` row never clears one).",
            "",
        ]
        body += untriaged_table(untriaged)
        body += [
            "",
            "This check is advisory (roadmap 955, part 4): it never blocks a merge, and the"
            " census revisits this slice next rotation.",
        ]
    else:
        body += [
            "",
            "No survivor and no timeout, so there is nothing to triage. This pass is a mutation"
            " result: the window's mutants ran under QEMU in two tiers, the kernel suite first,"
            " the system image only for what survived it.",
        ]
    text = "\n".join([head] + body) + "\n"
    print(text, end="" if text.endswith("\n") else "\n")
    if args.summary:
        with open(args.summary, "a") as fh:
            fh.write(text)
    return 0


def cmd_window(args):
    """The driver's `--budget` for this job, read seconds before the driver runs. Prints
    `budget=<seconds>` and `skip=<0|1>` for the job's shell to eval."""
    if args.arch not in BASELINE_SECONDS:
        print(f"mutation_kernel_census: unknown arch {args.arch!r}", file=sys.stderr)
        return 2
    if args.override is not None:
        if args.override < 0:
            print("mutation_kernel_census: --override must be a whole number of seconds",
                  file=sys.stderr)
            return 2
        print(f"budget={args.override}")
        print("skip=0")
        return 0
    now = args.now if args.now is not None else datetime.now(timezone.utc).timestamp()
    if args.stop is not None:
        stop = args.stop
    else:
        stop = datetime.now(timezone.utc).replace(
            hour=STOP_HOUR_UTC, minute=0, second=0, microsecond=0).timestamp()
    budget = int(stop - now - MARGIN_SECONDS)
    threshold = BASELINE_SECONDS[args.arch] + PER_MUTANT_SECONDS[args.arch] + MIN_RUN_SECONDS
    print(f"budget={max(budget, 0)}")
    print(f"skip={1 if budget < threshold else 0}")
    return 0


def _tsv_slots(tsvs_dir, matrix):
    """The downloaded census-tsv-* artifacts as (slot, slice, arch, tsv path) tuples, mapped
    through the plan's matrix so each table lands on the slice that produced it. A slot that
    skipped or ran nothing uploaded no artifact and is simply absent.

    Both layouts download-artifact produces are read: several artifacts arrive one subdirectory
    each (`tsvs/census-tsv-1/census-1.tsv`), a lone artifact arrives flat (`tsvs/census-1.tsv`),
    and the flat shape is the one a night with exactly one table hits, which is precisely the case
    the host census's report job once lost (its `shards/mutants-shard-*` glob matched nothing)."""
    slot_slice = {e["slot"]: e["slice"] for e in matrix["include"] if not e.get("noop")}
    slot_arch = {e["slot"]: e["arch"] for e in matrix["include"] if not e.get("noop")}
    found = {}
    root = pathlib.Path(tsvs_dir)
    if not root.exists():
        return []
    for tsv in sorted(root.rglob("*.tsv")):
        slot = None
        if tsv.parent.name.startswith("census-tsv-"):
            try:
                slot = int(tsv.parent.name.rsplit("-", 1)[1])
            except ValueError:
                pass
        if slot is None:
            m = re.fullmatch(r"census-(\d+)\.tsv", tsv.name)
            if m:
                slot = int(m.group(1))
        if slot in slot_slice:
            found[slot] = (slot, slot_slice[slot], slot_arch[slot], tsv)
    return [found[k] for k in sorted(found)]


def cmd_state(args):
    """Fuse the night's TSVs into the plan's pruned state and write the next census-state. A
    `stale` verdict measured nothing (the patch did not apply), so it covers nothing."""
    try:
        matrix = json.loads(args.matrix)
        material = json.loads(pathlib.Path(args.material).read_text())
        rotation = material["rotation"]
        if not isinstance(rotation, int) or rotation < 1:
            raise ValueError(f"{args.material}: rotation must be a whole number, got {rotation!r}")
        covered = {s: list(v) for s, v in material["covered"].items()}
        merged = 0
        for slot, s, _arch, tsv in _tsv_slots(args.tsvs, matrix):
            for name, verdict in read_tsv(tsv):
                if verdict == "stale":
                    continue
                bucket = covered.setdefault(s, [])
                if name not in bucket:
                    bucket.append(name)
                merged += 1
    except (OSError, ValueError, KeyError) as e:
        print(f"mutation_kernel_census: {e}", file=sys.stderr)
        return 2
    state = {"rotation": rotation, "night": args.date, "run": args.run, "covered": covered}
    pathlib.Path(args.out).write_text(json.dumps(state, indent=1) + "\n")
    total = sum(len(v) for v in covered.values())
    print(f"mutation_kernel_census: rotation {rotation} state written: {merged} verdict(s)"
          f" merged, {total} name(s) covered", file=sys.stderr)
    return 0


def cmd_record(args):
    """Append one row per slice a night worked to the census record. Rows are keyed on (run,
    slice), so a re-run adds nothing; the file ships with its header, and the command creates it
    if the tree ever lacks it."""
    try:
        matrix = json.loads(args.matrix)
        pairs = _tsv_slots(args.tsvs, matrix)
        csv_path = pathlib.Path(args.csv)
        lines = csv_path.read_text().splitlines() if csv_path.exists() else []
    except (OSError, ValueError, KeyError) as e:
        print(f"mutation_kernel_census: {e}", file=sys.stderr)
        return 2
    have = {(parts[1], parts[3]) for parts in (l.split(",") for l in lines) if len(parts) >= 4}
    rows = []
    for slot, s, arch, tsv in pairs:
        if (args.run, s) in have:
            continue
        kinds = collections.Counter(v for _n, v in read_tsv(tsv))
        rows.append(",".join(str(x) for x in (
            args.date, args.run, matrix["include"][0]["rotation"], s, arch, sum(kinds.values()),
            kinds["survived"], kinds["caught-kernel"] + kinds["caught-system"],
            kinds["unviable"], kinds["stale"],
            kinds["timeout-kernel"] + kinds["timeout-system"])))
    if not rows:
        print("mutation_kernel_census: nothing to record (no window produced a table, or every"
              " row is already recorded)", file=sys.stderr)
        return 0
    if not lines:
        lines = [RECORD_HEADER.rstrip("\n")]
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    csv_path.write_text("\n".join(lines + rows) + "\n")
    text = "### Kernel mutation census: recorded" + "\n\n| date | run | slice | mutants | survived | caught |\n|---|---|---|---|---|---|\n"
    for r in rows:
        parts = r.split(",")
        text += f"| {parts[0]} | {parts[1]} | {parts[3]} | {parts[5]} | {parts[6]} | {parts[7]} |\n"
    print(text, end="")
    if args.summary:
        with open(args.summary, "a") as fh:
            fh.write(text)
    return 0


def cmd_rotation(args):
    """Report each slice whose newest record row is older than one rotation, for
    script/cadence-check. Exit 1 when one is, 0 otherwise, 2 when the record cannot be read.

    A slice with no row is not yet due until the record's oldest row is itself a rotation old: the
    census works its slices in order, so a late slice legitimately waits for its turn. An empty
    record means the census has not begun, which is not a finding."""
    try:
        lines = pathlib.Path(args.csv).read_text().splitlines()
        today = datetime.strptime(args.today, "%Y-%m-%d") if args.today else datetime.now(timezone.utc).replace(tzinfo=None)
        rows = [l.split(",") for l in lines[1:] if l.strip()]
        newest = {}
        oldest = None
        for r in rows:
            d = datetime.strptime(r[0], "%Y-%m-%d")
            newest[r[3]] = max(d, newest.get(r[3], d))
            oldest = d if oldest is None else min(oldest, d)
    except (OSError, ValueError, IndexError) as e:
        print(f"mutation_kernel_census: {e}", file=sys.stderr)
        return 2
    late = []
    for name in SLICE_ORDER:
        d = newest.get(name)
        if d is not None:
            age = (today - d).days
            if age > ROTATION_DAYS:
                late.append(f"{name}: last worked {d:%Y-%m-%d}, {age} days ago")
        elif oldest is not None and (today - oldest).days > ROTATION_DAYS:
            late.append(f"{name}: never worked in {(today - oldest).days} days of record")
    for l in late:
        print(f"DEAD: kernel census slice {l} (a rotation is {ROTATION_DAYS} days)")
    return 1 if late else 0


def selftest():
    def listing(*blocks):
        return "\n".join("\n".join((f"{f}:{l}:5: {n}", f"--- {f}", f"+++ {n}", "@@ -1,2 +1,2 @@",
                                    "-a", "+b", "")) for f, l, n in blocks) + "\n"

    import tempfile
    with tempfile.TemporaryDirectory(prefix="nife955-census-selftest-") as tmp:
        root = pathlib.Path(tmp)
        toml = root / "mutants.toml"
        toml.write_text(
            'exclude_globs = [\n'
            '  "kernel/**",\n'          # the host census's crate-whole entry: must NOT end the run
            '  "kernel/src/bench/**",\n'
            '  "**/src/**/proofs.rs",\n'
            ']\n'
            'exclude_re = ["tests::"]\n'
        )

        # The slice table: every root it must cover has a slice that would hold a path under it,
        # the named slices win over the catch-all, and the arch slices carry their architecture.
        for r in ROOTS:
            assert slice_of(f"{r}/src/x.rs") is not None, r
        assert slice_of("kernel/src/syscall/entry.rs") == "syscall"
        assert slice_of("kernel/src/syscall/memory_region.rs") == "syscall"  # first match wins
        assert slice_of("kernel/src/syscall.rs") == "syscall"
        assert slice_of("kernel/src/revoke.rs") == "revoke"
        assert slice_of("kernel/src/capability_object.rs") == "cap"
        assert slice_of("kernel/src/memory_region.rs") == "memory_region"
        assert slice_of("kernel/src/sched/rendezvous.rs") == "sched"
        assert slice_of("kernel/src/arch/x86_64/gdt.rs") == "arch-x86_64"
        assert slice_of("kernel/src/arch/riscv64/trap.rs") == "arch-riscv64"
        assert slice_of("kernel/src/arch/aarch64/semihosting.rs") == "arch-aarch64"
        assert slice_of("kernel/src/user.rs") == "user"
        assert slice_of("components/src/init.rs") == "components"
        assert slice_of("kernel/src/kmem.rs") == "kernel-rest"      # the catch-all
        assert slice_of("kernel/src/drivers/foo.rs") == "kernel-rest"
        assert slice_of("crates/virtio/src/queue.rs") == "kernel-needing-crates"
        assert slice_of("crates/calendar/src/lib.rs") is None       # a host crate: no slice
        for s in SLICES:
            assert s.arch in ARCH_ROUTE, s
        # The window math: the caps fit the span at the measured cost, and the shared numbers are
        # the diff helper's, not a restatement.
        assert cap("aarch64", "kernel") == (WINDOW_SECONDS - SETUP_SECONDS - 380 - MARGIN_SECONDS) // 81
        assert cap("x86_64", "kernel") == (WINDOW_SECONDS - SETUP_SECONDS - 785 - MARGIN_SECONDS) // 106
        assert cap("riscv64", "kernel") == cap("x86_64", "kernel")   # the estimate borrows x86_64
        assert cap("aarch64", "system") == (WINDOW_SECONDS - SETUP_SECONDS - 380 - MARGIN_SECONDS) // (81 + SYSTEM_TIER_ADDER)

        big = listing(*[("kernel/src/syscall/entry.rs", i + 2,
                         f"replace == with != in f{i}") for i in range(300)])
        raw = root / "listing.txt"
        raw.write_text(listing(
            ("kernel/src/syscall/entry.rs", 9, "replace == with != in Entry::fire"),
            ("kernel/src/syscall.rs", 3, "replace < with <= in dispatch"),
            ("kernel/src/revoke.rs", 9, "replace == with != in Revoke::revoke"),
            ("kernel/src/capability_object.rs", 4, "replace | with ^ in Cap::subset"),
            ("kernel/src/memory_region.rs", 6, "replace == with != in Region::map"),
            ("kernel/src/sched/rendezvous.rs", 5, "replace == with != in Rsvp::wait"),
            ("kernel/src/arch/x86_64/gdt.rs", 7, "replace + with - in Gdt::load"),
            ("kernel/src/user.rs", 2, "replace == with != in User::wire"),
            ("kernel/src/bench/meters.rs", 3, "replace + with - in Meters::add"),   # toml drops
            ("kernel/src/syscall/proofs.rs", 1, "replace | with ^ in subset"),      # toml drops
            ("kernel/src/glob.rs", 2, "replace == with != in tests::helper"),       # regex drops
            ("kernel/src/kmem.rs", 4, "replace == with != in Kmem::pool"),
            ("components/src/init.rs", 6, "replace == with != in Init::start"),
            ("crates/virtio/src/queue.rs", 8, "replace == with != in Queue::kick"),
            ("crates/calendar/src/lib.rs", 3, "replace == with != in f"),           # outside roots
        ) + big)

        def plan_args(**over):
            base = dict(listing=str(raw), toml=str(toml), state=None, shard_dir=str(root / "s"),
                        only=None, limit=None, summary=None)
            base.update(over)
            return argparse.Namespace(**base)

        from mutation_kernel_diff import _capture_shards, _driver_parser

        # A fresh rotation: slot 1 opens the front-most slice, slot 2 opens the next one, and the
        # material carries rotation 1 with nothing covered.
        code, out, err = _capture_shards(cmd_plan, plan_args())
        assert code == 0, err
        matrix = json.loads(out.removeprefix("matrix="))["include"]
        assert [e["slot"] for e in matrix] == [1, 2] and not any(e["noop"] for e in matrix), out
        assert matrix[0]["slice"] == "syscall" and matrix[0]["arch"] == "aarch64"
        assert matrix[0]["count"] == cap("aarch64", "kernel"), matrix[0]
        assert matrix[1]["slice"] == "revoke" and matrix[1]["count"] == 1, matrix[1]
        assert matrix[0]["rotation"] == 1
        material = json.loads((root / "s" / "state-material.json").read_text())
        assert material["rotation"] == 1 and not any(material["covered"].values()), material
        job1 = (root / "s" / "job-1.txt").read_text()
        with _driver_parser() as mutant_patches:
            pairs = mutant_patches(job1, None)
            assert len(pairs) == cap("aarch64", "kernel"), len(pairs)
            assert pairs[0][0] == "kernel/src/syscall/entry.rs:9:5: replace == with != in Entry::fire", pairs[0]

        # The resume: a state covering syscall and revoke pushes both windows to the next slices.
        st = root / "state.json"
        syscall_kept = _all_kept(raw, str(toml))["syscall"]
        st.write_text(json.dumps({"rotation": 3, "night": "2026-10-27", "run": 99, "covered": {
            "syscall": [b.full for b in syscall_kept],
            "revoke": ["kernel/src/revoke.rs:9:5: replace == with != in Revoke::revoke"],
        }}))
        code, out, err = _capture_shards(cmd_plan, plan_args(state=str(st)))
        assert code == 0, err
        matrix = json.loads(out.removeprefix("matrix="))["include"]
        assert matrix[0]["slice"] == "cap" and matrix[1]["slice"] == "memory_region", out
        assert matrix[0]["rotation"] == 3
        assert "run 99" in err, err

        # A stale covered name (the tree shifted the line) is pruned, not carried.
        st.write_text(json.dumps({"rotation": 4, "night": "2026-10-28", "run": 100, "covered": {
            "syscall": ["kernel/src/syscall/entry.rs:999:5: gone"]}}))
        code, out, err = _capture_shards(cmd_plan, plan_args(state=str(st)))
        material = json.loads((root / "s" / "state-material.json").read_text())
        assert material["covered"]["syscall"] == [], material
        assert material["rotation"] == 4

        # The rollover: a state covering every slice restarts the rotation from the front.
        everything = {s: [b.full for b in bs] for s, bs in _all_kept(raw, str(toml)).items()}
        st.write_text(json.dumps({"rotation": 7, "night": "2026-10-28", "run": 101,
                                  "covered": everything}))
        code, out, err = _capture_shards(cmd_plan, plan_args(state=str(st)))
        assert code == 0, err
        matrix = json.loads(out.removeprefix("matrix="))["include"]
        assert matrix[0]["rotation"] == 8 and matrix[0]["slice"] == "syscall", out
        assert "begins rotation 8" in err, err

        # The proof path: --only with --limit gives both slots the same slice in disjoint windows.
        code, out, err = _capture_shards(cmd_plan, plan_args(only="syscall", limit=1))
        matrix = json.loads(out.removeprefix("matrix="))["include"]
        assert matrix[0]["count"] == 1 and matrix[1]["count"] == 1, out
        assert matrix[0]["file"] != matrix[1]["file"]
        with _driver_parser() as mutant_patches:
            first = mutant_patches((root / "s" / "job-1.txt").read_text(), None)[0][0]
            second = mutant_patches((root / "s" / "job-2.txt").read_text(), None)[0][0]
            assert first != second, (first, second)

        # An --only slice the state already covers: both slots noop, exit 0, nothing to run.
        st.write_text(json.dumps({"rotation": 5, "night": "2026-10-28", "run": 102, "covered": {
            "syscall": [b.full for b in _all_kept(raw, str(toml))["syscall"]]}}))
        code, out, err = _capture_shards(cmd_plan, plan_args(state=str(st), only="syscall"))
        matrix = json.loads(out.removeprefix("matrix="))["include"]
        assert all(e["noop"] for e in matrix) and code == 0, out
        # An unknown slice name is a broken plan, exit 2.
        assert _capture_shards(cmd_plan, plan_args(only="nope"))[0] == 2
        # A corpus with nothing in it is broken too: the census cannot proceed on an empty night.
        empty = root / "empty.txt"
        empty.write_text(listing(("crates/calendar/src/lib.rs", 3, "replace == with != in f")))
        assert _capture_shards(cmd_plan, plan_args(listing=str(empty)))[0] == 2
        # A malformed state is a broken run, never a silent fresh start.
        bad = root / "bad.json"
        bad.write_text('{"rotation": "eight", "covered": {}}')
        assert _capture_shards(cmd_plan, plan_args(state=str(bad)))[0] == 2

        # The verdict: a survivor keys and renders as the diff check's does; the advisory exit is
        # 0; a window that ran nothing, or no table at all, is exit 2.
        tri = root / "triage.csv"
        tri.write_text(
            "crate,function,mutation,disposition,reason,kind,verified,home\n"
            "kernel,Revoke::revoke,replace == with !=,equivalent,both arms refuse,algebraic,2026-10-28,\n"
        )
        renames = str(root / "absent.csv")
        tsv = root / "job.tsv"
        tsv.write_text(
            "mutant\tverdict\tkernel_s\tsystem_s\n"
            "kernel/src/revoke.rs:9:5: replace == with != in Revoke::revoke\tsurvived\t16\t126\n"
            "kernel/src/kmem.rs:4:5: replace == with != in Kmem::pool\tcaught-kernel\t17\t\n"
            "kernel/src/user.rs:2:5: replace == with != in User::wire\ttimeout-system\t18\t420\n"
            "kernel/src/capability_object.rs:4:5: replace | with ^ in Cap::subset\tunviable\t4\t\n"
        )

        def verdict_args(**over):
            base = dict(tsv=str(tsv), slice="kernel-rest", arch="aarch64", slot=1, count=4,
                        triage=str(tri), renames=renames, summary=None)
            base.update(over)
            return argparse.Namespace(**base)

        code, text = _capture(cmd_verdict, verdict_args())
        assert code == 0, text
        assert "slice kernel-rest on aarch64, job 1 of 2: 1 survivor key(s) untriaged" in text, text
        assert "1 key(s) cleared by the ledger" in text, text
        assert "| kernel | `Revoke::revoke`" not in text, text       # the equivalent row cleared it
        assert "| kernel | `User::wire` | `replace == with !=` | timeout |" in text, text
        assert "advisory (roadmap 955, part 4)" in text and "never blocks a merge" in text, text
        assert "stopped 0 mutant(s) short" not in text, text
        # A budget-cut window says so in the census's own words.
        code, text = _capture(cmd_verdict, verdict_args(count=7))
        assert code == 0 and "stopped 3 mutant(s) short of its window" in text, text
        assert "resumes from here on the next night" in text, text
        # Zero is loud: a window that held mutants and produced no rows is a broken run.
        zero = root / "zero.tsv"
        zero.write_text("mutant\tverdict\tkernel_s\tsystem_s\n")
        code, text = _capture(cmd_verdict, verdict_args(tsv=str(zero)))
        assert code == 2 and "runs nothing is a broken run" in text, text
        # So is a missing table.
        assert _capture(cmd_verdict, verdict_args(tsv=str(root / "absent.tsv")))[0] == 2

        # The window: the override bypasses the clock; the normal night budgets to the stop; a
        # start too late to pay even one mutant is skipped, not started.
        def window_args(**over):
            base = dict(arch="aarch64", now=3600.0, stop=7200.0, override=None)
            base.update(over)
            return argparse.Namespace(**base)

        code, out = _capture(cmd_window, window_args())
        assert code == 0 and out == "budget=2700\nskip=0\n", out   # 3600 - 900 margin
        code, out = _capture(cmd_window, window_args(now=6200.0))
        assert out == "budget=100\nskip=1\n", out                  # under baseline+1+slack
        code, out = _capture(cmd_window, window_args(now=8000.0))
        assert out == "budget=0\nskip=1\n", out                    # past the stop: clamped
        code, out = _capture(cmd_window, window_args(override=1200))
        assert out == "budget=1200\nskip=0\n", out
        assert _capture(cmd_window, window_args(arch="m68k"))[0] == 2

        # The state: the night's tables fuse into the material per slot, a stale verdict covers
        # nothing, and the rotation and night stamp travel.
        tsvs = root / "tsvs"
        (tsvs / "census-tsv-1").mkdir(parents=True)
        (tsvs / "census-tsv-1" / "census-1.tsv").write_text(tsv.read_text())
        st_mat = root / "s" / "state-material.json"
        st_mat.write_text(json.dumps({"rotation": 4, "covered": {}}))
        plan_matrix = json.dumps({"include": [
            {"slot": 1, "slice": "kernel-rest", "arch": "aarch64", "count": 4, "noop": False,
             "rotation": 4, "file": "job-1.txt"},
            {"slot": 2, "slice": "", "arch": "none", "count": 0, "noop": True, "rotation": 4,
             "file": ""}]})
        out_state = root / "next-state.json"
        code, _ = _capture_code(cmd_state, argparse.Namespace(
            matrix=plan_matrix, material=str(st_mat), tsvs=str(tsvs), date="2026-10-28",
            run="40640000000", out=str(out_state)))
        assert code == 0
        fused = json.loads(out_state.read_text())
        assert fused["rotation"] == 4 and fused["night"] == "2026-10-28" and fused["run"] == "40640000000"
        names = fused["covered"]["kernel-rest"]
        assert "kernel/src/revoke.rs:9:5: replace == with != in Revoke::revoke" in names
        assert "kernel/src/capability_object.rs:4:5: replace | with ^ in Cap::subset" in names
        assert len(names) == 4  # survived, caught-kernel, timeout-system and unviable cover; stale alone would not
        # A malformed material is a broken state, exit 2.
        bad_mat = root / "bad-material.json"
        bad_mat.write_text('{"rotation": null, "covered": {}}')
        assert _capture_code(cmd_state, argparse.Namespace(
            matrix=plan_matrix, material=str(bad_mat), tsvs=str(tsvs), date="2026-10-28",
            run="1", out=str(root / "nope.json")))[0] == 2

        # The lone-table layout: one artifact downloaded flat (`tsvs/census-1.tsv`), the shape a
        # night with exactly one table hits, which is precisely the case the host census's report
        # job once lost.
        flat = root / "flat-tsvs"
        flat.mkdir()
        (flat / "census-1.tsv").write_text(tsv.read_text())
        code, _ = _capture_code(cmd_state, argparse.Namespace(
            matrix=plan_matrix, material=str(st_mat), tsvs=str(flat), date="2026-10-28",
            run="40640000001", out=str(root / "flat-state.json")))
        assert code == 0
        fused = json.loads((root / "flat-state.json").read_text())
        assert "kernel/src/revoke.rs:9:5: replace == with != in Revoke::revoke" in fused["covered"]["kernel-rest"], fused

        # The record: one row per slice that ran, counts by verdict family, and a second call with
        # the same run adds nothing.
        csv = root / "census.csv"
        code, text = _capture(cmd_record, argparse.Namespace(
            matrix=plan_matrix, tsvs=str(tsvs), date="2026-10-28", run="40640000000",
            csv=str(csv), summary=None))
        assert code == 0, text
        body = csv.read_text()
        assert body.startswith("date,run,rotation,slice,arch,mutants,survived,caught,unviable,stale,timeout\n"), body
        assert "2026-10-28,40640000000,4,kernel-rest,aarch64,4,1,1,1,0,1\n" in body, body
        code, _ = _capture(cmd_record, argparse.Namespace(
            matrix=plan_matrix, tsvs=str(tsvs), date="2026-10-28", run="40640000000",
            csv=str(csv), summary=None))
        assert csv.read_text() == body
        # Nothing ran: nothing to record, exit 0.
        empty_tsvs = root / "empty-tsvs"
        empty_tsvs.mkdir()
        code, _ = _capture(cmd_record, argparse.Namespace(
            matrix=plan_matrix, tsvs=str(empty_tsvs), date="2026-10-28", run="2",
            csv=str(csv), summary=None))
        assert code == 0 and csv.read_text() == body

        # The rotation check: a young record is quiet, a stale slice or a never-worked slice
        # once the record is a rotation old is reported, a current record is quiet.
        rec = root / "rot.csv"
        rec.write_text(RECORD_HEADER)
        ns = lambda today: argparse.Namespace(csv=str(rec), today=today)
        assert _capture_code(cmd_rotation, ns("2027-01-01"))[0] == 0
        rec.write_text(RECORD_HEADER + "2026-10-29,1,1,syscall,aarch64,4,1,1,1,0,1\n")
        assert _capture_code(cmd_rotation, ns("2026-12-01"))[0] == 0
        code, text = _capture_code(cmd_rotation, ns("2027-01-10"))
        assert code == 1 and "syscall: last worked 2026-10-29" in text and "revoke: never worked" in text, text
        rec.write_text(RECORD_HEADER + "".join(f"2026-12-30,{i},2,{n},aarch64,1,0,1,0,0,0\n" for i, n in enumerate(SLICE_ORDER)))
        assert _capture_code(cmd_rotation, ns("2027-01-10"))[0] == 0
        assert _capture_code(cmd_rotation, argparse.Namespace(csv=str(root / "none.csv"), today=None))[0] == 2

    print("mutation_kernel_census selftest: ok")
    return 0


def _all_kept(listing_path, toml_path):
    """Every block the corpus keeps, per slice: the rollover fixture's covered set."""
    globs = corpus_globs(toml_path, ROOTS)
    regexes = corpus_regexes(toml_path)
    kept = collections.OrderedDict()
    for b in parse_blocks(pathlib.Path(listing_path).read_text()):
        if under_roots(b.file, ROOTS) and not excluded(b, globs, regexes):
            kept.setdefault(slice_of(b.file), []).append(b)
    return kept


def _capture_code(fn, args):
    """The return code only, with both streams swallowed."""
    import io
    from contextlib import redirect_stderr, redirect_stdout
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        return fn(args), out.getvalue()


def _capture(fn, args):
    import io
    from contextlib import redirect_stderr, redirect_stdout
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        code = fn(args)
    return code, out.getvalue() + err.getvalue()


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("plan", help="cut tonight's two windows out of the slice list")
    p.add_argument("--listing", required=True)
    p.add_argument("--toml", default=".cargo/mutants.toml")
    p.add_argument("--state", help="the previous night's census-state artifact")
    p.add_argument("--shard-dir", default="shards")
    p.add_argument("--only", help="the proof path: one slice only")
    p.add_argument("--limit", type=int, help="the proof path: cap both windows at N mutants")
    p.add_argument("--summary")
    v = sub.add_parser("verdict", help="read a job's TSV and write its advisory summary")
    v.add_argument("--tsv", required=True)
    v.add_argument("--slice", required=True)
    v.add_argument("--arch", required=True)
    v.add_argument("--slot", type=int, required=True)
    v.add_argument("--count", type=int)
    v.add_argument("--triage", default=TRIAGE_PATH)
    v.add_argument("--renames", default=RENAMES_PATH)
    v.add_argument("--summary")
    w = sub.add_parser("window", help="turn the wall clock into the driver's --budget")
    w.add_argument("--arch", required=True)
    w.add_argument("--now", type=float, help="seconds since the epoch; the clock, unless staged")
    w.add_argument("--stop", type=float, help="the stop's epoch; 13:00 UTC today, unless staged")
    w.add_argument("--override", type=int, help="the proof path: budget seconds, no clock read")
    s = sub.add_parser("state", help="fuse the night's TSVs into the next census-state")
    s.add_argument("--matrix", required=True)
    s.add_argument("--material", required=True)
    s.add_argument("--tsvs", required=True)
    s.add_argument("--date", required=True)
    s.add_argument("--run", required=True)
    s.add_argument("--out", required=True)
    r = sub.add_parser("record", help="append the night's rows to the census record")
    r.add_argument("--matrix", required=True)
    r.add_argument("--tsvs", required=True)
    r.add_argument("--date", required=True)
    r.add_argument("--run", required=True)
    r.add_argument("--csv", required=True)
    r.add_argument("--summary")
    o = sub.add_parser("rotation", help="report a slice older than one rotation (cadence-check)")
    o.add_argument("--csv", default=RECORD_PATH)
    o.add_argument("--today", help="YYYY-MM-DD; the clock, unless staged")
    args = ap.parse_args(argv)
    return {"rotation": cmd_rotation, "plan": cmd_plan, "verdict": cmd_verdict, "window": cmd_window,
            "state": cmd_state, "record": cmd_record}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
