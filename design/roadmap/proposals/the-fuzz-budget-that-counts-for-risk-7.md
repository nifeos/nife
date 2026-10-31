---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The fuzz budget that counts for risk 7's criterion (d)

Raised by lane/confined-fuzz-budget on 2026-10-31 (UTC), asked to set the bar that criterion (d)
of risk 7 (the confinement claim is false) names and nobody has set. It holds three questions for an
architect, one at a time, and a short measurement. It rules nothing and runs nothing at scale.

Reuse: the budget run reuses the three fuzzers and the weekly sweep jobs as they stand. Milestone
779's proposal considered syzkaller and lost it for a crash-only oracle.

## Where criterion (d) stands

Risk 7 is AMBER. The architect agreed the four criteria for green on 2026-10-06 (UTC). Criterion (d) reads:
milestone 779 (fuzz the surface a confined process can reach), "run to a set
budget with no escape." Re-derived from the tree and the workflow logs at `9f62334f5`:

- Milestone 779 reads BUILT, and it holds three fuzzers, not one. Part (a) is three host
  `cargo-fuzz` targets over service request handlers (milestone 751 (fuzz the services' request handlers): `redoxfs_server_session`,
  `system_log_session`, `compositor_session`). Part (b) is two in-guest drivers: the kernel-thread
  driver of milestone 752 (a seeded syscall driver with a shadow model) and the EL0 driver of milestone 779 itself
  (`fixtures/src/confined_syscall_fuzzer.rs`, judged by `confined_fuzzer_tests`).
- No budget exists. Nothing in `design/`, `notes/` or the workflows names a count, a time or a
  stopping rule for (d).
- No run counts toward it yet. The suite runs 34 fixed seeds of the EL0 driver on every merge.
  The weekly `falsifications.yml` sweep runs each driver for 60 s of guest time on one hosted
  aarch64 runner. Its last three EL0 sweeps: 2026-10-12 red (seed `202642000408`, step 80) and
  2026-10-19 red (seed `202643000066`, step 34), both on the oracle's missing `SLICE` arm, which the
  kernel was right about; 2026-10-26 clean, 801 seeds at 13 seeds/s. The kernel-thread driver's
  2026-10-26 sweep was clean, 2,450 seeds at 40 seeds/s.
- At the weekly rate, any meaningful budget takes years. 801 seeds a week is about 42,000 a
  year.
- Criterion (d) does not bind today. Criterion (c)'s count of clean outsider passes is zero:
  the tenth pass booted an escape on claim 11 on 2026-10-27 (UTC), and its fix, milestone 969 (a `MAP_INTO` refuses a frame mapped twice with different flags), is
  NOT-STARTED. But (d) needs its bar before (c) completes, or green waits on a question nobody asked.

## What the fuzzers do, in plain words

A confined process is a program the kernel holds to a fixed set of permissions, called
capabilities: this mailbox, that page of memory, nothing else. The confinement claim is that no
sequence of system calls lets it reach anything outside that set. The 36 rows of
[`notes/confinement-claims.md`](../../../notes/confinement-claims.md) each state one such rule.

The EL0 driver tests the claim by brute force. It starts three small user programs, two with a
handful of capabilities and one, the witness, with none it may use. A random number generator,
started from a number called the seed, picks each program's next system call: which capability,
which operation, which arguments. After every call a referee in the kernel checks four things.
Every new entry in a program's capability table came from a call allowed to grant it. No page
table maps a page the program never received. Every badge and register the kernel handed back
tells the truth. And the kernel refused the witness every time. One seed is 96 calls. The same seed
always replays the same calls, so a failure names its seed, and anyone can rerun it.

The kernel-thread driver is the same idea one layer in: kernel threads stand in for programs, and a
model predicts the exact answer to every call. The host targets are classic coverage-guided
fuzzing of a server's message handling, millions of inputs a minute on an ordinary machine.

The point that shapes every decision below: the in-guest drivers are **coverage-blind**. They
cannot tell which kernel code a seed reached, so they never learn, and "run until coverage stops
growing" has nothing to read. Each seed is an independent random draw. That makes the drivers
weaker than a coverage-guided fuzzer at finding deep bugs, and it makes their clean runs easy to
reason about statistically.

## Measured today

Run by this lane on 2026-10-31 (UTC), one QEMU at a time, on a shared many-core x86_64 build host
under TCG (full emulation, no hardware acceleration for aarch64), with other lanes' load on it.
Not a census.

| what | inputs | wall time | rate | result |
|---|---|---|---|---|
| EL0 driver, aarch64, seeds `900000000..900000277` | 278 seeds | 60,047 ms | 4 seeds/s | clean |
| Kernel-thread driver, aarch64, seeds from `900000000` | 542 seeds, 24,066 calls | 60,043 ms | 9 seeds/s | clean |
| EL0 driver, seed `202642000408` (the 2026-10-12 red) | 1 seed | 444 ms | | green |
| `redoxfs_server_session`, empty corpus | 103,159 sessions | 180 s | 569/s | no crash |

For comparison, the hosted runner's 2026-10-26 sweeps ran the same two drivers at 13 and 40
seeds/s, three to four times faster. This lane did not establish why. A cold build took 8 min 9 s for 60 s of fuzzing.

The replayed seed matters. The 2026-10-12 red never entered the corpus, and nothing recorded
whether the `SLICE` fix also covered it. It does, so both weekly reds were the referee's, and the
EL0 driver's record holds no open red.

The host target's coverage, in libFuzzer edges: 772 at start, 2,024 after 8,192 sessions, 2,453
after 32,768, 2,660 after 65,536, and 2,785 at the end. It gained about 5% per doubling of inputs
near the end, so three minutes reach no plateau.

Nobody can measure coverage growth for the drivers today: neither carries a coverage counter, and adding
one is a code change, out of this lane's scope. The host targets do carry one (libFuzzer's edge
count), reported above.

