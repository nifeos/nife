# The nine things that would kill nife

*Name: provisional, for the directory and every appendix stem in it. Each appendix's own preamble says the same of its stem.
Naming is calef's; `script/names --unratified` lists each stem.*

Eight of the nine risk files are ratified (2026-10-05, UTC; calef, PR #1675: "Use the claim
forms."): each stem states its risk's claim. Refused `5-multicore-reliability` and
`6-userspace-driver-speed`, which name a topic rather than the claim.

calef, 2026-08-30: *"something that would kill nife for me as a project is a fatal characteristic
that would demonstrate the approach isn't viable... We should then try to prove or disprove those
things."*

This directory is the falsification list. Not a risk register, which tracks things that might go badly. It
is a list of claims that, if false, mean the project should stop. A risk you can mitigate belongs in
a milestone. A risk you can only answer belongs here.

It was written the same week the project's first customer left, when the family's backups moved to
borg over SSH on cordoba because nife was not ready. With no customer, the ranking function has
nothing to rank by. The substitute is "find out whether this can work at all."

It is a six-pager, and the depth is in appendices (calef, 2026-09-23). At 17,742 words, reading it
once cost a maintainer session most of a context window. A reader can decide what to work on next from
this page and the nine entries, without opening a single appendix. Each entry links one, in
[`design/fatal-risks/`](.), holding that risk's evidence, numbers, corrections and refusals
for anyone who wants to verify or challenge a verdict. Each entry has a file of its own (calef,
2026-10-05), because one shared file was the conflict point for merge-queue ejections. Studies with a home of their own stay in
`notes/`. Earlier numbers are in `git log -p -- design/fatal-risks/`.

## The rule an entry has to meet

Three properties, and an entry that lacks one is a worry rather than a risk:

1. It can come back red. An experiment that can only confirm is not a test. Where an experiment
   is structurally confirmation-biased, the entry says so and names the second pass that fixes it
   (risk 2 is the worked example).
2. The experiment is cheap relative to the project. A test that costs a year answers a question
   the year would have answered anyway.
3. It does not wait on more of the project being built. Otherwise it is a schedule, not a test.

The ranking is chance-of-fatal times cheapness-of-test, which is why the running order at the bottom
is not the numbering. The numbers are identity, like a milestone's.

## Who may change an entry

Verdicts are the architect's: the Experiment status word, the colour and the running order. The
maintainer corrects a factual error (a wrong date or instrument, a claim the machine disproves)
without asking: the fact is fixed in place and cites its source, and the date and the old wording go
in the commit message. Facts arguing for a new verdict go to the architect.
[§216 (fatal-risk facts are correctable, and verdicts are the architect's)](../decisions/0216-fatal-risk-facts-are-correctable-verdicts-are-the-architects.md),
2026-09-25.

## What an entry's Experiment status says, and the three words it may say it in

Every risk file opens with a frontmatter block carrying one Experiment status, `experiment_status:`,
and, when the status is `RUN`, the date it ran as `experiment_run:` (both names ratified 2026-10-05 by calef). The
field answers one question: has the experiment happened. calef moved it out of a bold line and into
frontmatter on 2026-10-05 (UTC), and `script/fatal-risks` fails on the old bold line. calef ratified the field and its three values on 2026-09-23, in
§211 (what a fatal-risk verdict says, and what the chart can plot as a result).
`script/fatal-risks` fails on a fourth value, because the set was open until then and three lanes
minted three words in one day. The script's own header carries the ratification and the refusals.

| value | what it asserts |
|---|---|
| `RUN` | the experiment has been performed, whatever it found |
| `NOT-RUN` | it has not been performed, and could be |
| `CANNOT-RUN` | it cannot be performed at all, and the entry says what would change that |

What it does not say is what the experiment found. That is prose, and it is where `GREEN`, `AMBER`,
`MEASURED` and `AUDITED` live. None of the four is a value of this field. A reader who wants to know
whether nife is in trouble reads the paragraph. The colour has one machine-read home, the
appendix's `color:`, and the entry's verdict sentence is the prose it must agree with; the risk files
carry no `color:` key.

## The nine, one file each

A risk's file holds the claim of record: its status, its experiment, its cost and its caveats. Each links an appendix with the evidence.

1. [Only software written for nife runs on nife](1-only-software-written-for-nife.md)
2. [The proofs prove trivia, and the real bugs live where Kani cannot reach](2-the-proofs-prove-trivia.md)
3. [The tests do not test anything, and the quality is illusory](3-the-tests-do-not-test.md)
4. [The architecture imposes a per-crossing cost that cannot be engineered away](4-the-per-crossing-cost.md)
5. [It cannot be made reliable on multicore, and the bugs appear only on silicon](5-not-reliable-on-multicore.md)
6. [A capability-confined userspace driver cannot drive real hardware at real speed](6-a-confined-driver-is-too-slow.md)
7. [The confinement claim is false](7-the-confinement-claim.md)
8. [Nobody else builds on nife's mechanisms](8-nobody-else-builds-on-nife.md)
9. [The HAL is a fiction, and an architecture costs a restructure rather than a port, and so does the next machine](9-the-hal-is-a-fiction.md)

## The standard benchmarks each risk is measured by

calef ruled on 2026-10-08 (UTC) that nife is measured with the field's standard benchmarks, run
unmodified against Linux, and optimized toward. The ruling is §262 (nife is measured with the
field's standard benchmarks), [in its own file](../decisions/0262-nife-is-measured-with-the-fields-standard-benchmarks.md).
Each risk file lists its own under *Benchmarks that inform this risk*. None has produced a number yet.

| risk | milestones |
|---|---|
| 1, the ecosystem | 831 (SQLite), 833 (fio), 834 (ioping) |
| 4, the per-crossing cost | 25 (`sel4bench`), 826 (lmbench), 827 (hackbench), 828 (iperf3), 829 (netperf), 831, 832 (STREAM) |
| 5, multicore | 827, 830 (wakeup latency) |
| 6, a confined driver | 828, 829, 833, 834 |

Risks 2, 3, 7, 8 and 9 have none. They are claims about proofs, tests, confinement, uptake and
portability, which a speed benchmark does not measure.

## The running order

Ranked by chance-of-fatal times cheapness-of-test, not by number. Each cell's verdict is the entry's.

| order | risk | experiment | owner | cost |
|---|---|---|---|---|
| ~~1~~ | 2, the proofs | **RUN, 2026-08-30: amber**, because no standing proof has caught a regression, and `asm!` bounds the reach | milestone 191 | done |
| ~~2~~ | 9, the HAL, on the board that already boots | **RUN, 2026-09-03:** a board run ends in a verdict a machine reads, the UART verdict marker and SBI SRST shutdown in `kernel/src/arch/riscv64/semihosting.rs`, read by `script/soak` and `crates/serial_console`; radon's soak ran on it. Closed by calef, 2026-10-14 (UTC) | milestone 16 (real hardware and IOMMU-backed driver isolation) | done |
| ~~3~~ | 9, the HAL, on the architecture that carries the risk | **RUN, 2026-09-17: GREEN**, five of five on xenon, everything it needed inside `arch/x86_64/` | milestone 87 (the x86_64 bare-metal machine) | done |
| 4 | 9, the HAL, at the implementation grain, widened 2026-09-23 | a second machine of an architecture nife already boots | milestone 225 (run the soak on radon, argon and xenon) | **RULED 2026-10-06: riscv64 first** (calef, *"We can do the Scaleway first"*): milestone 89 (Scaleway EM-RV1), €1.51, then argon (the TX1) when it arrives; neither run. History: ruled 2026-10-05 "both, argon first"; on 2026-10-06 argon proved not in hand, as the seller shipped a TK1 against the TX1 order (`notes/bench-runbook.md`) |
| ~~4~~ | 1, the ecosystem | **RUN, 2026-08-31: GREEN** (calef, 2026-10-12: *"Track packaging separately. We already do."*; amber 2026-10-06 to 2026-10-12, green from 2026-09-16). All three gaps closed: CI re-checks `rg` (#1853), a second foreign program (835's `speedtest1` and ioping, #1896), and a foreign program running a second thread (`rg`'s parallel walk, #2016) | milestone 121 (`ripgrep` on nife); threads are milestone 812 (`std::thread::spawn` runs real threads in one address space) and 836 (a C library, stage 2: threads) for C | done. Packaging is tracked separately: milestones 810 (`ripgrep` is packaged in basalt) and 897 (SQLite is packaged in basalt), and a decision in flight (#2023) |
| ~~5~~ | 3, the tests | **RUN, 2026-09-19: amber.** 96.1% like-for-like against 92.4% on 2026-09-21, and 771 missed survivors (414 projected after #1277) hold the amber | milestone 326 | done; the triage remains |
| ~~6~~ | 4, performance | **RUN, 2026-10-04: GREEN** (calef, 2026-10-05; amber 2026-10-04). Throughput held, and the null syscall's rise under load was a fixable lock and false sharing, now 10 ticks of growth | milestone 168 (a multi-tasking workload benchmark); milestone 761 (capability lookup off the global lock) and milestone 766 (each core's PerCpu on its own cache line) done | done; about 5 ticks of growth remain undecomposed |
| 5 | 8, nobody else builds on nife's mechanisms | **NOT-RUN.** Honest attempts at releasing or contributing nife's mechanisms, each on its own 6-month clock, counted as adopted, absorbed, handed off or composed; red is readable once five attempts have expired with no uptake, and at 2027-10-18 if nothing has left nife. Placed after the HAL run (calef, 2026-10-18) | none yet; the first three mechanisms are an architect's pick | low: a release or pull request and its explanation per attempt, and months of waiting |
| 7 | 9 and 6 together | journey 3, end to end on three boards | journey 3 | months, and it is the capstone |
| -- | 5, multicore | **RUN on radon, 2026-10-10:** SS259 met for radon, 10.29 million crossings over 3 plain boots, every exposure row clean, the defect-discovery curve flat zero. Ruled facts-not-verdict by calef, 2026-10-10: the verdict waits until a second architecture runs SS259, and xenon is next | milestone 201 (is multicore reliability converging) | weeks, hardware |
| -- | 6, a confined driver at real speed | **RUN, 2026-10-04: AMBER** (calef, 2026-10-08; GREEN 2026-10-07, amber 2026-10-04). Write 1.08x and read 2.16x Linux's best on the same disk at queue depth 1; the read gap is unexplained, so the verdict is back to amber until a same-job-file benchmark (ioping proposed, then fio at queue depths 1, 4 and 32) explains it | milestone 261 (the NVMe driver leaves the kernel) done | the benchmark is the cost; fio waits on threads |
| -- | 7, confinement | **RUN, 2026-08-31, extended 2026-09-16, AUDITED 2026-09-17: amber** (calef, 2026-10-03). A confinement test could not fail, and DECISIONS §12 (call/reply IPC: a one-shot reply capability) was false on x86_64. Fixed. The outsider half remains. Fact, 2026-10-10: criterion (c)'s count restarted at zero on the sixth pass's DNS rebinding in the package fetch, which calef ruled "an escape on a shipped path"; fixed in #1901 | milestone 633 (an outside agent attacks the confinement claim); 202, 305 and 313 done | one agent run, token cost uncosted |

## BUGS

- ~~Nothing gates these entries.~~ Closed 2026-09-11 for the mechanical half by milestone 275 (a gate
  that diffs fatal risks against the roadmap it cites). `script/fatal-risks --check` runs
  in `script/lint` and compares what the nine entries claim about a milestone or a decision against what
  the record holds. It found four live disagreements on its first run.

  What is not closed is the larger half. A gate can see a status word contradicting the record. It
  cannot see a premise being overtaken, which is what happened to risk 9's cost line: it priced
  milestone 87 as bench time when `notes/x86-port.md` already recorded that no real firmware speaks
  PVH. A green `script/fatal-risks` means no status word here contradicts the record it names. It is
  not a warrant that the arguments still hold.
- Nothing gates an appendix. `script/fatal-risks` parses this page and the nine risk files, so a verdict restated under
  `design/fatal-risks/` can drift and no check will say so. The appendices therefore do not carry the
  Experiment status field at all, and each says at its head that this document is the claim of
  record. That is rung three of AGENTS.md's ladder, and honest about being rung three.
- The bold the risk files carry is the gate's, not the prose's. `script/fatal-risks` reads the
  experiment lead-ins and the running order's verdict cells as markup, so those spans are machinery
  rather than emphasis. The Experiment status lines left that list on 2026-10-05 (UTC), when calef
  moved them into frontmatter. Every other bold span is gone, and what is left spends the whole
  writing-convention budget of four per thousand words. Editing an entry means spending the gate's
  budget, not your own. calef ruled on 2026-09-24 (UTC) that markup a gate parses is counted like any
  other bold, so no exclusion exists. Until the lead-ins move too, do not add a bolded span to an
  entry without removing one.
- ~~Two entries have no owner.~~ Closed 2026-08-31: risks 5 and 7 are milestones 201 and 202, both
  scoped by calef and both reframed in the process, risk 7's by §134 (a harness carries a
  machine-replayable falsification record, or it is not evidence). Neither can return a clean green,
  and both blocks say so where a reader meets them.
- The ranking is a judgement, not a calculation. "Chance of fatal" is nobody's measurement, and two
  readers could order this differently on the same evidence.
- A green result is not proof of anything. Every experiment here can only fail to kill the project.
- The word budget is a constraint on these documents, not on the truth. calef asked for 3,000 words.
  With one file per risk this page is near 1,900 words and the largest entry (risk 7) near 1,700,
  so nothing here carries an exception marker. Put any growth in the appendix beside the entry.
