---
status: PARTIAL
promoted_from: every-job-is-built-the-way-a-supervised-job-is
raised: 2026-10-18
milestone_dependencies: 48
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 933. Every job is built the way a supervised job is

Raised by milestone 48 (job control)'s phase one, which could give `&`, `wait`, `kill` and `fg`
only to supervised jobs. Name: provisional.

Reuse: the tree's own supervised-job path (`spawn_interruptible` in the progenitor's spawn loop)
and the screen-narrowed tail's shell-held exit endpoint (§106) are the parts to take; no outside
code applies, since the question is this kernel's spawn protocol.

## Why

The progenitor builds a supervised job out of a region the shell holds, so the shell can tear it
down. The job shares a frame with the shell, so the shell can interrupt it and see it finish. The
progenitor built every other job in its own pool. The progenitor deleted its thread control block
at `START` (§142 (what a spawner retains over a child after `START`)), the job answered on the one
result rendezvous every job shares, and `job_undertaker` held its supervision endpoint and reaped
it. The shell held nothing for it, so it could neither wait on it nor stop it.

## Ruling

2026-10-27 (UTC), about 22:30: yes. The shell holds the region of every job it starts and an
endpoint where the job's exit arrives, the shape of [§106 (take the `terminal_sink_caretaker`
narrowing)](../decisions/0106-tail-output-narrowing.md). The shell reaps these jobs. The progenitor
learns that a job's file-service window is free some way other than the undertaker's "reaped"
message. The shell's endowment grows by what the progenitor's pool shrinks. The cooperative tier,
a `std` program's `SIGINT`, stays its own question. The architect weighed a Unix parent shell,
Fuchsia process handles and Genode's parent-paid quotas.

[§142 (what a spawner retains over a child after
`START`)](../decisions/0142-spawn-retention.md) carries the amendment.

## What is built

The shell holds a job from the program table, started by a plain line or by `&`. It does not hold
pipeline stages, a file run by its path, an installer holder or the graphical terminal; BUGS lists
each.

- The request. `Wiring::held` (`crates/grant_plan/src/spawnproto.rs`) puts up to three capabilities
  after the interrupt pair. The first is a region the shell split off its budget, sized by
  `grant_plan::job_region_pages` so both sides count the same pages. The second is the shell's exit
  endpoint. The third, for `free` and `slabtop`, is the shell's budget narrowed to `ENUMERATE`.
- The build. The progenitor builds the job from the shell's region, with the exit endpoint
  badged with the job's label as its supervision capability. A foreground job answers through a
  second copy of the same badged capability, so its output and its death arrive in order on one
  endpoint. A job that faults before it answers ends the read where it would have hung it. A
  background job's output goes to the terminal's sink.