## Decision 1: what budget makes a clean run count, and on how many architectures

**Recommendation: a fixed count of fresh seeds per driver per architecture, 300,000 for each
in-guest driver on each of aarch64, riscv64 and x86_64, plus 24 CPU-hours per host target with no
new coverage edge in its last quarter.** Read through the rule of three, 300,000 clean independent
seeds bound the chance that one random seed escapes below 1 in 100,000, at 95% confidence.

Background. A budget can take four forms. CPU hours depend on the machine: the same 60 s gives
13 seeds/s on a hosted runner and 4 on the build host this lane used, so an hour means
different things on different days. A coverage plateau needs coverage, which the drivers lack. A
per-claim corpus (so many seeds that reach each row) needs a map from seed to claim that does not
exist. A seed count is the one unit that is machine-independent, replayable and already printed by
every run.

For a generator that draws independent seeds, the count has a clean meaning. If N seeds all pass,
the 95% upper bound on the per-seed failure rate is about 3/N (the rule of three, `-ln 0.05 ≈ 3`).
The deepest planted defect either driver has found took seed 559 (milestone 752, revocation in
flight); the EL0 driver's deepest took seed 52. 300,000 is more than 500 times the deepest known
plant. It bounds only what the generator can draw: the EL0 driver's `BUGS` lists what it never
draws (its `GO` channel, threads, timers, the exits), and two arrival orders it cannot stage.

The options:

- (A) Seeds per driver per architecture, all three ISAs. Recommended. Parity is a gate in this
  tree (§19 (architectural parity is a tenet)). The escapes of record split by architecture. §12
  (call/reply IPC: a one-shot reply capability) was false only on x86_64, and so was the read-only
  port range; claim 11's alias was on all three.
- (B) The same count on aarch64 only, with the 34 suite seeds on the others. A third of the cost.
  The weekly sweep's own comment argues this: all three share one dispatcher, so breadth is about seeds,
  not ISA. Lost because the defects that were ISA-specific sat below the dispatcher, in exactly the
  code a shared-dispatcher argument does not cover.
