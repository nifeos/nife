# The gated, dated and owed rows, argued

*An appendix to [the register of measures](../register-of-measures.md). The name is provisional.*

## Gated

### The thresholds, read together

The ceilings are rows 2 through 5 and 7 and 8. Their thresholds are six different kinds of number:

- 5% is a tolerance.
- 4,096 is a hardware fact.
- 24,576 is a configuration constant.
- The high-water limits are margins over an observed maximum.
- The `unsafe impl Send`/`Sync` ceiling was set at 17, which was the tree exactly when it was
  written. The live gate in notes/unsafe-obligations.md is now 23 (corrected 2026-09-24).
- The unsafe density ceiling is a claim about the tree that was false until shortly before it was
  written.

The density ceiling is the only one that expresses a direction rather than a limit. That
distinction is what `count-at-most` exists for. Milestone 139 (drive the unsafe count down, and
cinch the ratchet behind it) lowered it from 100 to 97, then 96, then 95, then 94. Round 5 reduced
the block count further but left the truncated density, and therefore the ceiling, unchanged.
The live gate in notes/unsafe-obligations.md
is 88 on 2026-09-24. The drift was recorded by
design/roadmap/0522-a-boundary-drawn-by-dependency-not-by-path.md. See notes/unsafe-obligations.md
for the measurement behind the ceiling.

The unsafe rows did not exist when milestone 134 (the register of measures: every number this
kernel owes itself) opened. The `unsafe fn` count that would have been a third turned out to be
already derived by `script/lint`'s `==> unsafe fn contracts` check. Finding a number already tracked
is as much a result as finding one that is not.

### Row 7 mixes kernel and userspace

Row 7's density mixes kernel and userspace unsafe into one population, and nothing here gates the
split. `script/metrics` (the unsafe-census-by-trust-boundary milestone, provisional) now tracks
`unsafe_trust_kernel` and `unsafe_trust_userspace` alongside it, weekly. Neither has a ceiling of
its own. [The unsafe series](unsafe-series.md) has the numbers. It
also explains why row 7's ceiling (88, corrected 2026-09-24 from 94) answers a different question
than either half, and recommends which one a future ceiling belongs on.

## Dated

### The milestone 168 job-mix row

Milestone 168 (a multi-tasking workload benchmark: the number that would decide the event-kernel
question) was the register's first `dated` row that had never been taken. Five boots of radon on
2026-09-16 changed that. They measured the curve's shape: a knee near four tasks, then a plateau with
no decline through 32. The instrument kept the best of three repeats. Under that rule `tasks=4` was
not a number: it varied 29.4% across boots of one image.

