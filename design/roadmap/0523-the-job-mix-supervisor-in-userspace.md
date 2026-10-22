---
status: NOT-STARTED
raised: 2026-09-21
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 523. Moving the job-mix supervisor into userspace, and the five permissions it turns out to need

`script/board-image --job-mix --tftp` reported `NOT SEALED` on every attempt on radon, which is what
raised this. The move is investigated end to end and **none of it is built**, because every route to
it runs through a decision that is an architect's. What is below is the investigation, the one
premise in the brief that is false, the five things a shell-spawned program would have to be allowed
to do, and what each would cost. `--features job_mix` is how the number is taken.


Five of them, and they are not one question wearing five hats. Two are the
syscall surface (a thread's placement, a `START` return value), one is the spawn manifest, one is
the shell's authority model as `crates/grant_plan` states it in its own words. One is an ABI
fact about what a process may know about its machine. Any single one left unanswered leaves an
instrument that either cannot be built or produces a number its own page says is not quotable.

## What provoked it

Moved on 2026-10-12 (UTC) into [`what-provoked-it.md`](0523-the-job-mix-supervisor-in-userspace/what-provoked-it.md)
under §212 (a prose budget): why a job-mix card read `NOT SEALED`, measured. Why that finding is
separate from this milestone and became milestone 563 (a seal check that reads bytes cannot see a
check that was dropped).

## The premise in the brief that is false, and it is the good news

**The supervisor does not need the cycle-counter grant of milestone 229 (build the cycle-counter grant DECISIONS 139 decided)**, and the question does not arise.

`kernel/src/job_mix.rs` times a subrun with `arch::timer::now()`, which is `CNTVCT_EL0` on aarch64
(`kernel/src/arch/aarch64/timer.rs:556`) and `rdtime` on riscv64
(`kernel/src/arch/riscv64/timer.rs:136`). `user_mode_runtime::now()` reads **the same register with
the same instruction** on each, and is ambient at EL0 because the kernel opens
`CNTKCTL_EL1.EL0VCTEN` and `scounteren.TM` in its own timer init. `fixtures/src/job_mix_task.rs`
already self-times with it today.

The cycle counter is the *other* counter: `PMCCNTR_EL0` and the `cycle` CSR, which milestone 228 (the cycle counters are closed by assumption, and on two architectures the assumption is a comment)
deliberately closed and which only `fixtures/src/cycle_counter_reader.rs` reads. Nothing in this
instrument wants it. So there is no syscall to invent here and `abi::thread_control_block`'s
deliberate absence of a grant method stays undisturbed.

**A tick is a tick across the move**, on aarch64 and x86_64. The one architecture where it is not is
riscv64, and that is blocker 5 below.

## What a userspace supervisor has to be able to do

The kernel supervisor is 382 lines and every one of its privileged acts has a userspace verb. The
list is short and it is what makes the blockers concrete:

1. **Find `job_mix_task`'s ELF in the initrd archive.** `nifefs::Fs::parse(archive).read(..)` plus
   `elf::Elf::parse`, the shape `components/src/root_supervisor.rs:63-67` already uses.
2. **Create 67 rendezvous objects** out of its own budget: one report, 32 go, 32 child-done, 2 echo.
   `retype_into_kernel_object(budget, abi::objtype::RENDEZVOUS)`, which
   `fixtures/src/rendezvous_minter.rs` proves works from EL0.
3. Build and start 34 processes with a per-child grant set, each carrying its own rights.
   `supervision_protocol::build_child` and `start_child`.
4. Split 32 per-task budgets off its own region (`job_mix::TASK_BUDGET_PAGES` is 25 pages each,
   and a task needs its own because the map and spawn jobs spend it).
5. Read the wall clock and the counter frequency, and print machine-readable lines
   `crates/serial_console`'s recognizer can read.
6. Print the placement census, because `notes/job-mix.md`'s procedure reads it at step 3 and
   `kernel/src/job_mix.rs`'s first `BUGS` entry says a number without it is not quotable.

Items 1 to 4 are all things *some* program in this tree already does. None of them is a thing a
shell-spawned program does, and that is the shape of what follows.

## The five blockers, each with what would close it

### Blocker one: process construction is the authority the shell deliberately withholds

`crates/grant_plan/src/lib.rs:358-360` states it as design intent, not as an omission: *"spawning by
name is the shell's own capability and is granted to nothing the shell spawns"*. It is echoed in
milestone 126 (the `procps` package: who else is running, and who is allowed to ask)
and milestone 281 (`watch` holds exactly what `ps` holds, so it is nothing). Of the thirteen programs the shell can run
(`least_authority_demo`, `memory_grant_depleter`, `interrupt_heeder`, `interrupt_ignorer`, `date`,
`rm`, `wc`, `mdr`, `ps`, `pgrep`, `uptime`, `printenv`, `uuid`), **not one** contains a
`build_child`, a `retype_into_kernel_object(.., RENDEZVOUS)` or a `memory_region_split`. Even
`memory_grant_depleter`, the only one holding an untyped at all, only maps frames with it.

A job-mix supervisor is a program whose entire job is building 34 processes. Admitting it to that
list is a change to what the shell's manifest is willing to grant, and the manifest is a published
contract between the shell and the progenitor.

What would close it: a new `MemoryGrantDeclaration`-shaped declaration for construction authority, or a
decision that this one program is endowed by the progenitor at boot rather than by the shell. Both
are an architect's, and the second brings blocker 1b with it: a boot-built supervisor needs a wire
protocol for the shell to ask it to run, which is squarely *anything two programs agree on*.

### Blocker two: a shell-spawned program never receives the initrd

`crates/system_initializer`'s `spawn_service` builds every job with
`supervision_protocol::build_child` (`:2470-2496`), and `build_child_space` maps ELF segments, stack
pages, an x86_64 timebase placeholder and `endow.maps` -- nothing else. The `maps` a normal job may
receive are chosen at `:2451-2462` and are exactly three: a filesystem client page, the clock page,
or the config page, with the comment *"One extra mapping at most, today"*. `INITIAL_RAMDISK_VA` reaches only
the kernel-spawned progenitor (mapped in `user/entry.rs` and `user/progenitor_boot.rs`), and `a1` carries the manifest's
integer argument rather than `initial_ramdisk_len`.

So a supervisor launched from the prompt cannot read `job_mix_task`'s bytes. It is the same gap
`components/src/spawner.rs` was designed around from the other side: that program is handed one
image on purpose, *"so 'build me program X' is not a thing that can be asked of it"*.

What would close it: a fourth `maps` kind and a manifest field declaring it, which hands a
program the whole archive. That is a wide grant by this tree's standards and it deserves an argument
rather than a patch.

### Blocker three: `--mem` tops out at 64 pages against a need of about 3,000

The only non-`Forbidden` memory manifest in the tree is `memory_grant_depleter`'s,
`MemoryGrantDeclaration::Required { min: 1, max: 64 }` (`crates/grant_plan/src/lib.rs:450-454`), and its comment
says why the ceiling is where it is: *"a sanity ceiling the shell's own budget can actually back"*.
The shell's whole budget is `SH_BUDGET_PAGES = 128` (`components/src/swish.rs:143`, matched at
`crates/system_initializer/src/lib.rs:503`), and a job's region is `JOB_REGION_PAGES = 40` out of a
`JOBS_BUDGET_PAGES = 240` pool for six live jobs.