- (C) CPU hours. Lost: the unit moves with the machine, and no run compares with the next.
- (D) Coverage plateau for the drivers. Lost for now: it needs coverage, which is a separate
  build (QEMU's TCG plugins or a kernel counter). Worth a milestone if a clean budget run ever
  must show that it reached a code path.

For the host targets, coverage exists and the convention outside is time: 24 hours a trial in
Klees et al. Rule: 24 CPU-hours each, counted clean if no crash and no new edge in the last six
hours.

Prior art. OSS-Fuzz never stops: ClusterFuzz fuzzes every target continuously, and its pull
request check, CIFuzz, defaults to 600 s and says the time "should be at minimum 600 seconds and
scale with project size" (read 2026-10-31). syzkaller also has no stopping rule; syzbot runs
continuously on each tree, and a run's value is its coverage and reproducers (from memory, not
read for this proposal). Its Fuchsia port's documentation calls it incomplete, with "coverage feedback is
not supported" (read). seL4 has no syscall fuzzer that a search found; its assurance is the proof.
Klees et al., "Evaluating Fuzz Testing" (CCS 2018), recommend 24-hour runs and repeated trials
(read in summary). Böhme's species-discovery work bounds residual risk by the share of behaviors seen
exactly once (Good-Turing), and his 2021 paper finds that bound optimistic for greybox fuzzers because they
adapt (read in summary). That adaptation bias does not apply to a coverage-blind driver, which is
why the rule of three fits here and would not fit libFuzzer.

Where nife differs. Every system above fuzzes forever and treats a bug as the output. Criterion
(d) asks the opposite question: when has enough clean fuzzing happened to support a claim? That is
a stopping rule, and only the coverage-blind, seed-replayable shape of the drivers lets it carry a
number. The rule of three is not prior art from a kernel; it is plain statistics, and this proposal
recommends it because the drivers already satisfy its one assumption, independent draws.

Example. The EL0 driver at `<commit>` runs seeds `G..G+300,000` on each ISA, where `G` is a
published base. All 900,000 pass. The record states: no escape in 300,000 seeds per ISA, so the
per-seed escape rate is below 1 in 100,000 at 95% confidence, for what this generator draws.

## Decision 2: what counts as an escape, a clean result, and a reset

**Recommendation: only a referee verdict confirmed as the kernel's fault is an escape. A kernel
panic, hang or unexplained failure makes the run not clean until triage explains it. A wrong referee
pauses the run without resetting it.**

