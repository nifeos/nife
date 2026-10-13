---
status: BUILT
raised: 2026-09-03
built: 2026-09-12
---
# 277. Bound what one mutant may allocate, so a runaway kills the mutant and not the machine

Built (2026-09-12). Written by the milestone 247 sweep as a proposal on 2026-09-03, from
milestone 238's block. Promoted out of the proposal queue on 2026-09-11 by calef, who asked for the
oldest thing in the proposal queue: it was in the founding batch of 46 written the day the proposals
mechanism was ratified, and it had sat eight days while the workflow it repairs failed every
scheduled run. *(Number provisional until the merge queue lands it; 275 and 276 are in flight ahead
of it.)*

This was ungated, and it stayed ungated. Three shapes were priced in milestone 238's block and
choosing between them was engineering rather than a decision owed to calef, all three being a
wrapper around an existing command and all three reversible. Nothing found while building it moved
that: the one name minted is provisional like any lane's, and no syscall surface, dependency or wire
format was touched.

In brief. `script/mutation` gives each mutant a per-mutant timeout of 28 to 51 seconds. That
bound is on time, and the failure that actually occurs is on memory: one mutant goes from
1.4 GB to 15.8 GB in twenty seconds and takes the whole runner agent with it, comfortably inside
the timeout that therefore never fires. The work is to add a memory bound per mutant so the mutant
dies and the sweep continues.

## Why this matters

The mutation sweep is the refresh for `design/fatal-risks/README.md`'s third risk, and it has a history of
not producing results: four scheduled runs, four failures, zero reports before milestone 238. A run
that dies because one mutant ate the machine is that same outcome with a new cause, and it is worse
than a timeout because it takes the runner with it rather than reporting a single skip.

The measured shape of the failure is what makes this cheap and worth doing. It is not gradual
pressure; it is one process going ten times its size inside twenty seconds. A hard ceiling turns a
run-ending event into a one-line entry in a report, and a mutant that cannot be evaluated within a
sane memory budget is itself a useful thing to have recorded.

It also has a second victim this tree has already paid for. AGENTS.md records that concurrent heavy
jobs are bounded by memory rather than by the collision surface, and that a day of out-of-memory
kills cost a session, a confusing `ci-build` timing failure, and two `script/verify` runs killed
mid-CBMC. The tell is always the same and is always misread: a heavy job dying with no failing
assertion. An unbounded mutant is one more source of exactly that.

## The three shapes, from milestone 238's own pricing

- A memory cgroup via `systemd-run --scope`. The strongest bound and the most Linux-specific.
  Kills reliably at the limit, needs the runner to be Linux with cgroup v2, and does nothing on the
  dev Mac where `script/mutation` is also run by hand.
- `ulimit -v` ahead of `script/mutation`. Portable and one line, and it bounds address space
  rather than resident memory, which over-counts for anything that reserves generously. It applies
  to the whole sweep process tree rather than per mutant unless the wrapper re-applies it.
- A `cargo` runner wrapper. Applies per test binary, which is the exact granularity wanted, at
  the cost of a small program in the tree that every mutation run then depends on.

Choosing is the work, and the choice should say what happens on the dev Mac as well as on the
runner, because both are places this sweep gets run.

## Where it came from

Milestone 238's `## Follow-on`: *"Bound what one mutant may allocate, so a runaway allocation kills
the mutant instead of the machine. Three shapes are priced in this block (a memory cgroup via
`systemd-run --scope`, `ulimit -v` ahead of `script/mutation`, or a `cargo` runner wrapper) and
choosing between them is the work. Today one mutant goes 1.4 GB to 15.8 GB in twenty seconds and
takes the runner agent with it, inside the 28-to-51-second per-mutant timeout that therefore cannot
catch it."*

## Built: a cargo runner, and the first thing checked was whether this was needed at all

**cargo-mutants 27.1.0 has no memory bound.** `--timeout`, `--build-timeout`, a multiplier for each
and `--minimum-test-timeout` are the complete set of limits it offers, read off `cargo mutants
--help` and confirmed against its own argument definitions. The three shapes below were priced
without anyone checking for a fourth, and a flag would have made all three moot, so it was worth the
five minutes. Every bound the tool has is a clock, and a clock is precisely what this failure walks
past.

Chosen: the cargo runner wrapper, as `helpers/memory-bounded-runner.sh` (**name provisional**,
as every lane-minted name is). cargo runs each test binary through `target.<triple>.runner`, and
that is the only point in the pipeline that sees exactly one test binary and neither rustc nor the
other `-j 2` job's binary, which is the granularity a per-mutant bound needs by definition.
`script/mutation` exports `CARGO_TARGET_<HOST>_RUNNER` for the length of a run and nothing else in
the tree does, so an ordinary `cargo test` is untouched. The runner sets `RLIMIT_AS` and execs.