On 2026-09-19 the instrument changed to the median of 21 repeats. It also gained a page-mapping job
and a process-creation job. Five radon boots on 2026-10-04 re-dated the row with that instrument:
every point within 1.7% across boots, 2.62x at four tasks and 2.78x at 32, no decline
([the evening's page](../job-mix/radon-2026-10-04.md)). That is step 7's first row in
`notes/job-mix.md`. `design/fatal-risks/README.md`'s risk 4 takes its verdict from it, edited by
whoever holds that file.

### The filesystem row

The filesystem row was written as the one on the customer path, and it is the clearest case in the
register for why `dated` is a finding rather than a filing. Milestone 55 (Time Machine: SMB3 with
Apple's extensions, and mDNS) was a Time Machine target the family's Macs would back up to. It was
removed on 2026-08-30, when the family's backups went to borg on cordoba, and the customer path has
been vacant since (corrected 2026-09-24). The argument holds for any customer who stores data here. A three-times regression in
sequential write would show up as a backup that used to finish overnight and now does not. A person
would report it, not CI, and nothing in this tree would have said a word.

It is `dated` because taking it needs a boot with a disk attached, which does not belong on every
push. The honest promotion is a scheduled run rather than a gate, and it wants a lane.

### The cross-OS row

Its section, now in notes/benchmarks/cross-os-primitives.md, carried the title "The first cross-OS
numbers (nife vs Linux vs macOS)" and no date at all until 2026-09-24. PR #1200 then dated it from
git: 2026-07-25, and 2026-07-26 for the spawn row. Those are the numbers a stranger is most likely
to quote back at us, because they are the comparison against Linux and macOS. A dated measurement
with no date is a `gated` row's opposite and a `dated` row's failure mode at once. Nothing in this
tree would have flagged it. The git date says when the numbers entered a commit, not which run
produced them. Re-taking them is still the honest fix: a small lane, named in milestone 134's
handoff.

### The arch row

There is deliberately no ceiling on unsafe inside `kernel/src/arch/`. Driving that number down means
either writing assembly wrong or moving it out of `arch/`, and rule 1 of AGENTS.md says arch code
belongs there. A target would be a gate pushing against the architecture. But an unmarked number in
a note is the snapshot the register is against. So `script/lint` prints it on every run: on screen
every build, asserted never. **A number with a consumer gets a relation; a number with only a reader gets printed.**

## Owed

### Which rows the PMCCFILTR_EL0 ruling touches

calef's `PMCCFILTR_EL0` ruling (A1, 2026-10-07 UTC: `0` on every board, user and kernel counted)
touches M5 and M9 on aarch64, and M12. seL4's 413 and 426 are TX1 cycle counts under whatever
filter the TX1's firmware left. If argon's firmware left `P` set, M12 needs a second nife run with
the filter matched to seL4's, labeled as such (milestone 25 (cross-OS comparison)). M6 to M8 count events
rather than cycles, and `PMCCFILTR_EL0` filters only the cycle counter. Each event counter has its
own filter in `PMEVTYPER<n>_EL0`, which will raise the same question when a driver first writes
one.

### Why Tier B is not duplicated

The Tier B measures are not duplicated into the Owed table. They already have a home that carries
each one's instrument, its prediction, and what its outcome settles:
design/roadmap/0134-the-measurements-that-decide.md. Two open kernel decisions were waiting on the
Tier A half. §95 (a hand-written IPC fastpath, and whether it can stay proven) and §96 (process
kernel or event kernel, and how to decide it) both recommend waiting for the TX1. The block's own
correction is that both over-gated, because the experiments that produce a verdict need no silicon.
Tier A's results are summarized in the register's Dated section. Tier B remains gated on the
counters and the board.

### The Owed table in full

The register keeps a shortened form of this table. These are the cells in full, as it carried them
before the split.

| measure | its instrument, checked against the tree 2026-09-19 | what is still missing |
|---|---|---|
| M5, cycles per IPC round trip | exists on all three ISAs: every tick row times `bench::cycles_per_tick` (the two halves of milestone 74 (cycle counters); milestone 309 (unhalted core cycles on `x86_64`) for x86_64) | on riscv64, nothing: radon read `250.00` on 2026-09-16, so `call_reply` is about 1,256 cycles. On aarch64, argon's session and calef's `PMCCFILTR_EL0` ruling (the-aarch64-half-of-74, decision A) before any figure is published |
| M6, I-cache misses per IPC | none | an event-counter driver: nothing programs `PMEVTYPER<n>_EL0` or an SBI PMU cache event on any ISA (aarch64's boot line now reports six event counters visible, and none is used) |
| M7, D-cache misses per IPC in the stack region | half: the per-IPC stack depth (row above) bounds the bytes, not the misses | the same event-counter driver, and for the attribution half a data-address sampler that neither the A57 nor the U74 has; expect M7 to become "misses rise with thread count" plus the depth, rather than attribution |
| M8, TLB misses per IPC | none | the same event-counter driver |
| M9, per-phase cycles across one IPC | the counter: `arch::pmu::cycles()` is readable in-kernel on all three ISAs (riscv64 configures the boot hart only) | phase stamps at trap entry, dispatch, rendezvous, switch and exit, in a build that measures nothing else; not built |
| M10 to M12 | as milestone 134's block says | unchanged |