A supervisor needs, roughly: 34 children at about 40 pages each for segments, stack, page tables and
a TCB; 32 task budgets at `job_mix::TASK_BUDGET_PAGES` (25); and 67 rendezvous pages. That is about
2,200 pages, call it 3,072 with headroom, 12 MiB. The progenitor's own root region is 12,288
pages (created by `boot_progenitor`, `user/progenitor_boot.rs`) with roughly 2,800 already committed to the shell, the jobs pool,
the credentialer and login, so the memory exists. What does not exist is a route from it to a
shell-spawned program, and widening `SH_BUDGET_PAGES` by a factor of twenty-four to carry one
benchmark is a cost every interactive boot pays.

What would close it: a manifest ceiling per program rather than one shell budget for all of
them, or the boot-endowed route of blocker 1.

### Blocker four: nothing lets a userspace thread learn where it was placed, so the census dies

This is the one that decides whether the instrument is worth moving at all, because without it the
numbers are not quotable by the instrument's own published standard.

Milestone 240 (the soak reports what happened and not where, so an eightfold difference cannot be explained) found that placement decides throughput on radon by up to fifteenfold.
`kernel/src/job_mix.rs` answers it with `sched::spawn_reporting_placement`
(`sched/inspection.rs`), which returns the core the kernel picked, and prints a
`job-mix-census:` line per core. `crates/serial_console/src/lottery.rs` reads those lines to judge a
draw, and `notes/job-mix.md` step 3 tells the operator to read them before reading any number.

There is no userspace path to that fact, and it is not a gap in one place:

