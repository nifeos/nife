# The multi-tasking workload benchmark, and how to take its number

*(Milestone 168 (benchmark). Names here are provisional, per
the naming tenet; an architect names things.)*

Written on 2026-09-04 with the board off, rewritten on 2026-09-19 after five radon boots, and
followed by two bench evenings (Results, below). Everything else is about code in this tree, built,
host-tested and rehearsed under QEMU on all three architectures, or is a question for the bench,
marked as one.

## What this instrument is for, in one paragraph

`design/decisions/0096-process-kernel-or-event-kernel.md` asks whether this kernel should keep a kernel
stack per thread (a process kernel, what it is) or move to one stack per core with explicit
continuations (an event kernel, what seL4, OKL4 and NOVA all became). Three of its four inputs
are settled by measurement. The fourth is performance, and the retrospective it rests on says
precisely where the difference lives:

> Warton [2005] performed a thorough performance evaluation of the Pistachio process kernel vs an
> event-based (single-stack) kernel with continuations on an ARMv5 processor. He demonstrated
> comparable performance, generally within 1% on micro-benchmarks but a 20% performance advantage of
> the event kernel on a multitasking workload (AIM7).

Elphinstone and Heiser, *L4 Microkernels: The Lessons from 20 Years of Research and Deployment*, ACM
Transactions on Computer Systems 34(1), article 1, April 2016, section 4.1. Read on 2026-09-04
from <https://trustworthy.systems/publications/nicta_full_text/8988.pdf>.

Every instrument this project owns is on the left of that sentence. `ipc_rtt`, `ipc_rtt_el0`, the
icount tripwire, milestone 132 (fast)'s footprint gate, and milestone 134 (register)'s E1 through E4 are all
single-operation or single-shape measurements. This is the one that is not.

## What Warton ran AIM7 on, and why the 20% is not a target

The 20% is the retrospective's summary of Warton, not his own claim. His BE thesis (UNSW, 2005)
ran a modified AIM7 with two tasks and no sweep, on Wombat, a paravirtualized ARM Linux. So the
figure is a ratio between two microkernels, measured through a hosted Linux's syscall path. This
tree has one kernel and runs native tasks, so the mix produces one arm and no ratio. Warton himself
called the result something to treat with skepticism until explained, and never ran the cache
simulation that would have explained it.

What survives is the mechanism, not the number. Warton's only explanation was kernel cache and TLB
footprint, and whether per-thread kernel stacks displace enough cache to cost throughput here is a
real, open, local question. A knee in jobs per minute against task count answers it. The
quotations, the thesis's own table, the hardware, and the clause in §96 (process kernel or event kernel, and how to decide it) that the paper contradicts
are in [job-mix/warton-aim7.md](job-mix/warton-aim7.md).

## What the workload is, and what it is not

`crates/job_mix` and [job-mix/instrument.md](job-mix/instrument.md) are canonical, and this page
does not repeat them. The short version:

