# Milestone 168 appendix: what changed on 2026-09-19, the two holes closed without a board

An appendix to [milestone 168 (a multi-tasking workload benchmark)](../0168-multitasking-benchmark.md),
moved out of that page on 2026-10-12 (UTC) under [§212 (a prose
budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it. This is the lane of 2026-09-19 that changed the instrument between the two radon
readings. It holds the evidence for the new statistic, the two jobs added, what the lane found on the
way, the HVF cross-check that could not run, and the two proposals it absorbed.

A lane on patagonia, with no hardware, closed the two things that stopped the five-boot result of
2026-09-16 ([radon-2026-09-16.md](radon-2026-09-16.md)) carrying a verdict for
`design/fatal-risks/README.md`'s risk 4. Neither changes what a bench evening does beyond reading
different lines; `notes/job-mix.md` has the rewritten procedure.

## The first hole: every point is the median of 21, not the best of 3

The evidence comes from the five boots' own repeats. The lane pooled the fifteen repeats each sweep
point produced and resampled them: five simulated boots of N repeats, 2,000 trials, the reported
statistic's boot-to-boot spread averaged. `tasks=4` is the worst case:

| statistic | N = 3 | N = 5 | N = 9 | N = 15 | N = 21 | N = 31 |
|---|---|---|---|---|---|---|
| best of N, `tasks=4` | 21.9% | 19.1% | 13.7% | 8.7% | 6.0% | 3.3% |
| median of N, `tasks=4` | 16.5% | 11.9% | 7.6% | 5.1% | 4.2% | 3.3% |
| median of N, `tasks=8` | 7.9% | 6.5% | 5.0% | 4.0% | 3.4% | 2.9% |
| median of N, `tasks=32` | 5.4% | 4.8% | 3.9% | 3.1% | 2.6% | 2.0% |

The simulated best-of-3 (21.9%) is in the neighborhood of the observed 29.4%, which is what makes
the rest of the table worth reading. The best-of rows flatter themselves at large N: a resample
can never go below the fifteen samples' own minimum, where real repeats would keep finding luckier
ones. That artifact is exactly why the minimum is the wrong statistic for a wide distribution, and
the median has no such floor to hide behind.

Why the median and not the best, beyond the numbers. The best-of rule is `kernel/src/bench.rs`'s,
and it is right for a micro-benchmark on a shared host, where everything above the minimum is
somebody else's load. Here the spread is the tasks' own contention, the thing the instrument exists
to measure. It changes a reading: radon's `tasks=2` repeats are bimodal (five near 98,000 ticks, ten
near 111,000). Best-of-3 reported the fast mode on every boot. The median sits in the slow one,
about 1.73x over one task where the 2026-09-16 table reads 1.96x.

Why 21 and not a table per point. The proposal this absorbs (below) wanted repeats to vary by
point because board time was thought to be the cost. It is not: at 21 repeats the timed windows are
about eleven seconds at the old mix's rates, against a boot that takes minutes. One constant is one
fewer thing for a reader to hold, and 21 is odd so the median is a sample. The TCG rehearsal is what
grew (riscv64 from 20 seconds to 188), and a gate pays that, not a person.

The printed line changed, on purpose so nothing reads it silently. `ticks=` and `jpm=` no longer
occur on a point line. It now carries `repeats=`, `ticks_min=`, `ticks_median=`, `ticks_max=` and
`jpm_median=`. A new line after the census says which statistic is in use and that transcripts
before 2026-09-19 are not comparable. `notes/job-mix.md` has the old-and-new table. Only
`cargo xtask job-mix` reads these lines in-tree, and it counts points by the unchanged
`job-mix: tasks=` prefix; `crates/serial_console` has no recogniser for this run (milestone 324).

## The second hole: page mapping and process creation are in the mix

`job_mix::MAP` splits a region from the task's own budget, builds an address space and a frame in
it, maps the frame at 32 addresses and `DESTROY`s the lot. `job_mix::SPAWN` builds two children from
EL0 (address space, stack, thread, the shared child code), `RECEIVE`s each one's exit and reclaims its
region. No syscall, method or object type was added. Each task is granted an untyped budget
(`memory_region_cap`) and a child-done endpoint through the existing spawn grants. That is the shape
`kernel/src/bench.rs`'s `map_el0` and `spawn_el0` already had. The proposal worried that a map job
"needs the spawn path to hand a task a capability on its own address space"; that dissolved once the
task could create a space of its own. The mix is still sixteen jobs a round: two of each non-IPC kind
and four round trips, where it was three of each of four kinds and four round trips.

The allocator worry was measured rather than assumed. The proposal refused a spawn job because at 32
tasks it "would measure the memory-region allocator rather than the scheduler". Each task now times
its `SPLIT` and `DESTROY` calls, and the supervisor prints them per kind on a new `job-mix-kind:` line
beside each point, with the kind's total ticks and job count, collected after the clock stops. Under
TCG on aarch64 (a proof the accounting works, not a finding): region calls were 35% of a map job at
one task and 20% at 32, 27% of a spawn job at one task and 9% at 32. So under emulation the share
falls with load, because waiting for a child to be scheduled grows faster than the lock. Whether
that holds on radon is one of the things the next evening reads.

The disk-file mix was not built, and not for effort. Nothing nife runs on radon can read a disk
(every file-service path starts at a virtio block device), so it could only ever produce QEMU
numbers. And the file service maps one channel into every client, which 32 concurrent tasks would
race on. Both, with options and no recommendation (the second is a wire format), are in
`design/roadmap/0493-a-disk-file-job-mix-needs-a-disk-radon-can-drive.md`.

## Smaller things found on the way

- `script/job-mix --arch riscv64` failed on any fresh checkout (exit 3, before the kernel printed
  a line): the riscv64 runner refuses a missing `NIFE_DISK` and only the aarch64 arm built one. It
  passed on 2026-09-04 because an aarch64 run had come first. Fixed in `xtask`.
- The hand-assembled child both spawn loops run moved to `user_mode_runtime::child_stub`, from a
  private constant in `os_primitives_benchmarker`, rather than being copied. Bytes unchanged.
- The supervisor now fails loudly on a refused job: a task reports `REPORT_FAILED` with the error
  and the kind, and the run ends at `job-mix: FAILED` rather than printing ticks for work not done.

## The HVF cross-check calef asked for could not run

On 2026-09-19 calef asked for the sweep under HVF on patagonia's M3 cores as a cross-check on the
shape (never a result). `script/job-mix` gained `--hvf` and `--release`, spelled as `run` and
`bench` already spell them. Five attempts on an otherwise idle host were each refused by QEMU 11.1.1
before the kernel ran, `HVF does not support GICv2 emulation`, because `kernel/src/drivers/gic.rs` is
GICv2 only. Milestone 227 (GICv3 driver) is what gives this cross-check a machine. Transcript
`bench/patagonia-hvf-2026-09-19/jobmix-hvf-refused.log`; `notes/job-mix.md` has the section.

## Two proposals absorbed, and closed as the milestones they had become

They were deleted by the lane that absorbed them and restored as numbered blocks at merge, both on
2026-09-19. Another session had promoted them hours earlier under the drain of milestone 433 (drain the proposal pile to zero), and the two
sessions could not see each other. calef's ruling there decides it: a proposal is promoted and then
closed, since a numbered block marked BUILT is a record and a deleted file is nothing. So each is
`BUILT` rather than absent, with what it bought and what it did not.

- [Milestone 382 (three AIM7's)](../0382-three-aim7-job-categories-the-job-mix-does-not-have.md)
  (filed 2026-09-04): three AIM7 categories missing, why each was refused, and the order to add them
  (map, spawn, then disk as a second mix). Its content is "The second hole" above, and the refusals
  it recorded are answered there. The map job did not need a new capability, the spawn job's
  allocator share is now printed, and the disk mix has its own proposal.
- [Milestone 419 (`tasks=4` needs)](../0419-more-repeats-where-the-job-mix-contends.md) (filed
  2026-09-16): `tasks=4` needs more repeats, not more power cycles, with four options and a
  recommendation of reporting the spread. What was built is its options 1 and 3 together (a uniform
  21, and the median with the ends beside it). Its option 2, a per-point table, was refused above on
  measured board time. It marked itself `Gate: DECISION` because the line format changes. The
  maintainer's brief for this lane assigned the change, both readers are in-tree, and it is recorded
  here so the decision is visible rather than implied. That gate had a written-up section by the end
  of the same day, [§191](../../decisions/0191-job-mix-repeats-and-what-the-line-reports.md), from
  milestone 435 (gated on a decision nobody wrote down), a sweep of forty-five blocks whose `DECISION` gate named no decision. calef ratified
  what shipped on 2026-09-26 (*"Yes, ratify §191 as shipped."*), so §191 (whether the job mix
  reports the spread rather than the best) is `DECIDED`.
