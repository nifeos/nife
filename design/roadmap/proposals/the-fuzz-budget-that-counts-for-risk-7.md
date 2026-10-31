---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Continuous fuzzing for risk 7's criterion (d)

Raised by lane/confined-fuzz-budget on 2026-10-31 (UTC), asked to set the bar that criterion (d)
of risk 7 (the confinement claim is false) names and nobody has set. An architect ruled on
2026-10-31 (UTC) on the standard and on question 1, and this document states those rulings. Four
questions remain open, and I ask them one at a time. The file name keeps its first form so that
links to it hold.

Reuse: the continuous run reuses the three fuzzers and the weekly sweep jobs as they stand. Milestone
779's proposal considered syzkaller and lost it for a crash-only oracle; this document adopts its
service's standards, not its code.

## What is in force

- nife adopts the same standards as OSS-Fuzz and syzbot (syzkaller's continuous service) for the
  confinement fuzzers. Decided 2026-10-31 (UTC).
- That replaces the earlier idea of a fixed seed count or a fixed number of CPU-hours. The fuzzers
  run continuously, and those two projects' standards, set out below, bound what they claim.
- Criterion (d) of risk 7 is the four conditions below, all of which must hold on a date. Decided
  2026-10-31 (UTC).
  1. The fuzzer runs now, on the architectures claimed.
  2. A measure of its coverage exists, and coverage has stopped growing or the growth has an explanation.
  3. Every finding has a fix or sits inside the triage clock, and none is older.
  4. The corpus and the findings live where others can see them.
- The rating follows what is true on that date, and it can rise again if a condition lapses.
- A 14-day run is at most a release smoke test. It does not count toward the rating.
- Both in-guest drivers get compiler-inserted coverage counters (LLVM sanitizer-coverage inline
  8-bit counters on the kernel code). The kernel supplies a counter region and a way for the driver
  to read it after each seed. One option serves all three architectures and runs under KVM. The
  counters measure coverage first and may later steer the fuzzer. Decided 2026-10-31 (UTC).
  Not chosen: QEMU-side block tracing, which needs full emulation and runs 20 to 29 times slower,
  and hand-placed counters.
- The first work fixes the harness false reds: guests that fail on harness deadlines under load,
  where every failing seed replays green. It comes before any long or wide run. Decided 2026-10-31
  (UTC). Not chosen: coverage first at 1 to 4 guests, and discarding failures that a replay clears.
- The second work is a build for one architecture that measures the speed cost of the coverage
  counters under KVM in the nife kernel. Nobody has measured that cost.
- Not yet in force: the host and operator (question 4), and the triage clock's length (question 5). Until an architect rules on those,
  nothing can satisfy conditions 1 to 4, so risk 7 stays AMBER and no run counts toward (d).

## Where criterion (d) stands

Risk 7 is AMBER. The architect agreed the four criteria for green on 2026-10-06 (UTC). Criterion (d)
names milestone 779 (fuzz the surface a confined process can reach) and now reads as the four
conditions above. Re-derived from the tree and the workflow logs at `9f62334f5`:

- Milestone 779 reads BUILT, and it holds three fuzzers. Part (a) is three host `cargo-fuzz`
  targets over service request handlers (milestone 751 (fuzz the services' request handlers):
  `redoxfs_server_session`, `system_log_session`, `compositor_session`). Part (b) is two in-guest
  drivers: the kernel-thread driver of milestone 752 (a seeded syscall driver with a shadow model)
  and the user-mode driver of milestone 779 itself (`fixtures/src/confined_syscall_fuzzer.rs`,
  judged by `confined_fuzzer_tests`).
- No budget exists. The suite runs 34 fixed seeds of the user-mode driver on every merge. The
  weekly `falsifications.yml` sweep runs each driver for 60 s of guest time on one hosted aarch64
  runner. Its last three user-mode sweeps: 2026-10-12 red (seed `202642000408`, step 80) and
  2026-10-19 red (seed `202643000066`, step 34), both on the referee's missing `SLICE` arm, which
  the kernel was right about; 2026-10-26 clean, 801 seeds at 13 seeds/s. The kernel-thread
  driver's 2026-10-26 sweep was clean, 2,450 seeds at 40 seeds/s. At the weekly rate, 801 seeds a
  week is about 42,000 a year.
- Criterion (c)'s count of clean outsider passes is zero: the tenth pass booted an escape on claim
  11 on 2026-10-27 (UTC), and its fix, milestone 969 (a `MAP_INTO` refuses a frame mapped twice
  with different flags), is NOT-STARTED.

## What the fuzzers do, in plain words

A confined process is a program the kernel holds to a fixed set of permissions, called
capabilities: this mailbox, that page of memory, nothing else. The confinement claim is that no
sequence of system calls lets it reach anything outside that set. The 36 rows of
[`notes/confinement-claims.md`](../../../notes/confinement-claims.md) each state one such rule.

The user-mode driver tests the claim by brute force. It starts three small user programs, two with
a handful of capabilities and one, the witness, with none it may use. A random number generator,
started from a number called the seed, picks each program's next system call. After every call a
referee in the kernel checks four things. Every new entry in a program's capability table came from
a call allowed to grant it. No page table maps a page the program never received. Every badge and
register the kernel handed back tells the truth. And the kernel refused the witness every time. One
seed is 96 calls. The same seed always replays the same calls, so a failure names its seed.

The kernel-thread driver is the same idea one layer in: kernel threads stand in for programs, and a
model predicts the exact answer to every call. The host targets are classic coverage-guided
fuzzing of a server's message handling.

The in-guest drivers are coverage-blind: they cannot tell which kernel code a seed reached, so they
never learn, and each seed is an independent random draw.

## The two standards, as read

Read on 2026-10-31 (UTC) from the projects' own documentation. I mark anything I did not read
unverified.

### OSS-Fuzz

Read from google.github.io/oss-fuzz.

- Integration, the "ideal integration" page. Fuzz targets live in the project's own repository and
  are easy to find. Each target is also run regularly "not necessarily fuzzed" as a regression test
  over its seed corpus, with sanitizers. The project keeps the seed corpus in revision control and regularly
  extends it with inputs that triggered bugs or reached new code, ideally a minimal set with maximal
  coverage. Targets must have good coverage, be fast and be memory-efficient. Read.
- Disclosure and fix deadline. OSS-Fuzz opens a reported issue to the public in 90 days, or after the fix ships, whichever is earlier. A deadline that falls on a weekend moves to the next work
  day. If the project says before the deadline that a patch lands within 14 days after it,
  disclosure waits for the patch. Read. The page says nothing of a 30-day period after a fix.
- Reproducers. Each report carries the bytes the fuzzer fed to the target. Rerunning that input checks a fix; after the fix lands, ClusterFuzz reruns the input and closes the issue,
  stated as "in < 1 day." Read.
- Coverage reports. Produced with Clang source-based coverage by running the targets over the
  corpus OSS-Fuzz has aggregated; the pages call coverage reports public. The page does not state how often OSS-Fuzz
  publishes them on the page read. Partly read.
- Corpus. Kept in cloud storage with daily zipped backups. Pruning is not described on the page
  read. Read, with that gap.
- CIFuzz. Runs on each pull request for 600 s by default, which "should be at minimum 600 seconds
  and scale with project size." It uses 30-day-old public regressions and corpora from OSS-Fuzz.
  A crash fails the check only if it reproduces on the pull request's build and does not occur on
  older OSS-Fuzz builds, so a pull request is not blamed for an older bug. Read.
- Fuzz Introspector. Reports reachability and coverage per fuzzer and in aggregate, flags blockers
  that limit what a fuzzer can reach, and suggests new targets. The service generates and uploads reports
  automatically. Read.
- Unverified: how ClusterFuzz deduplicates crashes, how it finds the range of commits that
  introduced a crash, and any reminder or auto-close schedule for stale issues.

### syzbot and syzkaller

Read from the syzkaller repository's `docs/syzbot.md`, `docs/coverage.md` and
`docs/syzbot_assets.md`, and from the syzbot dashboard page.

- Dashboard. Shows open, fixed and invalid bug counts, a per-manager table (uptime, corpus size,
  coverage, crashes, executions, kernel build, freshness), and coverage views for the total, a
  repository heat map and a subsystem heat map. Each bug shows the reproducer type, cause and fix
  bisection status, crash count, time since the last crash and time since the report. Read from
  the first part of the page only; the rest of the page was not read.
- Reproducers. The aim is a stand-alone C reproducer for every reported bug; some have only the
  syzkaller program, which converts to C. Bugs without a reproducer first wait in a moderation
  queue. Read.
- Bisection. syzbot bisects bugs with a reproducer to the commit that introduced them; one crash
  marks a revision bad. A separate fix bisection runs only when the bug has not occurred for at
  least 30 days. Read.
- Fix tracking. A person attaches a fix by naming the exact commit title; syzbot closes the bug
  only when the commit reaches all tracked builds, and a similar crash after that opens a new bug.
  While a bug is open, similar crashes merge into it. A person marks a one-off report invalid, and a
  recurrence creates a new report. Read.
- Retention. syzbot keeps build assets until 30 days after the bug becomes fixed or invalid. Read.
- Coverage. Collected by KCOV, in basic blocks or edges, with the warning that coverage "may poorly
  relate to the source code"; the web report colors lines as hit, partly hit, uncovered in a run
  function, or in a function that never ran. Read.
- Unverified: how syzbot ages open bugs, whether syzbot closes a bug with no crash for a long period
  automatically (the pages read do not say), where syzbot publishes the corpus, and what syzbot claims
  after a long quiet stretch. The pages read make no such claim.

## What nife already meets, and what it lacks

| standard | nife today |
|---|---|
| Replayable inputs | Met. A seed replays the same calls; a failure names its seed. |
| Determinism across accelerators | Met. The same seed gives the same verdict under hardware acceleration and under emulation. |
| An oracle beyond a crash | Met, and stronger than the sanitizers the others rely on. The referee reports capability, badge and oracle violations. |
| Targets in the project's own tree | Met. |
| Targets run regularly as regression | Partly met. 34 fixed seeds run on every merge. Nothing keeps a failing seed as a named regression test. |
| Coverage measure | Missing for the two in-guest drivers, which carry no coverage counter. The host targets have libFuzzer's edge count. |
| A usable verdict under concurrency | Missing. Several guests at once produce false reds (see the appendix). |
| A corpus kept across runs | Missing. No corpus of seeds or inputs survives a run. |
| Reproducer to regression test | Missing. A failing seed is not turned into a standing test by any step. |
| A triage clock | Missing. No rule gives the time allowed to examine a finding or the age at which an open escape becomes a failure. |
| A dashboard | Missing. Counts, rates and open findings are not collected anywhere. |
| Bisection and fix tracking | Missing. A finding is not tied to an introducing or a fixing commit. |
| Introspection of what is unreachable | Missing. |

## Scope note (§19)

x86_64 only for now. A dedicated x86_64 virtual machine of about 4 vCPUs and 8 GB runs x86_64
comfortably under hardware acceleration. aarch64 and riscv64 run only under full emulation (TCG)
and I have not measured them for continuous work. §19 (architectural parity is a tenet) makes parity a gate,
so this is a scoped exception for the fuzzers only. It ends when hardware
runs the fuzzers on the other two architectures at a useful rate. The escapes of record split by
architecture. §12 (call/reply IPC: a one-shot reply capability) was false only on x86_64, and claim
11's alias was on all three. So the exception costs coverage of the claim, and the record must say
so rather than imply parity.

## The questions that remain

I ask them one at a time, in this order, each after an architect rules on the one before. An
architect has ruled on questions 1 to 3; question 4 sits on the pull request. This list keeps the
rest, and each returns in full when its turn comes.

1. An architect ruled on 2026-10-31 (UTC): what criterion (d) becomes, and the 14-day run.
2. An architect ruled on 2026-10-31 (UTC): the coverage measure. See "What is in force".
3. An architect ruled on 2026-10-31 (UTC): the false-red fix is the first work.
4. Where the fuzzers run and who operates them.
5. The triage clock, which condition 3 names.

### Question 4: who runs the fuzzer

Recommendation: (A) a dedicated x86_64 virtual machine that the architect creates, which runs the
fuzzers continuously under a systemd unit, with the corpus and findings published to a place
others can read. A lane operates it through the repository's scripts, and the architect owns the
machine.

Background. Condition 1 says the fuzzer runs now, and condition 4 says the corpus and findings
live where others can see them. Something has to host both, around the clock. The appendix shows a
virtual machine of about 4 vCPUs and 8 GB runs the x86_64 drivers comfortably under KVM, at about
100 MB per guest. Hosted Actions runners cap a job at six hours (from memory), share the merge
queue's 60 slots. They offer no operator who owns a corpus.

Options:

- (A) A dedicated x86_64 virtual machine. Recommended.
- (B) Scheduled hosted jobs that resume from a corpus in an artifact store. Free, but never
  continuous, and it competes with the merge queue by day.
- (C) The shared build host, at low concurrency in the night window. It has idle cores, but other
  lanes' load caused the slow rates in the appendix, and a self-hosted runner never takes a fork's
  pull request.

Prior art. OSS-Fuzz runs on ClusterFuzz, a scalable fuzzing infrastructure built for it (read); its pool
size is unverified. syzbot runs named managers, listed with uptime and freshness on its dashboard (read).

Where nife differs. One machine, one architecture for now (the §19 scope note), and an owner who
is also its architect.

Example. The virtual machine runs both drivers at 4 guests, 24 hours a day. A daily job copies the
counter totals, the new corpus and any findings to the published place. On a given date the
dashboard shows the fuzzer up for 61 days, which meets condition 1.

## Appendix: measurements, 2026-10-31 (UTC)

One run by this lane on a many-core x86_64 build host, with other lanes' load on it, one QEMU
guest per process. Not a census. The per-guest logs stay off-tree; the text cites figures only.

Rate per guest, x86_64 under hardware acceleration (KVM):

| guests at once | kernel-thread driver (milestone 752), seeds/s | user-mode driver (milestone 779), seeds/s |
|---|---|---|
| 1 | about 420 | about 230 |
| 2 to 8 | 440 to 460 | about 220 |

Baseline under full emulation (TCG): 20 seeds/s for the kernel-thread driver and 8 for the
user-mode driver. Hardware acceleration is about 20 times faster for the first and about 28 times
for the second. These are also many times the hosted aarch64 sweep rates quoted above (40 and 13),
which run under emulation.

Memory: about 100 MB per guest.

Host target `redoxfs_server_session`: 794 runs/s on one process and about 5,470 runs/s on eight
workers.

False reds, guests that reported a failure that did not reproduce:

| guests at once | kernel-thread driver | user-mode driver |
|---|---|---|
| 4 | 1 of 4 | 0 of 4 |
| 8 | 4 of 8 | 1 of 8 |
| 16 | 6 of 16 | 6 of 16 |

Replays of failing seeds were green. The failures were the harness's deadlines expiring under load,
not escapes. Until a fix lands, more concurrency buys false reds faster than it buys seeds, which
is why question 3 exists.

## The seven questions

1. Considered and lost: question 4's options above.
2. In this tree: the weekly sweeps run on hosted runners; no machine runs the fuzzers continuously.
3. Prior art: above, with what I did not read marked.
4. The premise holds: condition 1 needs a host.
5. Cost: one virtual machine, about 4 vCPUs and 8 GB, and its operator's time.
6. Reversible: a machine can move; the published place is the lasting part.
7. At equal cost this would still choose (A); the recommendation is not about effort.

## What waits on this

Conditions 1 to 4 of criterion (d), and so risk 7's green, until an architect rules on questions 4
and 5. The coverage build for one architecture can start now.

## What I need from you

Question 4 only. Question 5 waits.
