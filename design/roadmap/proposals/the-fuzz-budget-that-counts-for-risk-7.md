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

Reuse: the continuous run reuses the three fuzzers and the weekly sweep jobs as they stand. It adopts
syzkaller's service standards, not its code.

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
  Not chosen: QEMU-side block tracing (20 to 29 times slower) and hand-placed counters.
- The first work fixes the harness false reds: guests that fail on harness deadlines under load,
  where every failing seed replays green. It comes before any long or wide run. Decided 2026-10-31
  (UTC). Not chosen: coverage first at 1 to 4 guests, and discarding failures that a replay clears.
- The second work is a build for one architecture that measures the speed cost of the coverage
  counters under KVM in the nife kernel. Nobody has measured that cost.
- A dedicated x86_64 virtual machine, which the architect creates, runs the fuzzer continuously
  (both drivers). A lane operates it through the repository's scripts, and a daily job publishes
  counter totals, the corpus and findings where others can read them. Decided 2026-10-31 (UTC).
  Not chosen: scheduled hosted jobs, and the shared build host at night. The maintainers keep the
  machine's specification off-tree.
- Not yet in force: the triage clock's length (question 5). Until an architect rules on those,
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
comfortably under hardware acceleration. aarch64 and riscv64 run only under full emulation (TCG), at 6 to 20 seeds/s per guest. §19 (architectural parity is a tenet) makes parity a gate,
so this is a scoped exception for the fuzzers only. It ends when hardware
runs the fuzzers on the other two architectures at a useful rate. The escapes of record split by
architecture. §12 (call/reply IPC: a one-shot reply capability) was false only on x86_64, and claim
11's alias was on all three. So the record must not imply parity.

## The questions that remain

I ask them one at a time, in this order, each after an architect rules on the one before. An
architect has ruled on questions 1 to 4; question 5 sits on the pull request. This list keeps the
rest, and each returns in full when its turn comes.

1. An architect ruled on 2026-10-31 (UTC): what criterion (d) becomes, and the 14-day run.
2. An architect ruled on 2026-10-31 (UTC): the coverage measure. See "What is in force".
3. An architect ruled on 2026-10-31 (UTC): the false-red fix is the first work.
4. An architect ruled on 2026-10-31 (UTC): a dedicated x86_64 virtual machine runs the fuzzers.
5. The triage clock, which condition 3 names.

### Question 5: the triage clock

Recommendation: (A) a finding gets a first look within 2 days. An escape, or a crash or hang
reached from user mode, gets its fix within 14 days. Any other finding gets its fix within 90 days. A
finding past its limit makes condition 3 fail, and the rating falls until it closes.

Background. Condition 3 says every finding has a fix or sits inside the triage clock, and none is older.
The clock needs three limits: how soon someone must look, how soon the worst findings need a fix,
and how soon the rest do. The limit for escapes matters most, because an open escape is the claim
being false. An unexplained finding counts as a crash until someone explains it.

Options:

- (A) 2 days to triage, 14 days to fix an escape or crash, 90 days for the rest. Recommended.
- (B) 90 days for every kind, as OSS-Fuzz does. Simple, but it lets an open escape wait three
  months.
- (C) 7 days for an escape or crash. Tighter, but one large fix could outlast it.

Prior art. OSS-Fuzz discloses an issue to the public 90 days after it notifies the project, or
after the fix ships, and moves a weekend deadline to the next work day. A project that tells
OSS-Fuzz a patch lands within 14 days after the deadline gets the disclosure delayed until the patch
(read). syzbot publishes each bug's age since the report and since the last crash, and closes a
bug only when the fixing commit reaches all tracked builds (read). I could not verify whether syzbot sets a fix
deadline or closes a bug after a quiet period.

Where nife differs. The 90 days are a disclosure clock for outsiders, and nife's findings are public
from the start. The clock here protects the rating, which is why an escape gets the shorter limit.

Example. A sweep reports an escape on 2026-11-03. A lane triages it on 2026-11-04, inside 2 days,
and confirms it. The fix is due 2026-11-17. The fix merges on 2026-11-12, and the daily job
shows the finding closed on 2026-11-13. With no fix by 2026-11-17, condition 3 fails on
2026-11-18 and the rating falls until a fix merges.

## Appendix: measurements, 2026-10-31 (UTC)

Runs on a shared multi-core build host with other lanes' load, one QEMU guest per process.

Rate per guest, x86_64 under hardware acceleration (KVM):

| guests at once | kernel-thread driver (milestone 752), seeds/s | user-mode driver (milestone 779), seeds/s |
|---|---|---|
| 1 | about 420 | about 230 |
| 2 to 8 | 440 to 460 | about 220 |

Under full emulation (TCG), per guest, flat from 1 to 16 guests within noise: aarch64 17 to 20
(kernel-thread) and 9 (user-mode); riscv64 9 to 11 and 6 to 7; x86_64 15 to 18 and 9 to 10. KVM is
about 25 times faster on x86_64. An emulated guest uses about 1.2 host cores and 126 MB
(kernel-thread) or 264 to 375 MB (user-mode), and 62 guest-runs showed no false red. The
emulation note in `notes/fuzzing/` holds the table.

Memory under KVM: about 100 MB per guest.

Host target `redoxfs_server_session`: 794 runs/s on one process and about 5,470 runs/s on eight
workers.

False reds, guests that reported a failure that did not reproduce:

| guests at once | kernel-thread driver | user-mode driver |
|---|---|---|
| 4 | 1 of 4 | 0 of 4 |
| 8 | 4 of 8 | 1 of 8 |
| 16 | 6 of 16 | 6 of 16 |

Replays of failing seeds were green. The harness's deadlines expired under load; no escape
occurred. Until a fix lands, more concurrency buys false reds faster than seeds.

## The seven questions

1. Considered and lost: question 5's options above.
2. In this tree: no finding has a clock today.
3. Prior art: above, with what I did not verify marked.
4. The premise holds: condition 3 names the clock and no number exists.
5. Cost: lane time to fix inside the limit.
6. Reversible: the numbers are a record, and nobody has acted on them.
7. At equal cost this would still choose (A); the recommendation is not about effort.

## What waits on this

Conditions 1 to 4 of criterion (d), and so risk 7's green, until an architect rules on question 5. The false-red fix can start now.

## What I need from you

Question 5 only.
