---
status: BUILT
raised: 2026-09-27
built: 2026-10-06
promoted_from: a-job-is-finished-when-its-memory-is-back
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 685. A job is finished when its memory is back

*Title and slug are drafts.*
Raised by milestone 205 (how a foreign program is told what to do) (#1402). The slug and every
name below are provisional. Every real fix is a change two programs agree on, so it is calef's.

## The ruling

calef, 2026-10-06 (UTC): "A looks right." Option A below, with B, C and D refused for the reasons
in the table. The proof he set, as fatal risk 7's criterion (a) is amended: "claim 24 closed by
milestone 685, proven by a host test of the window-reuse rule (`Windows::take` never hands out a
window whose last holder is unreaped) that goes red without the fix, plus a booted test of the
reap protocol on all three ISAs."

## What was built

This closes confinement claim 24 for fatal risk 7 (the confinement claim is false): a
directory-granted job's file-service window is never handed to another job while its last holder
is unreaped.

- The message: `grant_plan::spawnproto::reaped` and `UNDERTAKER_BADGE` (both provisional).
  `job_undertaker` holds a `WRITE` copy of the spawn endpoint at slot 2, badged
  `UNDERTAKER_BADGE` (1), and sends this after every collect, never before:

  ```text
    w0  label   the badge the progenitor put on the job's supervision capability (DECISIONS §148
                (resolves by asking the kernel)); 0 for a job built without one
    w1  tid     the dead thread's id, as the kernel stamped it on the death message
    w2  0       reserved
  ```

  `w0` and `w1` are what a `RECEIVE_CAP` returns as data, so the message reads whole wherever in a
  request it lands. The shell's copy is unbadged and lacks `GRANT`, so it cannot forge the badge.
- The progenitor does not hold each child's tid from `build_child`: `build_child` returns a thread capability, and no method reads a tid
  from one. So the progenitor labels each job's supervision capability with a number it mints,
  and keys on that; the tid rides along as the ruling asked.
- The progenitor. Every receive on the spawn endpoint goes through `SpawnEndpoint`, which takes
  reaped messages out of the way, since one can arrive between any two of a request's messages. A
  reap frees the job's window (`grant_plan::job_windows::Windows`, moved out of
  `system_initializer` so its rule is a host test) and settles any wait for it.
- The pool waits for the event. #1402's 1,024-yield retry is gone. A carve that fails, once
  the request is read to its end, waits for reaps while a finished job's reap is due
  (`grant_plan::job_windows::ReapsDue`). That means a job whose whole answer went to the shell's
  result endpoint, which the shell reads to its end before its next request. A pipeline stage with a sink
  is not due, because it may be waiting on a stage not yet built. A full window pool waits the same
  way, then refuses.
- Images are copied after the request ends. The two image-pool carves used to happen before
  the frames arrived, where a wait would take the shell's next frame instead of a reap. The frames
  are now mapped as they arrive (`take_frames`) and copied once the last message is in
  (`stage_frames`), region first and staging second as before.

Tests:

- `grant_plan::job_windows::tests::take_never_hands_out_a_window_whose_last_holder_is_unreaped`,
  host. Red on the pool as it was (commit "grant_plan: model the file-service window pool"), and
  red again under its replayable falsification (take skips the reaped check).
- `system_tests::user::job_undertaker_tests::job_undertaker_says_which_job_it_reaped_and_only_then_is_its_window_free`,
  booted, on aarch64, riscv64 and x86_64 in CI. Green on aarch64 locally on 2026-10-06 (UTC), and
  red under its replayable falsification (the undertaker forgets the label) on aarch64.

## BUGS

- **A job that answers and then does not exit holds a short pool's wait** (`SpawnEndpoint`'s
  `BUGS`). Before, that request was refused after 1,024 yields; now it waits, and `^C` cannot
  reach it.
- **A screen-narrowed job behind a directory grant would keep its window for the life of the
  boot**, since the shell reaps it, not the undertaker. No manifest asks for both today.
- **A second holder of the spawn endpoint** gets one request kept while the progenitor waits, and
  the request in hand refused. Only the boot shell holds it.
- **No boot drives seven concurrent directory-granted jobs through the real progenitor**; the
  booted test runs the protocol and the pool, not the progenitor's loop.
- The booted test's "region back when the message arrives" assertion is the message's meaning,
  but no falsification reaches it on one core: an undertaker announcing first would still collect
  before the test thread ran.

## Follow-on

- **Recorded.** The wait that a job which answers and never exits can hold, and the one-request
  stash for a second holder: `SpawnEndpoint`'s `BUGS` in `crates/system_initializer/src/lib.rs`.
- **Recorded.** A screen-narrowed job behind a directory grant keeping its window: the `BUGS` in
  `crates/grant_plan/src/job_windows.rs`.
- **Recorded.** No boot drives seven concurrent directory-granted jobs through the real
  progenitor; this block's `BUGS` above says so.
- **Done.** #1418's window reuse, by the same message (`Windows::reaped`).

## The problem, measured

A job's region goes back to the progenitor's pool when `job_undertaker` reaps it. The shell shows
the next prompt when it has read the job's output, and nothing orders the two. So a job typed at
once can find the pool still holding the last one. The pool holds one `std`-sized share
(`JOBS_BUDGET_PAGES` in `crates/system_initializer`), so two `std` jobs in a row are where it shows.