- No syscall. `crates/abi` mentions a CPU only in prose.
- `abi::rendezvous::SURVEY` returns exactly `(next_cursor, tid, state)`
  (`kernel/src/syscall.rs:272-284`), and `state` is one of `READY`, `RUNNING`, `BLOCKED`, `DEAD`.
  `RUNNING` means *on a CPU right now*; it does not say which.
- `crates/ps` has no CPU column, by construction (`crates/ps/src/lib.rs:306`).
- `crates/cpu_set` is kernel-internal mask arithmetic and exports nothing to EL0.
- No mapped page carries it: the only pages a job can be endowed are the clock and config pages.
- `abi::thread_control_block::START` returns `Ok(0)` (`kernel/src/syscall.rs:346-352`). Returning
  the placed core instead would be `spawn_reporting_placement` exactly, available to any builder,
  and it is one line of kernel -- and it changes a syscall's return contract, which existing
  callers test for equality with zero (`fixtures/src/os_primitives_benchmarker.rs`).

Three options, and all three are an architect's:

- A. `START` returns the placed CPU instead of zero. Smallest change, gives a builder exactly
  what the kernel supervisor gets, ships on all three architectures for free because placement is
  `sched`'s and not `arch`'s. Costs: a syscall return contract that today means only "ok", and every
  caller that compares it to zero.
- B. A fourth word from `SURVEY`. Fits an existing method and a supervision endpoint is already
  the thing that names a domain's threads. `crates/grant_plan/src/lib.rs:350-352` already reserves
  the idea of a fourth `SURVEY` word for scheduled CPU *time* (milestone 282 (a thread's CPU time, and the `top` it makes possible), DECISIONS §150 (how does a thread's CPU time reach userspace?)).
  This would want to be decided alongside that rather than ahead of it.
- C. An EL0-readable core id, written by the context switch into a register the way milestone
  229 (a thread reading the CPU's cycle counter from user mode) writes its grant. No syscall number
  at all, and the same mechanism shape this tree already chose once. It is nonetheless an ABI
  fact: a register a program may read and rely on, on three architectures, forever.

Doing none of them is also an option and it should be stated as one. The supervisor could print
the per-task subrun ticks it already collects and let an uneven spread stand in for the census. That
is strictly weaker: it says the arrangement was uneven without naming a core, `lottery.rs` cannot
judge a draw from it, and `notes/job-mix.md`'s step 3 would have to be rewritten to ask for less.
This lane's recommendation is that it is not enough, because the instrument's own page says a number
without the arrangement is not a number.

### Blocker five: on riscv64 a userspace program cannot learn the counter frequency, and radon is where the number is taken

`user_mode_runtime::cntfrq()` on riscv64 returns a hardcoded `10_000_000`, and its own doc says
why: RISC-V has no register that reports the timebase, it lives in the device tree, and *"userspace
cannot read it"*. Radon's is 4 MHz (`design/roadmap/0375-e3-on-radon-with-real-cycles.md`,
`bench: cntfrq 4000000`). The kernel supervisor reads the real value through
`arch::timer::frequency()`.

So a userspace supervisor on the one board this milestone's number is taken on would report
`jpm_median` 2.5x too high, silently, while its `ticks_median` stayed correct. That is the
failure mode this tree fears most: a number that looks right.

The obvious fix is the timebase page `counter_frequency_contract` already defines, extended from
x86_64 to riscv64. It does not work here, and the reason is already on the roadmap. Milestone 167
(handing a computed page to a userspace-built child: closing the x86_64 timebase page's delegation
gap) records that a process built by `build_child_space` gets a zeroed placeholder, because
nothing mints a capability naming the kernel's frame. A shell-spawned supervisor is built exactly
that way. So blocker 5 is gated on milestone 167 before it is gated on anything else.

The cheap alternative is to make the frequency the program's declared integer argument
(`job_mix_supervisor 4000000`), which needs no surface change and is honest about being an input. It
is rung four of `AGENTS.md`'s ladder: a person types a number, and typing the wrong one produces a
plausible wrong answer.

## What the move would do to the number, stated now so the next lane does not have to re-derive it

Two changes, and the first is the reason to want the move:

The measurement would include the supervisor's own scheduling, and that moves the instrument
closer to the workload it claims to model. Today the supervisor is a kernel thread calling
`sched::` directly: it releases tasks and drains reports without paying the EL0 trap, and its own
runnability is not the scheduler's problem in the way a process's is. A userspace supervisor's
releases and drains are 33 real `svc` round trips per subrun at the top of the sweep, inside the
timed window, and the supervisor is a 35th process the scheduler must place and run. Under
DECISIONS §96 (process kernel or event kernel) that is not noise: the spawn and scheduling costs are
the subject. The honest framing is that the number would go down and mean more, not that the
move is free.

