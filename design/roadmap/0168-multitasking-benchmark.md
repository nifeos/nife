---
status: BUILT
raised: 2026-08-25
built: 2026-10-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: real silicon; PMU cycle counter
specific_machine: none
needs_person: yes
---
# 168. A multi-tasking workload benchmark: the number that would decide the event-kernel question

The instrument is built, gated and rehearsed on all three architectures, and every point of its
sweep is a number on silicon. Five radon boots with the current instrument (median of 21 repeats,
seven jobs) agreed within 1.67% at every point: 2.62x at four tasks, 2.78x at 32, no decline. That
is the first row of `notes/job-mix.md`'s step 7, read in "What five boots measured, radon,
2026-10-04" below, and risk 4's entry is written from them. This is the instrument [DECISIONS §96
(process kernel or event kernel, and how to decide
it)](../decisions/0096-process-kernel-or-event-kernel.md) recommends: *"Build the instrument that
could decide it. The blocker is that a multi-tasking workload is the only place the difference
appears, and we have none."*

Real silicon, and that is all (calef, 2026-09-04). A multi-tasking difference *"needs real hardware
scheduling behavior, not TCG or HVF timing"* and wants *"a place to run it that has real PMU
access"*. radon satisfies both. It is real silicon, it boots, and `kernel/src/arch/riscv64/pmu.rs`
reads cycles through the SBI PMU extension, from milestone 74 (cycle counters).

What milestone 127 (seL4) is for is a different question, and its own title says so: *the seL4
machine: a Jetson TX1, so identical silicon referees the comparison.* That matters for milestone
25's cross-OS numbers, which are compared against seL4's published runs and therefore want the
machine those runs were made on. It does not matter for DECISIONS §96's question, which asks how much
of *this kernel's* time goes into process-kernel overhead under multi-tasking load. That is a number
nife produces about itself, on any real silicon.

So the two uses split, and this block serves both without needing the same machine for each:

- For §96 (process kernel), radon is sufficient and available now.
- For milestone 25's comparison against seL4, argon is still required, and nothing here loosens
  that.

And building the instrument is gated by neither. The workload is architecture-neutral and can be
developed and tested under QEMU; what needs silicon is the *number*, not the code. A lane may build
this today and leave the measurement to a bench evening, which is how this milestone was launched.

## What this number will be compared against, written before it exists

