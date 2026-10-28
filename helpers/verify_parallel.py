#!/usr/bin/env python3
"""Size a local `script/verify --parallel` run: how many shard children it starts, and why.

    python3 helpers/verify_parallel.py plan --shards S --slots L [--jobs J] [--wanted N] [--memory BYTES|auto]
    python3 helpers/verify_parallel.py --selftest

`plan` prints the child count on stdout and every reason on stderr, so the shell reads one word and
a person reads the reasons. It exits 2 when not even one child fits, and script/verify then refuses
to start rather than starting a run it cannot size.

Name: provisional, minted 2026-10-28 (UTC) by lane/milestone/956-local-parallel-prove. A python
helper under `helpers/`, outside `script/names`' scope, so this paragraph is its provenance. An
architect names things, and `NIFE_VERIFY_MEMORY_BYTES` beside it is provisional the same way.

# WHY

Three bounds decide the count, and each is a fact another file owns:

- CI's shard count, read by the caller from `.github/workflows/verify.yml` through
  `helpers/verify_times.py shards`. More children than CI runs prove smaller slices of the same
  table and buy nothing: `glob` is atomic at crate granularity and it is the floor, so a fifth shard
  is idle time (design/roadmap/0957-local-parallel-prove.md has the measurement).
- The machine's heavy-job slots (`helpers/heavy_slot.py`, one per 8 GiB of memory). The parent holds
  one slot and the children run inside it, so the slot count is what the machine is willing to carry
  and the default count must not exceed it.
- Memory. Each child runs VERIFY_JOBS harnesses at once (4 by default) and one harness has been
  measured at 3.5 GiB (`notes/verification/glob-proofs.md`). That figure is a peak of the largest
  harness, not a typical, so budgeting on it errs high, which is the direction a memory guard
  should err. It is read in GiB, so the budget is larger than the recorded 3.5 GB.

**The count never drops silently.** Every call prints its reasons, including the line that lowers a
number somebody asked for, because a run that quietly proves four shards when you asked for eight is
the same defect as a gate that quietly skips.

# BUGS

- The memory figure is one harness's peak from 2026-08-14, not a per-crate profile. A crate whose
  harnesses are all small pays for the largest one's budget. Raising VERIFY_JOBS is the knob that
  spends it, and this file does not model per-crate peaks because nothing measures them per crate.
- `--memory` is a CLI fact, so a machine's physical memory is read by this file and by
  `helpers/heavy_slot.py`, each through `sysconf`. The two agree today because neither rounds; a
  future container limit that one seen and the other not would be a disagreement nothing checks.
"""

import os
import sys

GIB = 1 << 30
HARNESS_BYTES = int(3.5 * GIB)
DEFAULT_JOBS = 4


def physical_bytes():
    """This machine's physical memory, by the same `sysconf` pair helpers/heavy_slot.py uses."""
    return os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")


def gib(n):
    return "%.1f GiB" % (n / float(GIB))


def plan(shards, slots, jobs=DEFAULT_JOBS, wanted=None, memory=None, per_harness=HARNESS_BYTES):
    """(children, reasons): how many shard children to start, and the sentences explaining it.

    `shards` is CI's shard count, `slots` the machine's heavy-job slot count, `jobs` VERIFY_JOBS,
    `wanted` an explicit count from the command line or None for the default, `memory` bytes or
    None for this machine's physical memory, `per_harness` the peak one harness is budgeted at.

    Returns 0 children and the reasons when not even one child fits in memory, which the caller
    turns into a refusal: a caller that went ahead would start a run the machine cannot carry.
    """
    reasons = []
    if wanted is not None and wanted < 1:
        reasons.append("parallel: %d children is not a count; a parallel run starts at least one, "
                       "and leaving the number off sizes it for this machine" % wanted)
        return 0, reasons
    if wanted is None:
        n = min(shards, slots)
        reasons.append("parallel: %d children, the smaller of CI's %d shards and this machine's %d "
                       "heavy-job slots" % (n, shards, slots))
    else:
        n = wanted
        reasons.append("parallel: %d children asked for" % n)
        if n > shards:
            reasons.append("parallel: CI packs %d shards; a child past %d proves a smaller slice of "
                           "the same table and buys nothing, because the floor crate is atomic"
                           % (shards, shards))
        if n > slots:
            reasons.append("parallel: %d children on %d heavy-job slots; they all run inside the one "
                           "slot this parent holds, so the slots do not cap an explicit count and "
                           "memory still does" % (n, slots))
    if memory is None:
        memory = physical_bytes()
    need = jobs * per_harness
    fit = memory // need
    if fit < 1:
        reasons.append("parallel: one child needs VERIFY_JOBS=%d x %s per harness = %s, and this "
                       "machine has %s; no child fits, so lower VERIFY_JOBS"
                       % (jobs, gib(per_harness), gib(need), gib(memory)))
        return 0, reasons
    if n > fit:
        reasons.append("parallel: lowered from %d to %d children: %d x VERIFY_JOBS=%d x %s per "
                       "harness is %s of %s"
                       % (n, fit, n, jobs, gib(per_harness), gib(n * need), gib(memory)))
        n = fit
    else:
        reasons.append("parallel: %d x VERIFY_JOBS=%d harness(es) at %s each is %s of %s"
                       % (n, jobs, gib(per_harness), gib(n * need), gib(memory)))
    if n == 1:
        reasons.append("parallel: one child is this file's serial run; the flag buys nothing at this size")
    return n, reasons


