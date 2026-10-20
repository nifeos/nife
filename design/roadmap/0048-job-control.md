---
status: BUILT
raised: 2026-07-31
built: 2026-10-18
milestone_dependencies: 47
decision_dependencies: 300, 309
machine_requirements: none
specific_machine: none
needs_person: no
---
# 48. Job control: `jobs`, `wait`, `kill`, `fg`, `bg`, and a stopped state

**BUILT, 2026-10-18 (UTC).** For supervised jobs, a trailing `&`, `jobs`, `wait`, `kill`, `fg`,
`bg` and `Ctrl-Z` work at a real prompt, proved by `script/swish-check` on all three architectures.
[§300 (the paused state)](../decisions/0300-tcb-suspend-resume-the-paused-state.md) gives the
kernel `ThreadControlBlock::SUSPEND` and `RESUME`: they ride the TCB capability, resume continues
at the PC, and [§26 (the fault endpoint)](../decisions/0026-fault-endpoint.md) carries no stopped
event. [§309 (a supervised job's `ThreadControlBlock` goes to the shell that asked for
it)](../decisions/0309-a-supervised-jobs-tcb-goes-to-the-shell.md) puts that capability in the
shell's hands.

Why it matters. Unix job control is one of the most intricate things in a kernel: sessions, a
controlling terminal, process groups, `tcsetpgrp`, and `SIGTSTP`/`SIGCONT`/`SIGTTIN`/`SIGTTOU`. Most
of that machinery exists to answer one question (*who may read the keyboard*), and here that
question answers itself.

## A job is what the shell holds capabilities for

Structural rather than conventional. Unix's process group is a *number* with inherited, mutable
membership; "what I hold" cannot drift. What the shell holds depends on how the job was built:

- A supervised job (`interrupt_heeder` and its kind) is built by the progenitor out of a region
  the shell split off its own budget, and shares a job frame with the shell. The shell holds the
  region, so `DESTROY` tears the job down, and the frame, whose flags interrupt it and say it is
  done. That is everything job control needs short of pausing.
- Every other job is built in the progenitor's pool. The progenitor deletes its TCB at `START`
  ([§142 (what a spawner retains over a child after
  `START`)](../decisions/0142-spawn-retention.md)), it answers on the one result rendezvous every
  job shares, and `job_undertaker` holds its supervision endpoint. The shell holds nothing for it
  after the spawn, so it cannot wait on it, stop it, or keep its answer apart from the next
  command's.

So phase one covers supervised jobs, and `&` on any other program is refused with a sentence.

## Phase one: built, for supervised jobs

- `<command> &`: the line's last unquoted byte, on a line of one command
  (`swish::sequence::background`). The job starts and the prompt comes back as `[N] name running
  in the background`.
- `jobs`: each held job, running or done. A done job is reported once and its region reclaimed.
- `wait [%N]`: until that job, or every job, is done, reported as a foreground job's finish would
  be. `^C` stops the waiting and leaves the jobs running.
- `kill %N` sets the job frame's interrupt flag, [§24 (interrupting the
  foreground)](../decisions/0024-interrupting-the-foreground.md)'s cooperative tier; `kill -9 %N`
  is its forcible tier, `DESTROY` of the region.
- `fg [%N]`: the shell watches the job, the newest by default, under the same `^C` escalation a
  foreground start gets.

Foreground versus background is not a capability transfer, and the reason is in the tree: no
spawned program holds terminal input. The progenitor grants a child the terminal's sink, which
writes, and never the line discipline's `READLINE`. So `fg` has nothing to revoke. A foreground
job is one the shell is watching, with the terminal out of raw mode so its line discipline counts
`^C`; at the prompt a `^C` is a byte to the line editor and reaches no background job. A
background job that wanted to read the terminal would find no capability to read with.

The swish-check lines are in `xtask/src/swish_check/script/interrupts.rs`. The parsing is
host-tested in `grant_plan` (`job_operand`, `kill_operands`) and `swish::sequence`.

### BUGS

- `&` refuses every program but a supervised job, for the reason above.
- A finished background job is reported when `jobs`, `wait` or `fg` next looks, not before the
  prompt, because the shell is blocked reading the line.
- Background regions come off the shell's budget in start order, so finishing jobs out of order
  leaves a hole until the later ones go (`notes/object-revocation.md`).

## Phase two: the stopped state

Only `Ctrl-Z`, `bg` on a stopped job, and zsh's `suspend` need pausing a thread resumably. §24's
tiers are notify and kill, with pause deliberately absent. §300 adds `SUSPEND` and `RESUME`,
silent, designed for all three of §24's triggers (job control, a userspace pager, and a debugger)
rather than for job control alone. `notes/suspend-and-resume.md` has the kernel's cases.

- The progenitor builds a supervised job with `Retention::Requester` and sends its TCB, `WRITE`
  without `GRANT`, as the one message after `SPAWN_OK` (§309). The shell keeps it with the job.
- The line discipline counts `^Z` as it counts `^C` (`OPERATION_SUSPEND_COUNT` beside
  `OPERATION_INTRCOUNT`). While the shell watches a foreground job, an advance suspends the job,
  and the prompt comes back with `[N] name stopped`.
- `bg [%N]` resumes a stopped job without watching it; `fg [%N]` resumes it and watches.
- `kill %N` on a stopped job sets the flag and resumes it so it can see the request, which is
  bash's order; `wait` does not wait for a stopped job.
- A program is never told. `c_library` still ignores `SIGTSTP` and `SIGCONT`, because no signal
  arrives on either side of a silent stop.

### BUGS

- zsh's `suspend`, the shell stopping itself, is not built: nothing holds the shell's own TCB.

## Follow-on

- **Done.** The kernel verbs, by nifeos/nife#2331, and `Ctrl-Z` and `bg`, by nifeos/nife#2332.
- **Recorded.** `&` on anything but a supervised job is refused, in this block's BUGS and in
  `components/src/swish/jobs.rs`.
- **Recorded.** zsh's `suspend` is not built, in this block's BUGS.
- **Milestone 933.** Job control for every program, in milestone 933 (every job is built the way a
  supervised job is): `design/roadmap/0933-every-job-is-built-the-way-a-supervised-job-is.md`.

## The open question: `disown`

If the shell drops its capabilities to a job, nobody can reap it, and §26's dead-until-reaped means the
corpse persists. Unix reparents orphans to init and lets init reap them; here reparenting means
transferring the supervision endpoint, which is an explicit act rather than a rule nobody thinks
about. **Decided as DECISIONS §40**: a supervisor's death is its subtree's death, because a child's
resources come from its supervisor's region and §16's revocation reclaims the whole subtree in one act.
So `disown` means **transfer supervision upward**, not "abandon". §40 (a supervisor's death is its
subtree's death) records the hole that makes
the cascade close to the only coherent answer. §32 (a supervisor may collect a corpse) authorizes
reaping by matching the
child's recorded `fault_ep`, which nobody can satisfy once the supervisor's endpoint is gone.

## Index row

most of it needs no new kernel surface, and the tty's most tangled feature turns out to be a
capability transfer