Recorded 2026-09-21, deliberately in advance. When this milestone produces a multi-tasking
number it will be read against the language-isolation line of work, by anyone who knows the
literature: *The Case for Writing a Kernel in Rust* (APSys '17, DOI 10.1145/3124680.3124717) and
RedLeaf (OSDI '20). Writing down now what makes that comparison honest is what stops it being a
defense afterwards.

Three things the comparison has to say, or it is not one.

- The guarantee differs. A language-isolated crossing trusts the compiler and the absence of
  `unsafe` in the isolated code; a capability crossing trusts the hardware and a kernel small enough
  to prove things about. A faster number under a weaker guarantee is not a better result, and a
  slower one under a stronger guarantee is not an excuse. Say which was bought.
- The hardware and the baseline are part of the number. This tree's own rule already: state what
  each number means and where it is not apples-to-apples, the way milestone 25 (cross-OS performance
  comparison) records the map "tie" as zeroing-bound and the spawn caveat as a lighter object than a
  Unix process.
- What each system can isolate is not the same set. nife runs an unmodified `ripgrep` with zero
  patches and confines C behind DECISIONS §31 (the foreign-language seam)'s narrow interface; a
  language-isolated system isolates code it compiled. That is a difference in what the crossing is
  *for*, and it belongs beside the timing rather than after it.

This section does not say who wins. That is a claim that leaves the machine, and
`AGENTS.md` puts it in calef's hands along with every other published fact.

## What this is for

DECISIONS §96 asks whether nife should stay a process kernel (what it is today: every thread gets
its own kernel stack) or move to an event kernel (one stack per core, explicit continuations).
That is the model seL4, OKL4 and NOVA all eventually adopted. §96 found three of the four inputs
to that question already settled by measurement (memory savings are negligible at this project's
scale; stack-shrinking is closed off, no slack remains; the verification argument doesn't transfer,
since Kani never reaches `kernel/src` here). The fourth input, performance, is the one live,
unmeasurable argument: the paper §96 cites (Warton, on Pistachio) found event kernels roughly tied
with process kernels on micro-benchmarks but 20% better on a real multi-tasking workload (AIM7).
Every instrument this project currently owns (`ipc_rtt`, `ipc_rtt_el0`, the icount tripwire,
milestone 132 (fast path's)'s footprint gate) is a micro-benchmark, and would show approximately
nothing for this question.

This milestone is that missing instrument, and nothing else. It does not decide §96; it produces the number §96 needs to decide itself.

## What was built, 2026-09-04

Four pieces, and the shape is `kernel/src/soak.rs`'s rather than `kernel/src/bench.rs`'s, for a
reason the gate makes concrete: this number has to be taken on a board, and the bench boot has
never run on one. What a board takes is `script/board-image` plus `script/board-console`, which is
a kernel feature printing to the serial console.

- `crates/job_mix`, the workload's definition, host-tested. What AIM7 is, read on
  2026-09-04 from the benchmark's own README and from the ACM TOCS 2016 retrospective rather than
  guessed at. Which four of its methodological properties are kept (heterogeneity, per-task random
  order, a task-count sweep, a throughput metric), and which of its 53 Unix-shaped jobs are
  deliberately not. Five jobs stand in for its categories.
- `fixtures/src/job_mix_task.rs`, one EL0 task. Processes rather than kernel threads, which is the one
  place this differs from milestone 134's E1 on purpose: §96 asks what a process kernel costs a
  *workload*, and a workload is processes.
- `kernel/src/job_mix.rs` (`--features job_mix`), the supervisor. It builds the pool once, releases
  a subrun's worth of tasks, owns the wall clock, and prints the placement census before the first
  number. Milestone 240 (soak reports) found placement decides throughput on radon by up to fifteenfold,
  and a figure without its arrangement is a draw rather than a result.
- `script/job-mix` (QEMU rehearsal) and `script/board-image --job-mix` (the card).
  `notes/job-mix.md` is the bench procedure, with a table mapping every observable outcome to what it
  means and where it routes.

Rehearsed on patagonia on 2026-09-04: aarch64, riscv64 and x86_64, sweep complete on each, six
points per run. Those magnitudes are not a result and are not recorded as one; TCG models no
cache.

The 20% figure's provenance, and why Warton's own claim is weaker, is in `notes/job-mix.md`.

## What it needs

- A real multi-tasking workload, not a single-operation timing. The cited paper's own instrument is
  AIM7 (see `notes/l4-lessons.md`'s citation for the exact numbers this project has already quoted
  from it: *"20% performance advantage on a multi-tasking workload (AIM7)"*). Whoever builds this
  should read the paper's own methodology rather than guess at AIM7's shape from the name. They then
  decide whether to port something AIM7-equivalent or design a workload that exercises the same
  property (many threads, contended scheduling, real context-switch pressure) in a way that fits
  this project's own capability model.
- Real hardware, per the gate above: this specific difference does not show up under QEMU TCG or Apple's HVF, the same reason `sel4bench` (milestone 25) is deferred to real silicon.
- A place to run it that has real PMU access, matching what milestone 25's own `sel4bench` piece already needs from milestone 127's machine, so the two pieces of hardware-gated work should likely be sequenced together rather than treated as unrelated.

## Why it matters, beyond §96

Milestone 25's cross-OS comparison has the same hole, and this milestone closes it too rather than duplicating it. Checked directly: milestone 25 is explicitly a set of EL0-measured *primitive* benchmarks (single syscall, single context switch, single IPC round trip, single map, single spawn) compared against lmbench and `sel4bench`. Every one of them is a micro-benchmark in the same sense §96 means the word. Milestone 25's own remaining piece (`sel4bench`) is single-operation PMU timing, not a multi-tasking workload. So neither milestone currently has an instrument that could show what a real multi-tasking difference looks like, and building one here serves both.

## What this does not decide

Whether nife should switch kernel models. That is DECISIONS §96's own question, and it stays open until this milestone's number exists (or until a real customer-path workload starts creating threads in the hundreds, the other condition §96 names for reopening early).

## What five boots measured, radon, 2026-10-04

Image `04a8f9e6c`, netbooted, sealed, power-cycled between boots by calef. Transcripts
`bench/radon-2026-10-04/jobmix-boot1.log` to `boot5.log`. The full step 6 reading, with the per-kind
table and every caveat, is [its own page](../../notes/job-mix/radon-2026-10-04.md).

| tasks | boot 1 (representative) | median of five boots | spread | over 1 task |
|---|---|---|---|---|
| 1 | 323,972 | 323,972 | 0.03% | 1.00x |
| 2 | 567,826 | 568,578 | 0.26% | 1.76x |
| 4 | 845,791 | 848,794 | 1.12% | 2.62x |
| 8 | 876,087 | 876,637 | 1.67% | 2.71x |
| 16 | 895,000 | 893,887 | 1.08% | 2.76x |
| 32 | 899,399 | 899,399 | 0.42% | 2.78x |

`jpm_median`, jobs per minute; spread is (max - min) / min across boots, as below. Boot 5's census
differed from the others' and it read within 0.6% of the median everywhere.

Not apples-to-apples with the 2026-09-16 table: a different statistic and two heavier jobs. Not a
comparison with Warton: one arm, one machine, no disk job, 32 tasks at most. At 32 tasks the
`round_trip` job holds 80% of task time, waiting on two shared servers, so the plateau is partly
the instrument's own. And `null_syscall` nearly doubles from one task to four while `compute`
grows 6%: a per-crossing cost that rises with busy cores, unexplained, and recorded in
`notes/job-mix.md`'s `BUGS`.

## The earlier reading and the instrument change

The 2026-10-04 numbers above are the second silicon reading. The first, five radon boots on
2026-09-16 with the old instrument (best of three repeats, five-job mix), established the curve's
shape and showed that `tasks=4` was not a number under that statistic. The current instrument takes
every point as the median of 21 repeats with the ends beside it, and its mix includes page-mapping
and process-creation jobs with no new syscall, method or object type. §191 (whether the job mix
reports the spread rather than the best) records the line format, ratified by calef on 2026-09-26.

- [radon-2026-09-16.md](0168-multitasking-benchmark/radon-2026-09-16.md): the five boots' table,
  the within-boot spread behind `tasks=4`, why that point and no other, the census correction, and
  what was solid.
- [holes-closed-2026-09-19.md](0168-multitasking-benchmark/holes-closed-2026-09-19.md): the
  resampling evidence for the median of 21, the new line format, the map and spawn jobs and their
  allocator accounting, why the disk mix was not built, the smaller fixes, the HVF attempt, and the
  two proposals absorbed.

## BUGS

- Every silicon number is radon's; argon and xenon have not run the sweep.
- The disk-file category is still absent, with its own proposal. A flat curve from this mix is
  evidence about compute, memory, trap, scheduling, IPC, mapping and process creation, and silent on
  the filesystem.
- The per-kind ticks include preemption, by design, so they are not a pure path cost. Read their
  growth with task count, not their magnitude.
- The breakdown exchange between repeats warms the report endpoint and the supervisor's stack,
  which the old sweep did not do. It is identical for every repeat.
- The HVF cross-check had no machine until milestone 227 (a GICv3 driver) was built on 2026-09-19,
  and the full mix has completed under HVF only since #1663 (2026-10-05). The runs since then took
  `null_syscall` at one to four tasks; none is recorded as the cross-check of this curve's shape.

## Follow-on

- **Done.** Risk 4's entry, written from the 2026-10-04 numbers in the change that built this block.
- **Recorded.** What the sweep cannot see above 32 tasks. `job_mix::MAX_TASKS` is 32 because
  every task is a process with an address space, where milestone 134's E1 reached 96 kernel threads.
  Checked by reading `crates/job_mix` and `kernel/src/job_mix.rs`: nothing has measured where a board
  runs out, and the supervisor's `FAILED: could not spawn task N` line is the measurement
  that would say. radon fit 32 on 2026-10-04 and printed none; above that is unmeasured.
- **Milestone 324.** `crates/serial_console` has no recogniser for this run, so an operator tells a
  finished sweep from a wedged one by reading the log, with the reason it
  was deliberately not written blind. The outcome table in `notes/job-mix.md` becomes evidence after
  the first bench evening and a guess before it.
- **Milestone 493.** The disk-file mix, blocked on a disk radon can drive and on a file-service
  channel per client (a wire format, so calef's).
  milestone 493 (a disk-file job mix needs a disk), `design/roadmap/0493-a-disk-file-job-mix-needs-a-disk-radon-can-drive.md`.
- **Recorded.** The HVF cross-check on patagonia, unblocked since milestone 227 (a GICv3
  driver) and #1663: `script/job-mix --hvf --release --smp 4`, alone on the host.
- **Recorded.** The mix proportions are chosen rather than derived from an AIM7 workfile, so a
  different mix gives a different number and no result is quotable without saying which mix produced
  it. It stays a limitation: nobody has an AIM7 workfile for a capability microkernel, and inventing
  one would be a stronger claim than the evidence supports. Recorded beside the feature in
  `crates/job_mix`'s own `BUGS` and again in `notes/job-mix.md`.
- **Refused.** A committed baseline and a `--check` for this sweep, the way `script/bench` gates
  against `bench/baseline-*.txt`. Refused because the icount instrument's determinism is what makes
  that gate meaningful, and a workload whose entire subject is scheduling under contention is not
  deterministic on any accelerator this tree has. A gate here would be asserting a tolerance nobody
  has measured, which is how `script/lint` has already lost three checks.
  This refusal is
  milestone 464 (design/roadmap/0464-a-committed-baseline-for-the-multitasking-sweep.md), which
  carries it with the condition that would change it.

## Index row

Minted from DECISIONS §96's own recommendation: every benchmark this project owns is a
micro-benchmark, and the one number that could decide process-kernel-vs-event-kernel only shows up
on a real multi-tasking workload, on real hardware. The instrument is built, gated and rehearsed
on all three architectures (`crates/job_mix`, `fixtures/src/job_mix_task.rs`, `--features job_mix`, `script/job-mix`). Five radon boots on 2026-10-04 with the 2026-09-19 instrument gave a number at every point, within 1.67% across boots, with no decline through 32 tasks. Closes the same hole in
milestone 25's cross-OS comparison too.