def selftest():
    """The sizing rule against fixture slot and memory counts. No cargo, no Kani, milliseconds."""
    ok = True

    def expect(label, got, want):
        nonlocal ok
        if got != want:
            ok = False
            print("FAIL %s: got %r want %r" % (label, got, want), file=sys.stderr)

    big = 128 * GIB
    small = 16 * GIB
    # The measurement's host: four shards, sixteen slots, four jobs, 128 GiB. Nothing binds but the
    # shard count, and the memory the four children budget is well under the machine.
    expect("four shards on a big host", plan(4, 16, 4, None, big)[0], 4)
    # A small machine caps the default even when CI and the slots would allow more.
    expect("slots bind on a two-slot machine", plan(4, 2, 4, None, big)[0], 2)
    expect("shards bind on a big machine", plan(3, 16, 1, None, big)[0], 3)
    # 16 GiB carries one child at VERIFY_JOBS=4 and four at VERIFY_JOBS=1.
    expect("memory lowers the default from 4 to 1", plan(4, 8, 4, None, small)[0], 1)
    expect("memory carries four children at one job each", plan(4, 8, 1, None, small)[0], 4)
    # An explicit count is lowered too, out loud, and never silently.
    n, reasons = plan(4, 16, 4, wanted=8, memory=small)
    expect("an explicit eight is lowered to one", n, 1)
    expect("the lowering says what it lowered", any("lowered from 8 to 1" in r for r in reasons), True)
    # Above CI's shard count it still runs, and says the extra children buy nothing.
    n, reasons = plan(4, 16, 4, wanted=6, memory=big)
    expect("an explicit six is honored", n, 6)
    expect("the pointlessness is printed", any("buys nothing" in r for r in reasons), True)
    # Above the slot count is allowed and explained; memory is the remaining bound.
    n, reasons = plan(4, 2, 4, wanted=3, memory=big)
    expect("three children on two slots", n, 3)
    expect("the slot line is printed", any("heavy-job slots" in r for r in reasons), True)
    # Nothing fits: refused rather than started.
    n, reasons = plan(4, 8, 4, None, 8 * GIB)
    expect("8 GiB at four jobs fits no child", n, 0)
    expect("the refusal says how to fix it", any("lower VERIFY_JOBS" in r for r in reasons), True)
    expect("zero children is not a count", plan(4, 8, 4, wanted=0, memory=big)[0], 0)
    # Every call explains itself, including the unremarkable ones.
    for label, args in [("default", (4, 16, 4, None, big)), ("wanted", (4, 16, 4, 6, big)),
                        ("refused", (4, 16, 4, None, GIB))]:
        expect("reasons are never empty (%s)" % label, bool(plan(*args)[1]), True)
    # The live figure is the recorded peak, in GiB so the budget is not under the measurement.
    expect("3.5 GiB, not less", HARNESS_BYTES >= int(3.5 * 1000 ** 3), True)
    expect("physical memory plausible", physical_bytes() > 1 * GIB, True)

    print("verify_parallel selftest: %s" % ("ok" if ok else "FAILED"))
    return 0 if ok else 1


def main(argv):
    if argv[1:] == ["--selftest"]:
        return selftest()
    if len(argv) < 2 or argv[1] != "plan":
        print(__doc__, file=sys.stderr)
        return 2
    import argparse
    p = argparse.ArgumentParser(prog="verify_parallel.py plan")
    p.add_argument("--shards", type=int, required=True)
    p.add_argument("--slots", type=int, required=True)
    p.add_argument("--jobs", type=int, default=DEFAULT_JOBS)
    p.add_argument("--wanted", type=int, default=None)
    p.add_argument("--memory", default="auto")
    args = p.parse_args(argv[2:])
    try:
        memory = None if args.memory == "auto" else int(args.memory)
    except ValueError:
        sys.exit("verify_parallel: --memory wants bytes or auto, got %r" % args.memory)
    n, reasons = plan(args.shards, args.slots, args.jobs, args.wanted, memory)
    for r in reasons:
        print(r, file=sys.stderr)
    if n < 1:
        return 2
    print(n)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