CI hit it on 2026-09-27 on #1402, aarch64: `/installed/std-grep needle`, typed right after
`/installed/std-grep needle docs`, answered "could not spawn (the progenitor is out of memory)".
A riscv64 failure earlier that day, on a pipeline typed after a plain `std_exerciser`, fits the
same race; that one was inferred, not measured.

What #1402 ships is a stopgap: the progenitor yields up to 1,024 times and retries before it
answers "out of memory" (`split_job`). That is a timing guess. Whether 1,024 yields cover one reap
depends on what else is runnable, and a guess that holds on a quiet runner may not on a loaded one.
It is kept only until one of the options below is ruled.

## A second consumer

Lane 606's #1418 (milestone 599 (a frame per filesystem client channel)'s window pool) hit the same
missing signal from the other side. The progenitor is never told a job has died, so it hands out
file-server windows round-robin, and an eighth concurrent job shares a window. It needs to know
which job was reaped, after its memory is back, to free that job's window. So the fix should carry
a job identity the progenitor already holds: the child's thread id, which `build_child` returns and
the undertaker's death message carries.

## What already carries it, checked

Nothing does. The undertaker holds two capabilities: `DEATHS`, the supervision endpoint it receives
on, and `REPORT`, the result endpoint the shell reads (`components/src/job_undertaker.rs`). It holds
nothing that reaches the progenitor. A notification (§101 (notification objects)) ORs bits and
cannot carry a thread id, or count. A badge (milestone 599) names a sender, not a job. So every
option below gives the undertaker one new capability and gives some message a new meaning, which is
a change two programs agree on.

## The options

| | the carrier | failure mode |
|---|---|---|
| **A. A badged send on the spawn endpoint** | The undertaker gets a `WRITE` copy of the spawn endpoint, badged. After each collect it sends "reaped, tid". The progenitor's loop tells it from a request by the badge, notes the job gone (and, for #1418, frees its window). A pool split that fails while the progenitor has a job not yet reaped receives on the spawn endpoint until that reap arrives, then retries once | While the progenitor waits, a request from another holder of the spawn endpoint could arrive first. Today the boot shell is the only holder and it is parked in its own request, so nothing can; a second holder would need the wait to set requests aside |
| **B. A notification plus a shared page** | The undertaker writes reaped tids into a small ring on a page and signals a notification the progenitor binds to its spawn receive (`notification::BOUND`) | Two contracts, the ring's layout and the signal, where A has one message |
| **C. The shell waits for the reap** | The undertaker sends a word on the result endpoint after every collect, and the shell reads one per job before it prompts | Does nothing for #1418, which needs A or B anyway. Every shell spawn path must read exactly one word per job; one too few leaves a stale word, one too many hangs the prompt |
| **D. Keep the retry** (#1402 ships this) | Nothing | A pool the reaper would have refilled answers "out of memory" when the runner is busy. A timing guess |

Recommendation: A. It is one message on an endpoint the progenitor already serves. It carries
the thread id #1418 needs, and it makes the pool wait for the event it is waiting for rather than
for a number of yields. C would be the more honest prompt on its own, "the prompt came back" meaning
"the job's resources came back", but #1418 needs A or B regardless, so C plus A is more machinery
than A alone. The prompt can adopt C's promise later on top of A, since A is what tells the system
a job's memory is back. D is chosen on effort, and 205's `BUGS` says so.

## The seven questions

1. Considered and refused. B, C and D above, each with its reason. A caller-side sleep, because
   there is no timed wait at the prompt (§147 (a timer a userspace service cannot hold) is not wired
   to the shell). Folding the undertaker into the progenitor, because the undertaker exists so that
   a parked report cannot stall spawns (`components/src/job_undertaker.rs`, `report`).
2. What the tree does already. The undertaker already sends `JOB_FAULTED`, after collecting, on
   the result endpoint; A sends its sibling to the progenitor instead. Milestone 599 built badged
   endpoints for telling senders apart on one endpoint, which is A's discriminator. Jobs the shell
   supervises itself (`spawnproto::Wiring::screen`, §106 (the `terminal_sink_caretaker`
   narrowing)) and interruptible jobs are not reaped by the undertaker, so A's count must leave them
   out.
3. Prior art, from memory and not re-read. Unix delivers `SIGCHLD` to the parent and a
   `waitpid` names the child, so the parent learns which process ended. seL4 leaves it to the
   user-level manager, which is this tree's shape: the undertaker is that manager's reaping half.
4. Is the premise true? Yes, measured once in CI on 2026-09-27 (#1402, aarch64), with an
   earlier riscv64 failure that fits it, and #1418 found the missing signal independently. A larger pool moves the line and does not
   order the reap.
5. Cost. A: one badged capability in the undertaker's table, one message kind in `spawnproto`
   (constants there, which both programs already link), a per-job table or count in the progenitor,
   and a receive-until-reaped on a failed split.
6. Reversibility. Inside the tree; no outside program has acted on it. It changes
   `spawnproto`, which the shell, the progenitor and now the undertaker read.
7. At equal cost? A, still, because it serves both consumers with one mechanism.

## What is blocked on the answer

Answered 2026-10-06 (UTC). #1402's retry is replaced, and #1418's window reuse is fixed by the same message.

## Index row

The shell shows the next prompt when it has read a job's output, but the job's memory returns when `job_undertaker` reaps it, and nothing orders the two. Proposed: a job is finished only when its memory is back; every real fix changes something two programs agree on.