Background. A run can go red for three different reasons, and the record already has each. On
2026-10-06 the EL0 driver crashed the kernel (`AddressSpace::LIST` followed a caller's pointer): a
crash, and one that could have been a read of kernel memory. On 2026-10-12 and 2026-10-19 it
reported escapes that were the referee's own missing rule. And on 2026-10-11 a fuzzer had no
address space after parking, cause unknown (milestone 779's `BUGS`). Each needs a different
answer.

The rule recommended:

1. Escape. The referee reports a reach beyond the grant, and triage shows the kernel at fault.
   The count resets to zero for every driver, and (d) needs the full budget again after the fix.
2. A crash or hang reached from user mode. Not clean. It is a denial of service at least, and the
   `LIST` case shows a crash can sit one argument away from a read. It resets the count unless
   triage shows the fault was in the test scaffolding, not the kernel.
3. A wrong referee. The kernel was right, and the referee lacked a rule (the `SLICE` case). The
   run pauses while a lane fixes the referee. A fix that only admits more keeps every earlier seed, since
   none of them failed the stricter rule. A fix that rejects more re-runs the seeds it could change.
4. Unexplained. Treated as (2) until explained. The 2026-10-11 failure is the template.

What does not reset: an ordinary kernel change. Main moves daily, and a count that reset on
every commit would never finish. The budget runs at one pinned commit, like an outsider pass. It
reruns in full when the syscall surface changes (a new method or syscall, which already needs a
`design/decisions/` section), and the weekly sweep keeps running afterward, where any escape reopens
(d).

A clean run speaks only for the rules the referee checks. The tenth pass's escape, a page mapped
writable at one address and executable at another, is invisible to it: both mappings use a frame
the program holds. A budget record should list which claims rows the referee judges.

Options: (A) as above, recommended. (B) Count only referee escapes and ignore crashes. Lost, for
the `LIST` reason. (C) Reset on any kernel change in `kernel/src/syscall.rs` or `sched.rs`. Lost:
those files change most weeks, so (d) would rarely hold.

Prior art. OSS-Fuzz deduplicates crashes by stack and checks daily whether each still reproduces,
closing it when it does not (from memory). syzbot closes a bug when a commit naming it lands and the
crash stops, and treats a crash with no reproducer differently from one with (from memory). Both
count crashes, because their oracles are sanitizers and panics. nife's drivers have a semantic
oracle, so nife needs the referee-defect category the others do not.

Example. A sweep reports seed `S` step 34 filling a slot with a `PageFrame`. Triage finds `SLICE`
legal. Category 3: the referee gains its arm, seeds before `S` keep their verdicts, and the sweep
resumes at `S`.

## Decision 3: where and when it runs, and the cost

**Recommendation: on GitHub's hosted aarch64 runners, dispatched by hand once per budget, at
most six jobs at a time, over two consecutive 08:00 to 13:00 UTC windows.**

Background. Two kinds of machine could run it. Hosted Actions minutes cost this public
repository nothing (§214 (the Team plan buys runner concurrency)). But every job takes one of the 60
slots the merge queue also uses. An owned build host has idle cores. §299 (the build machine is bought) leaves open whether any workflow runs on it, and a job there needs a self-hosted runner. The drivers run under full
emulation on an x86_64 host and under emulation for riscv64 and x86_64 on an aarch64 runner too.

The cost, sized from the hosted rates, since that is the machine recommended. At 13 seeds/s, the
EL0 driver's 300,000 seeds take 6.4 runner-hours on aarch64; the kernel-thread driver's take 2.1 at
40 seeds/s. No sweep has ever run riscv64 or x86_64 on a hosted runner, so nobody has measured those
rates. If the suite's 2026-10-06 ratios hold there (21 and 36 seeds/s against 32 on
aarch64), riscv64 takes about 9.8 hours and x86_64 about 5.7 for the EL0 driver. That is an
estimate, and the first job of the budget run measures it. The drivers total about 30
runner-hours, and the three host targets add 18 more on a four-core runner (core count from
memory). About 48 runner-hours, at $0 cash. A hosted job stops at six hours (from memory), so
each driver shards at about 50,000 seeds a job. Six jobs at a time is 10% of the pool, for about
eight hours: two night windows, when the queue is quietest.

Options: (A) hosted runners, recommended: free, already wired, and the budget is a one-off of a
few runner-days. (B) The owned build host, at low concurrency in the night window. It needs §299's
open runner question answered first, and a self-hosted runner never takes a fork's pull request.
(C) Raise the weekly sweep's cap until the budget accrues. Lost: it needs months, and a budget that
spans many commits answers Decision 2's pinning poorly.

Prior art. ClusterFuzz runs on a pool sized to its project count, and syzbot runs a fixed set of VMs
per kernel tree (from memory). Neither has a window; nife's matters because the merge queue shares
its pool by day.

Example. A maintainer dispatches the budget at a pinned commit at 08:00 UTC. The first window runs
aarch64 and x86_64 for both drivers, six shards at a time. The second runs riscv64 and the host
targets. The record lands on the risk 7 page as a dated fact: the commit, each seed range, each
rate, and the verdict.

## The seven questions

1. Considered and lost: above, per decision.
2. In this tree: the outsider passes pin a commit and a floor of booted rows; this copies the pin.
   The sweeps print seeds and rates.
3. Prior art: above. Read where marked; the rest says from memory.
4. The premise holds: nothing sets a budget, and no run against (d) exists.
5. Cost: measured above.
6. Reversible: a budget is a record, not a contract, and nobody has acted on one. Changing it
   later moves the bar, which is the architect's call.
7. At equal cost, this proposal would still choose three ISAs. It recommends the hosted runner
   partly because it costs nothing; that part is about cost, and says so.

## What waits on this

Criterion (d), and so risk 7's green, until an architect answers Decision 1. A lane then writes the budget
into the risk 7 page and runs it.

## What I need from you

Decision 1 first, since its answer sizes 2 and 3. If the answer is no to seeds, the lane needs the
unit chosen instead; (d) stays unmeasurable until then.
