---
status: BUILT
raised: 2026-09-02
built: 2026-09-03
---
# 235. A command that faults hangs the prompt, because nothing has a word for it

Built. Milestones 231 (nothing counts how many capability slots a boot actually uses) and 233
(`login` dies on every boot) found it. They were proving their own gate could fail.

In brief. A spawned command that traps hangs the prompt. `swish` waits on the job's result
endpoint and a killed thread never sends, so the shell waits forever. An ordinary non-zero exit is
handled correctly; it is specifically a fault that has no path back.

Found by deliberate experiment rather than by accident: milestone 233's lane patched `worker` to trap
in order to prove its new no-thread-killed assertion could actually go red, and the prompt hung.

## Why it is worth its own block

The pieces already exist. DECISIONS §26 (the fault endpoint: thread death becomes a message a
supervisor holds) means somebody already learns that the thread died. What is missing is a word: `grant_plan::spawnproto` has no way to say
*this job faulted* as distinct from *this job has not answered yet*, so the knowledge cannot travel
from the supervisor to the shell that is waiting.

That makes this a protocol gap rather than a bug in `swish`, and it is the kind that gets patched
locally by whoever meets it next unless it is written down.

## What was built

`grant_plan::spawnproto::JOB_FAULTED`, a third value on a read that already carried two.
`job_undertaker` gained one capability (`WRITE`, no `GRANT`, on init's result endpoint) and sends
that word once, after collecting the corpse and only for `abi::fault::EVENT_FAULT`. `swish`
recognizes it in the four places it reads that endpoint (an answer, a byte stream being printed, a
byte stream going into a file, a pipeline stage's ack). It prints `swish::FAULTED_SENTENCE` and sets
`Status::Failed`; `$?` reads 1 and `&&` stops.

**The design question this block left open was which of three couplings**, and the answer is the
middle one: the supervisor tells. The other two lose on properties this tree already recorded, and
the argument is at `JOB_FAULTED` itself, in notes/supervision.md, and in one line each here. The
shell asking needs a poll interval, because the ABI has no non-blocking receive, and a poll
interval cannot tell a slow job from a dead one. **The endpoint carrying the death** works for the
fault and breaks the ordinary path. DECISIONS §26 (the fault endpoint: thread death becomes a message
a supervisor holds) flows clean exits down the same endpoint. Every ordinary job would leave a
second message behind its answer, and every job would leave init's supervision domain, which is what
`ps` and `pgrep` read.

Nothing here touches the system call surface. No new system call, no new method, no new argument: one
userspace constant, one capability in an endowment, and four call sites.

Proven the way the defect was found, on both architectures. `components/src/least_authority_demo.rs` was patched to
trap on argument 6 and `script/swish-check` run against it. Before: `worker 6` killed the thread and
"the prompt never came back to take `worker 7`". After, on aarch64 and riscv64 alike, the transcript
reads the kernel's own report of the killed thread, then `that command faulted and was killed before
it answered`, then `echo $?` answering `1`, then `worker 7` answering `7*7 = 49` and the rest of the
script running to its end. The scaffold was then removed and `script/swish-check` is green on both
legs.

## BUGS

- The hang is worse than it looks in a test. A test harness times out and reports; a person at a
  prompt sees nothing and has no way to know whether the command is slow or dead.
- Nothing says how common this is. Every program in the tree that faults under a shell hits it,
  and nobody has counted which ones can.
- **This block does not cover a job that hangs without faulting**, which is a different problem with
  the same symptom and no obvious answer at all. It is untouched: a live thread blocked in a `RECV`
  nobody will answer is not dead, so there is no death message to route and none of the three
  couplings had anything to say about it.
- A fault reported while the shell is watching a screen-narrowed tail arrives one command late.
  The report is an ordinary rendezvous `SEND` and the ABI has no non-blocking form, so it completes
  only when someone reads the result endpoint; a screen-narrowed line (DECISIONS §106) leaves nobody
  reading there. `job_undertaker`'s own `BUGS` carries the mechanism, and the collect happens before
  the report so a parked report can never cost the reclamation the prompt's memory depends on.
  Closing it needs a non-blocking send or a receive that waits on two endpoints, both of which are
  the system call surface (DECISIONS §10 (process model: capability-based, microkernel) and §16 (object revocation: reclaim the objects a
  process built)) and therefore calef's.
- **There is no regression gate, and that is a real gap.** Proving this needs a command that faults
  on purpose, which is a new program and therefore a name calef decides, and milestone 233's
  no-thread-killed assertion in `script/swish-check` would have to learn to except it. Proposed as
  its own milestone rather than smuggled in here.

## Follow-on

- **Recorded.** `design/roadmap/0235-a-faulted-job-should-reach-the-prompt.md` BUGS: a job that hangs
  without faulting has the same symptom and no answer. A live thread blocked in a receive nobody
  will answer is not dead, so there is no death message to route and none of the three couplings had
  anything to say about it.
- **Recorded.** `components/src/job_undertaker.rs` carries the mechanism in its own BUGS: a fault reported
  while the shell is watching a screen-narrowed tail arrives one command late, because the report is
  an ordinary rendezvous send and nobody is reading the result endpoint. The collect happens before
  the report, so a parked report can never cost the reclamation the prompt's memory depends on.
- **Refused.** The two couplings that lost. The shell asking needs a poll interval, because the ABI
  has no non-blocking receive, and a poll interval cannot tell a slow job from a dead one. Flowing
  the death down the fault endpoint works for the fault and breaks the ordinary path. Every
  clean exit would leave a second message behind its answer, and every job would leave init's
  supervision domain, which is what `ps` and `pgrep` read.
- **Milestone 330.** A regression gate
  for the fault path. It needs a program that faults on purpose, which is a new name and therefore
  calef's. Milestone 233 (`login` dies on every boot, and the boot says it is
  ready) and its no-thread-killed assertion in `script/swish-check` has to learn to
  except it. The scaffold that proved this milestone was a patch to `components/src/least_authority_demo.rs` and was
  removed afterwards, so nothing stops the lost prompt returning.
- **Milestone 339.** Count which of the tree's
  programs can fault under a shell, so the exposure of this defect class is a number rather than
  a guess. Every program that can fault hit this path and nobody has looked, so there is no way to
  say whether the fix mattered to two programs or to forty.

## Index row

`spawnproto::JOB_FAULTED`: `job_undertaker` already held the death, and now has one word and one `WRITE` to say it with