- The answer. One message, `(SPAWN_OK, label, 0)`, then the job's `ThreadControlBlock`
  (§309 (a supervised job's `ThreadControlBlock` goes to the shell that asked for it)). A refusal
  is a sentinel word, and the progenitor reclaims the region.
- The exit. The kernel stamps a dying thread's label on the sixth word of its death message
  and on nothing else, so the shell tells a death from the job's own output by that word. A death
  it hears while asking after another job waits in a table of sixteen.
- Asking without waiting. The ABI has no non-blocking receive. The shell counts the dead
  threads in its domain with `SURVEY`, and receives only when that count is above the deaths it
  has heard, so the receive returns at once.
- The reap. `DESTROY` of the region takes the job, its corpse and anything built beside it, a
  directory grant's caretaker included. `REAP` would return the pages to the shell's budget and
  leave the region carved.
- The release. The shell then sends `spawnproto::release(label)` on the spawn endpoint, before
  its next request. The progenitor frees the job's window, resolver grants and installer reply as
  it does for the undertaker's message, and answers nothing. The shell sends it only when the
  region came back, so a job that is still there keeps its window. The shell sends nothing
  mid-request, so the progenitor never waits for a reap that will not come.
- Job control. `&` keeps a held job in the table; `jobs`, `wait`, `kill -9`, `fg`, `bg` and
  `^Z` act on it. `^C` under `fg` tears it down at once.

## Memory

The numbers are in pages of 4 KiB, measured with `free` at the prompt on all three architectures
(`script/swish-check`), and counted from the constants.

| | before | after |
|---|---|---|
| progenitor's job pool (`JOBS_BUDGET_PAGES`) | 672 | 672 |
| shell's budget (`SHELL_BUDGET_PAGES`) | 1,152 | 1,824 |
| a native job's region | 48, from the pool | 48, from the shell |
| a directory-granted job's region | 98 | 98 |
| a `std` job's region | 384 | 384 |
| `free`'s `Yours:` total | 672 | 1,824 |
| `free`'s `Yours:` used, idle shell plus `free` | 48 | 67 |

The shell's own static pages are 19 (76 KiB): `Yours:` reads 268 KiB with `free` running and
192 KiB of that is `free`. A held job costs the shell 48 pages for its life and nothing after, and
the same 48 came from the pool before, so the cost per job moved and did not grow. After a hundred
spawn-and-reap cycles of `job_sleeper` it reads 576 KiB on both sides of the cycles on aarch64,
riscv64 and x86_64, the same figure to the kibibyte. The boot shell holds six native jobs and a
`std` job at once (`script/swish-check`), the pool's old mix; its budget has 1,824 pages for them.

The pool has not shrunk, and the shell's endowment grew by 672 pages. The ruling says the shell
grows by what the pool shrinks. The pool still serves pipeline stages, and one graphical terminal
session takes 544 of its 672 pages (`GRAPHICAL_TERMINAL_SESSION_PAGES`), so it cannot shrink until
the stages move and the session has another builder. The 672 pages are the same mix the pool
holds (six native regions and one `std` region), so the total grows until the pool follows. The
shell's budget also keeps its 1,024 pages for an image's frames.

## What `ps` shows

Milestone 942 (`ps` and `top` show which process each thread belongs to, and how many threads it has)
groups threads by process within the domain a viewer holds (§223 (the process view is the
supervision domain)). A held job is not in the progenitor's domain, because its death arrives on
the shell's endpoint. So the progenitor gives a held `ps` or `top` the shell's exit endpoint with
`ENUMERATE` as its domain. It lists the jobs this shell holds, itself among them. It does not list
the stages of a pipeline, which stay in the progenitor's domain. A `ps` that is itself a pipeline stage sees the
progenitor's domain and not the held jobs. Milestone 942 groups what a domain holds and adds no
domain, so it does not change this; a viewer of both domains would need a second domain slot.

## Tests

- Host: the wiring flags do not collide with `held` (`the_wiring_flags_do_not_collide`), a release
  names its job and only a release is one, and the region size is one function both sides call.
- `script/swish-check`, all three architectures. A blocked ordinary program and a timed one run in
  the background and appear in `jobs`. The cooperative `kill` refuses with a sentence, and `kill -9`
  tears down the blocked job. `wait` sees the timed one exit. A blocked job comes to the foreground
  and `^C` tears it down. A foreground job ends on its own. A hundred spawn-and-reap cycles leave
  `free`'s `Yours:` row equal on both sides (`held_cycles_failure`).

## BUGS

- The cooperative tier is unbuilt. A held job has no frame to raise a request in, and a `std`
  program's `SIGINT` is its own question (the ruling leaves it open). `kill %N` without `-9`
  refuses with a sentence, and `^C` under `fg` tears the job down at once. The entry is in
  `components/src/swish/jobs.rs`.
- A foreground job that answers and does not exit holds the prompt. The shell reads for its
  death after its output, and `^C` cannot reach a blocked receive. The entry is in
  `components/src/swish/held.rs`.
- A background job's output arrives over the prompt, whenever the job writes it, because it goes to
  the terminal's sink. The shell refuses a program that answers in a word in the background, since
  only the shell can read a word.
- **Pipeline stages, a file run by its path, an installer holder and the graphical terminal stay in
  the progenitor's pool or image pool**, reaped by `job_undertaker`. The progenitor refuses a held request that also sets
  the image bit. The pool and the image pool cannot shrink while they stay.
- A `--mem` budget on a job built in the pool is never returned. The shell deletes its copy
  after delegating, and nothing destroys the untyped, so each such job costs its pages for the
  boot. A hundred held `job_sleeper --mem 4` jobs leaked 4 pages each until the shell kept the
  budget and destroyed it with the job's region; the pool path still has the leak.
- A held job's `--mem` budget comes back only if its first `DESTROY` succeeds. `release_region`
  in `components/src/swish/running.rs` and `release` in `jobs.rs` destroy it once, read nothing
  back and delete the slot. A job that split its budget leaves a sub-region nobody names, and the
  `DESTROY` refuses for good (`HasChildren`). A job that built a process there and exited with it
  running gets a refusal that arms the kill; a retry would succeed, and none comes. Either way the
  pages stay spent for the boot. `system_tests/src/user/memory_grant_reclaim_tests.rs` shows both
  refusals. No program the shell runs builds from its budget today, so it is latent for all of
  them.
- Background jobs finishing out of order leave a hole in the shell's budget until the later ones
  go, the carve order's cost (`notes/object-revocation.md`).
- A held `ps` does not list a pipeline's stages, and the reverse (above).
- The kernel suite's test progenitor cannot reap a scripted shell's held jobs, so each leaves a
  corpse: 157 frames on aarch64, in `SUITE_PAGE_FRAME_BUDGET`. The production path does not.
- A background job holds up to three slots in the shell's capability table (its region, its thread
  control block and its budget), so eight of them take twenty-four of the table's sixty-four. No
  test fills the table.

## Follow-on

- **Outstanding.** Pipeline stages are not held. Each stage would take a region and an exit, and
  the tail's output and every stage's death arrive on the one endpoint. Checked 2026-10-27 against
  `components/src/swish/pipeline.rs`, whose `spawn_stage` still sends `held: false`.
- **Outstanding.** A file run by its path and an installer holder are not held. Their regions come
  from the image pool (`IMAGE_POOL_PAGES`), sized for a package, which the shell's budget would
  have to carry beside its staging frames. Checked 2026-10-27 against
  `crates/system_initializer/src/spawn_loop.rs`, where `answer` refuses a held image request.
- **Outstanding.** The pool and the image pool shrink, and the shell's endowment stops growing,
  only when the two items above are done and the graphical terminal session has another
  builder. Checked 2026-10-27 against `JOBS_BUDGET_PAGES` in
  `crates/system_initializer/src/layout.rs`, still 672.
- **Recorded.** The cooperative tier for a `std` program, in `components/src/swish/jobs.rs`.
- **Recorded.** A `--mem` budget never returned on the pool path, in this block's BUGS.

## Index row

The shell holds the region and the exit of every job it starts from the program table, so `&`, `wait`, `kill -9` and `fg` reach an ordinary program. Pipeline stages, files run by their path and the graphical terminal still use the progenitor's pool and `job_undertaker`. The pool has not yet shrunk.