Why the other two lost, with reasons rather than a list.

- `systemd-run --scope` lost on granularity before it lost on portability. A scope around the
  sweep bounds the *sweep*, so the 15.8 GB mutant would be killed by taking its innocent neighbor
  and the `-j 2` sibling with it, and the run would still end. Getting per-mutant granularity out of
  it means spawning a scope per test binary, which is a cargo runner with a heavier dependency
  (cgroup v2, a session bus, and a root-or-delegation question on a hosted runner) bolted inside it.
  The portability objection is real and is the smaller one.
- `ulimit -v` ahead of `script/mutation` lost for the same reason, and the milestone block
  already said so: it applies to the whole process tree, so two jobs share one ceiling and a
  legitimate build is inside it. It also bounds rustc and the linker, which legitimately want a lot
  of address space, so the number would have to be set by the build rather than by the tests.
  Note that the mechanism it proposes is the one that won: `ulimit -v` is right, and it is *where*
  it is applied that was wrong.

The number is 4 GiB and it is measured from both ends, which the block asked for and which is
the part worth keeping. The largest of this tree's 143 host test binaries peaks at 1,028 MiB
(`board_console`), the mean is 169 MiB, so the ceiling is 4.0x the largest honest binary; and `-j 2`
means two binaries can be resident at once, so 2 x 4 GiB is survivable on a 16 GiB box where twice a
larger ceiling would not be. Lowering it until it bites confirms the other end: unaffected at 4 GiB
and at 1 GiB, and at 256 MiB `board_console` fails exactly where its measured peak says.

**It was watched killing something**, because a bound nobody has seen fire is decoration. A
synthetic mutant that allocates without limit dies at whatever ceiling it is given (0.4 s at
512 MiB through 8.6 s at 4 GiB, and nowhere else), and 8.6 s is inside the 20 s minimum auto
timeout, so it lands in cargo-mutants' accounting as an ordinary **caught** mutant and the sweep
continues. That accounting was the success criterion rather than the kill: `script/mutation` treats
anything but 0, 2 and 3 as a broken run, and a bound that turned a machine kill into a red run would
be the same failure with a new cause.

## What this does not do, and what it says so

**On the dev Mac the ceiling is off by default**, and the reason is not that XNU ignores it. XNU
*does* enforce `RLIMIT_AS` (`vm_map_set_size_limit`, `KERN_NO_SPACE` out of `vm_map_enter`,
inherited across exec, since at least macOS 13). What is unknown is what number is safe there, since
only Linux was measured and macOS reserves address space far more freely. Shipping it
on with a guessed number would fail every Mac sweep at its baseline; shipping it off leaves the Mac
as exposed as it was, which is the cost, stated rather than hidden. One environment variable turns
it on, and the first person to measure should write the number into the runner's header.

**The build is not bounded**, only the test binary. No build-side runaway has been observed and a
linker legitimately wants a great deal of address space, so it would need its own number.

**And the workflow has still never succeeded.** This is demonstrated and measured, not proven: the
first green scheduled run is the evidence that matters, and until one lands, `mutation.yml`'s BUGS
header keeps saying so.

## Follow-on

- **Recorded.** The dev Mac is not covered by default, and the limitation is written where a reader
  meets the feature, in the `BUGS` section of `helpers/memory-bounded-runner.sh`, with the XNU
  source that shows the kernel would enforce it and the reason a number was not shipped unmeasured.
- **Recorded.** The build is bounded by nothing; same `BUGS` section, beside it.
- **Recorded.** That the weekly workflow has still never succeeded, in the `BUGS` header of
  `.github/workflows/mutation.yml`, which keeps saying so until a green scheduled run lands.
- **Milestone 288.** Promoted 2026-09-14, which is what a `**Proposed.**` disposition becomes when
  the proposal it names gets a number. The test
  `Builder` hardcoded `e_machine: EM_AARCH64` while `EXPECTED_MACHINE` follows `cfg(target_arch)`,
  so 20 of 25 tests fail with `WrongMachine` on an x86_64 checkout. Invisible because CI is
  `ubuntu-24.04-arm` and the dev Mac is Apple Silicon. This is milestone 117's stranger-test class
  exactly, found here only because this lane measured every host test binary. Written up as its own
  proposal rather than fixed in a memory-bound lane.

## Index row

Promoted from the proposal queue 2026-09-11 by calef, the oldest thing in it. One mutant goes 1.4
GB to 15.8 GB in twenty seconds and takes the runner agent with it, inside a per-mutant timeout
that is on time and therefore never fires; the sweep that refreshes fatal risk 3 has failed every
scheduled run since. A cargo runner gives each mutant's test binary a 4 GiB address-space ceiling,
4.0x the largest of the tree's 143 host test binaries; cargo-mutants 27.1.0 has no memory option
at all, checked first.