AIM7 (read on 2026-09-04 from the benchmark's own README at
<https://github.com/davidlohr/areaim/blob/master/osdl-aim-7/_NOTICES/README.aim7>) forks many
tasks, each running in random order a set of subtests called jobs. It runs a sequence of
subruns with the task count incremented between them, and reports jobs per minute against task
count. Four methodological properties do the work, and this instrument keeps all four: heterogeneity,
per-task ordering, a task-count sweep, and a throughput metric.

It keeps none of AIM7's 53 jobs, which name Unix services this system does not have and should
not grow in order to be measured. Seven jobs stand in for AIM7's categories. Five are a compute grind, a
32 KiB working-set walk, a null syscall, a yield burst, and a `CALL`/`REPLY` round trip against a
shared server. Since 2026-09-19 a page-mapping job and a process-creation job build
their objects from a per-task untyped budget. `crates/job_mix`'s `BUGS` records the one category
still missing (disk-file operations) and why, and
`design/roadmap/0493-a-disk-file-job-mix-needs-a-disk-radon-can-drive.md` carries the options.

So this is not AIM7 and no number from it is comparable with an AIM7 number. It is an
AIM7-*shaped* instrument for a capability microkernel.

## Rehearsing it, which needs no board

```sh
script/job-mix                          # aarch64
script/job-mix --arch riscv64 --smp 4   # radon's architecture and core count
script/job-mix --arch x86_64            # xenon's
script/job-mix --release                # the optimization level script/board-image builds
```

Each prints the placement census, 21 `job-mix-repeat:` lines per subrun, one result line per sweep
point and seven `job-mix-kind:` lines under each. The magnitudes are fiction (TCG models no
cache) and the rehearsal exists to prove the mechanism, not to produce a result. A job the kernel
refuses (a map or spawn budget too small, say) ends the run with `job-mix: FAILED` and exit 1, so a
green rehearsal also proves the budgets fit on that architecture.

Rehearsed on 2026-09-04 (five-job mix, best of three) and again on 2026-09-19 (seven-job mix,
median of 21) on patagonia, all three architectures, sweep complete on each: aarch64 in 115
seconds, riscv64 with four harts in 188, x86_64 in 173. The riscv64 run took 20 seconds with the
old instrument the same day; seven times the repeats accounts for most of that growth.

## What changed on 2026-09-19, and how to tell an old transcript from a new one

Each sweep point is the median of 21 repeats, with the fastest and slowest beside it, and the mix has
seven job kinds, including `MAP` and `SPAWN`. A transcript from before 2026-09-19 took the best of
three repeats over five kinds, and each difference on its own makes its numbers incomparable.

A new transcript's started line says `one of 7 kinds`, and the next line says `median of 21
repeats`. A new point line carries `jpm_median=` and no `jpm=` or `ticks=`, so a script written
against an old transcript finds nothing rather than reading a median as a best. `jpm_median` is not
comparable with an old `jpm` at any task count. The line-by-line table, the resampling that chose
21, and what a `job-mix-kind:` line says are in
[job-mix/instrument-change-2026-09-19.md](job-mix/instrument-change-2026-09-19.md).

## The next bench evening on radon, start to finish

What it is for: a sweep whose every point is a number, with the two jobs that block deepest
in the kernel in the mix. **Its result is what `design/fatal-risks/README.md`'s risk 4 gets a
verdict from.** First run 2026-10-04.

Everything about the card, the console and U-Boot is `notes/bench-runbook.md`'s,
`notes/visionfive2.md`'s and milestone 257's (network boot); this page does not copy it.

### 0. Before power

- `lsof /dev/cu.*`: one capture per serial port. Two split the byte stream silently, which cost a
  boot on 2026-09-16 (`notes/bench-runbook.md`'s `BUGS`).
- `pgrep -l qemu` on the serving Mac is irrelevant to radon's numbers, but a busy TFTP server is
  not: leave the host alone while a boot fetches.

### 1. Build the image once, from a commit that has this page's 2026-09-19 section

```sh
git log -1 --format=%h                  # write this down; it goes in the Results row
script/board-image --job-mix --tftp     # network boot, the 2026-09-16 workflow (milestone 257)
# or, with the card mounted on this machine:
script/board-image --job-mix --card /Volumes/NIFE
```

**The archive is not optional** and a mismatched pair halts at `MEASURED BOOT REFUSED`, which cost a
boot on 2026-09-01; `--card` copies all three files as a set for that reason (milestone 217).
`--job-mix` and `--soak` are refused together: both replace the end of the boot tour.

**A job-mix card is sealed like any other.** Since milestone 563 (a seal check that reads bytes
cannot see a check that was dropped), the kernel measures `job_mix_task` against the archive's
table before the sweep. So `NOT SEALED` from `script/board-image` or `script/card-check` means what
it says: the two files are from different builds. A card built before 2026-09-25 still reads `NOT
SEALED` falsely.

### 2. Attach the console, then power on

```sh
script/board-console --for 30m --until none --log bench/radon-$(date -u +%F)/jobmix-boot1.log
```

115200 8N1, a WCH CH343 at `/dev/cu.usbmodem*` on the Mac radon's UART goes into. `--until none`
because this run ends by halting rather than by reaching a stage the console recognises (milestone
324): read the log for `job-mix: done`. The timed windows of the seven-job sweep sum to about 12
seconds on radon (2026-10-04), so most of a boot is U-Boot and the fetch.

### 3. Check the boot is the new instrument before reading any number

The started line must say **`one of 7 kinds`** and the next line **`median of 21 repeats`**. If
either is missing the image is older than this page and the boot is not comparable with anything
this evening is for. Then read the census (`job-mix-census:` lines), which says where the 34
threads landed.

### 4. Boots: at least five, power-cycling between

Five is the floor because it is what 2026-09-16 used and the two evenings should be comparable in
design. Nothing can power-cycle radon remotely (milestone 224); this is a person at the bench.

A jobs-per-minute figure from a single boot is a draw rather than a result: on radon placement alone moves throughput by up to fifteenfold (`notes/soak.md`).

### 5. Clean each log before committing it

```sh
LC_ALL=C tr -cd '\11\12\15\40-\176' < raw.log > bench/radon-<date>/jobmix-bootN.log
```

The console writes bytes that are not UTF-8 under sustained output and `grep` then silently reports
nothing (`notes/bench-runbook.md`'s `BUGS`). Read with `LC_ALL=C grep -a` until they are cleaned.

### 6. What to read, in this order

```sh
cd bench/radon-<date>
grep -ah '^job-mix: tasks=' jobmix-boot*.log            # six points per boot
grep -ah '^job-mix-kind: tasks=32 ' jobmix-boot*.log    # which kinds slowed at the top
grep -ah '^job-mix-kind: tasks=1 ' jobmix-boot1.log     # and what they cost alone
```

1. Is every point a number now? For each `tasks`, compare `jpm_median` across the boots. The
   bar this change was built to clear is **under 10% boot-to-boot at every point**, `tasks=4`
   included (the resampling predicts about 4%). If `tasks=4` is still wide, the median did not fix
   it and the reason is new: record the spread and do not quote the point.
2. Is `ticks_max` far from `ticks_median` on one boot only? One outlier boot is the placement
   lottery (`notes/soak.md`, milestone 240); compare its census with the others'.
3. The shape. Throughput against task count, from the medians. Record where it stops rising,
   and whether anything past that point declines. On 2026-09-16, with the old mix, it rose to
   about 4 tasks and then plateaued without declining through 32.
4. Which kinds pay? For each kind, `per_job` at `tasks=32` over `per_job` at `tasks=1`. Kinds
   that never enter the kernel (`compute`, `touch`) grow only by preemption; a kernel-entering kind
   growing much faster than they do is kernel cost under load. For `map` and `spawn`, subtract
   `region_ticks` to separate the region lock from the rest.

### 7. What the answer means for risk 4

| What the medians show | Reading for `design/fatal-risks/README.md` risk 4 |
|---|---|
| rises to the core count, then flat or rising through 32, every point within 10% across boots | **no architectural per-crossing cost visible at this scale on this silicon**; the risk's decisive experiment ran and the defense held, within the caveats in the Warton section above |
| a knee followed by a **decline**, repeatable across boots | a cost exists and grows with load; the `job-mix-kind:` lines say which path. Milestone 188 (the IPC fastpath) is the follow-on if it is `round_trip` |
| points still wider than 10% across boots | not a verdict; record the spread and say which point failed |

**Who writes the verdict.** The risk's entry in `design/fatal-risks/README.md` is edited from these numbers
by whoever holds that file, not by a lane and not from QEMU.

### 8. Record it

A row in this page's Results table, the row in `notes/register-of-measures.md`, and milestone 168's
block: date, boots, commit, the six medians from one representative boot plus the boot-to-boot
spread per point, and the census summary.

## What each outcome means

| What the log shows | What it means | Where it routes |
|---|---|---|
| `job-mix: FAILED: no 'job_mix_task' program in the initrd archive` | an archive built before this milestone (a mismatched pair halts earlier, at `MEASURED BOOT REFUSED`) | rebuild with `script/board-image --job-mix ...`, which packs the archive before the kernel for exactly this reason |
| `job-mix: FAILED: could not spawn task N of 32` | the board ran out of memory or thread slots partway through building the pool | a real finding: `job_mix::MAX_TASKS` is 32 against `sched::MAX_THREADS`'s 256, so this is memory. Record N and reduce `MAX_TASKS` |
| `job-mix: FAILED: could not create task N's 25-page budget` | the kernel could not carve a task's untyped region | memory again, before any subrun; the same routing as the line above |
| `job-mix: FAILED: task N could not finish a map job` (or `spawn`) | the kernel refused a verb inside the job; the error is printed | a budget constant is too small for this machine (`job_mix::MAP_REGION_PAGES`, `CHILD_PAGES`), since QEMU passes on all three architectures. Record the error and raise the constant |
| the census, then nothing, ever | a task, a server or a child wedged before the first subrun finished | the hang case, and since milestone 324 the tool says so rather than leaving it to the operator: `script/job-mix` and `script/board-console --until sweep-done` both exit **2**. `ROUND_TRIP` blocks on a server and `SPAWN` on a child; either wedged looks exactly like this |
| `jpm_median` roughly flat across the whole sweep | this machine's scheduling is not the bottleneck at 32 tasks | **the honest negative**, and it is a result: see step 7 |
| `jpm_median` rising and then falling, with a knee | throughput collapsing under task count | the positive result. Record where the knee is, which `job-mix-kind:` lines grew, and compare against milestone 134's E1 knee (8 to 11% by 64 to 96 threads on the dev Mac) |
| `jpm_median` varying more between boots than across the sweep | the placement lottery dominates | not a result about §96 at all. More boots, and read `notes/soak.md`'s milestone 240 section |
| `job-mix: done` and six clean points | the sweep ran | exit **0**, and step 8 |

## Results

| Date | Machine | Boots | Instrument | Census summary | jpm at 1 / 32 | Notes |
|---|---|---|---|---|---|---|
| 2026-09-16 | radon (4 harts, 4 MHz `rdtime`) | 5 | five-job mix, **best of 3** | 32 tasks and 2 servers over 4 cores, 6 to 10 threads a core | 319,013 to 319,072 / 1,032,586 to 1,060,264 | shape solid (1.96x at 2 tasks, plateau past 8, no decline at 32); `tasks=4` spread 29.4% across boots, so **not a number**. Transcripts `bench/radon-2026-09-16/jobmix-boot*.log`; the per-point table is in `design/roadmap/0168-multitasking-benchmark/radon-2026-09-16.md` |
| 2026-10-04 | radon, `04a8f9e6c` | 5 | seven-job mix, median of 21 | 34 threads over 4 cores; boot 5 placed differently, read the same | 323,951 to 324,033 / 897,375 to 901,122 | every point a number (widest spread 1.67%, `tasks=4` 1.12%); 2.62x at 4 tasks, rising to 2.78x at 32, no decline. Step 7's first row. Transcripts `bench/radon-2026-10-04/jobmix-boot*.log` |

The 2026-10-04 evening, read in step 6's order with its step 7 judgment, is
[its own page](job-mix/radon-2026-10-04.md). It checked this page's prediction that the median would
read about 1.73x at two tasks where the best of three had read 1.96x: it read 1.76x.

## A cross-check on the Apple cores

A cross-check of the curve's shape under QEMU with Hypervisor.framework on patagonia was attempted
on 2026-09-19. It produced no numbers: QEMU refused HVF with a GICv2 on all five attempts. It could
never have decided anything for §96, because macOS schedules the vCPUs under the guest.
`script/job-mix --hvf --release --smp 4` is the command for it, and the full mix completes there
since 2026-10-05. The attempt is in
[job-mix/hvf-cross-check-2026-09-19.md](job-mix/hvf-cross-check-2026-09-19.md).

## What this instrument cannot settle, said plainly

It does not compare a process kernel against an event kernel, and it cannot, because there is no
event kernel here to compare against. It measures what *this* kernel does under multi-tasking load.
That is the input §96 says it is missing, and §96 stays open either way. A flat curve says the cost
is not visible at this scale on this silicon, which is a reason not to spend a pervasive rewrite. A
knee says there is something to price, and pricing it would then want the rewrite prototyped on one
path rather than the whole kernel.

## BUGS

Each entry is in full in [job-mix/limitations.md](job-mix/limitations.md).

- The sweep has no wall-clock heartbeat, so a watcher's wedge timer is a guess with headroom
  (sixty seconds by default; `--quiet-after 0` gives up the detection).
- The trap still slows a little with busy cores.
- There is no committed baseline and no `--check`, because nothing here is deterministic.
- The sweep tops out at 32 tasks. If the knee is above 32, this instrument will not see it.
- The pool is built once and released in slices, where AIM7 forks a fresh set per subrun.
- The mix proportions are chosen, not derived. Say which mix produced a number when quoting it.
- The repeat count was sized from the five-job mix's distribution. The 2026-10-04 evening showed
  21 was enough on radon.
- The supervisor is in the kernel. If a userspace one is ever built, the Results table needs a
  column saying which supervisor produced a row.
- This page has not been followed end to end by its author.