It is a third instrument, and the Results table would need a third row shape. The 2026-09-16
radon curve is already incomparable with anything taken after 2026-09-19. The mix went from five job
kinds to seven and the statistic from best-of-3 to median-of-21, and `notes/job-mix.md`'s own table
says the field names changed on purpose so an old parser finds nothing. A userspace supervisor
would break comparability a second time and in a different place: same mix, same statistic, same
field names, different timed window. That is the dangerous kind, because nothing in the line
would look different. Whoever builds this owes the Results table a column saying which supervisor
produced a row, and owes the printed lines something a script can tell apart. Milestone 324
(the bench console cannot speak to any of the three boards) already applied the same reasoning
to the markers.

## The kernel feature stays, and that is a finding rather than a deferral

The brief's fourth requirement was to decide whether `--features job_mix` is deleted or kept as
dead weight, and it is right that keeping it as a fallback would be the failure mode. It is kept
because nothing replaces it, and the argument for deleting it is exactly the one that is blocked
above. What it costs is now written down rather than rediscovered: it diverts the tour past
`boot_progenitor`, so a job-mix card verifies no archive, and `sealed_pair` misreports that as a
refusal. Both halves of that are recorded where a reader meets them.

## What this lane did

Investigation and records only. No code changed, no name was coined, `design/decisions/` and
`design/fatal-risks/README.md` were not touched. Added: this block; the `sealed_pair` proposal; a `BUGS`
entry in `notes/job-mix.md` and one in `crates/sealed_pair` so the next bench evening does not spend
an hour on a tool that cries wolf.

## Follow-on

- **Milestone 563.** The seal check reads bytes and cannot see a check the linker dropped:
  `design/roadmap/0563-a-seal-check-that-reads-bytes-cannot-see-a-check-that-was-dropped.md`. It is
  the urgent half of what provoked this milestone, told in the appendix above.
- **Recorded.** Whether a diverted boot tour should keep measured boot at all is a question about
  three kernel features rather than about a tool, and it is option B of that proposal. A card left
  running a rebooting soak unattended for hours is arguably the last one that should skip
  verification.
- **Recorded.** `user_mode_runtime::cntfrq()`'s hardcoded riscv64 value, in `crates/user_mode_runtime`
  where it says so. Blocker five above is the first workload it would make silently wrong, and
  milestone 167 (handing a computed page to a userspace-built child), which gates the fix, is
  NOT-STARTED.
- **Recorded.** A `job-mix` card has no measured boot, so a mismatched pair does not halt: it runs
  the sweep against whatever archive is beside it. `notes/job-mix.md`'s outcome table already has
  the row that catches it (`no 'job_mix_task' program in the initrd archive`), and that row is now
  load-bearing rather than belt-and-braces.

## BUGS

- Unbuilt, and deliberately so. Everything above is investigation. The instrument is unchanged
  and milestone 168 (a multi-tasking workload benchmark: the number that would decide the event-kernel question) still takes its number through
  `--features job_mix`.
- **The page counts in blocker 3 are arithmetic, not a measurement.** 40 pages per child is
  `system_initializer`'s `JOB_REGION_PAGES` used as a stand-in; a real supervisor might need more
  for a debug build of `job_mix_task` and would find out by being refused. Nobody has built one.
- **Blocker 4's option A was not tried.** That `START` returning the placed core is one line is read
  off `sched::spawn_reporting_placement`'s body, not compiled. The count of callers that compare its
  result to zero was not taken either.

## Index row

Investigated and stopped at a fork rather than built. Moving `kernel/src/job_mix.rs` (382 lines) to a
userspace program would delete the special build whose diverted tour drops measured boot and makes
`sealed_pair` misreport a matched card as `NOT SEALED`. It would measure the supervisor's own
scheduling, which DECISIONS §96 says is the subject. The brief's premise that it needs milestone
229's cycle-counter grant is false: the clock is the ambient generic counter and userspace already
reads it with the same instruction. Five real blockers, each needing calef. A shell-spawned program
is denied process construction by `grant_plan`'s stated intent, never receives the initrd, tops out
at a 64-page memory grant against a need of ~3,000. It cannot learn where its threads were placed (so
milestone 240's census dies, and three options for fixing that are priced here). On riscv64
it cannot learn the counter frequency, which would make radon's `jpm_median` silently 2.5x high and is
itself gated on milestone 167.
