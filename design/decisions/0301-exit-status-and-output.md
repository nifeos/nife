---
status: DECIDED
raised: 2026-10-15
decided: 2026-10-15
ratified_by: calef
---

# 301. How a spawned child's exit status and output reach the program that spawned it

Raised by [milestone 652 (twelve `DECISION` gates cite only sections that are already ruled)](../roadmap/0652-twelve-decision-gates-cite-only-ruled-sections.md). The fork belongs to
milestone 172 (a capability-native subprocess primitive), whose gate has named it
since the block was written. [§219 (how the shell names an installed program to the
spawner)](0219-naming-an-installed-program-to-the-spawner.md) decided how a caller *starts* a child.
[§264 (fork is declined, and spawn is supported)](0264-fork-is-declined-and-spawn-is-supported.md)
declined `fork` and made spawn the supported path. What neither settles is the other half of
`cargo`'s loop, "run
this, block until it is done, give me its output".

## The decision

**`SYSCALL_EXIT` carries one status word, and the kernel forwards it in the `EVENT_EXIT` message.**
(`SYSCALL_EXIT` is syscall 0, its name spelled out by the ruling of 2026-11-11 (UTC).) The pieces,
each already in the tree:

- `SYS_EXIT` exists and takes six argument words, all discarded today
  (`crates/user_mode_runtime/src/process.rs` calls it as `trap6(abi::SYS_EXIT, [0; 6])`). Word
  one becomes the exit status.
- The exit departure stamps zeros where a fault stamps `pc` and `addr`
  (`kernel/src/sched/departure.rs`). The exit status travels in one of those words, so
  [§26 (the fault endpoint)](0026-fault-endpoint.md)'s message shape is unchanged:
  `(event code, tid, status, 0, reserved)` for an exit,
  `(event code, tid, pc, addr, reserved)` for a fault. Event code and status never compete: a
  child that dies without stating a code is a fault, and `EVENT_FAULT` is the whole report, the
  same encoding `WIFSIGNALED` makes on Unix.
- The wait is the §26 receive. The parent receives the death message on the supervision endpoint
  it chose at spawn, reads the status from it, then reaps
  ([§32 (a supervisor may collect a corpse)](0032-reap-without-build.md)). This is the handle to
  wait on that [§264 (fork is declined, and spawn is supported)](0264-fork-is-declined-and-spawn-is-supported.md)
  promised: the death message *is* its news.
- Output stays grant-routed, the one half with no contested prior art: on every system, the
  parent builds the channels before the child runs, and the child inherits ends. Here the parent
  grants frames or endpoints at spawn, the way [§219 (how the shell names an installed program to the
  spawner)](0219-naming-an-installed-program-to-the-spawner.md)'s request already carries grants.
- The status word's meanings (zero is success, what each nonzero class means) live in a crate
  beside the spawn protocol, so the two sides cannot drift (rule 7).

No new syscall, no new object, no blocking verb. What the decision commits is semantic: two
words that were reserved-zero on existing wires now carry the code. That is the decide-once
category, which is why it was a fork and not a lane's ride.

## What this tree already does in the analogous case

`crates/supervision_protocol`'s `build_child` constructs the child and `start_child` runs it; the
crate holds the report and construction protocol words, and its own doc points the supervisor who
wants a corpse at [§32 (a supervisor may collect a corpse)](0032-reap-without-build.md)'s `REAP`.
Death arrives as §26's five words on the fault endpoint,
distinguished as fault or exit by the event code. Nothing in the kernel or the crates carries an
exit status: the event code says *that* a thread exited, never *how*, and `SYS_EXIT`'s argument
words are thrown away.

The composition the block named is real and remains the fallback for anything this decision does
not cover. [§101 (notification objects)](0101-notification-objects.md) support `POLL`, a non-blocking
wait, and can be bound to a TCB so a signal ends a blocked receive;
[§12 (call/reply IPC)](0012-call-reply-ipc.md)'s `CALL` mints a one-shot
reply capability; §26's death message is never lost.

Prior art, read and recalled: the near-universal shape is child-states-code-to-kernel, kernel
stores it in the corpse, parent reads it after the wait. Unix and Plan 9 carry it in `waitpid`
(Plan 9 carries a status string). Windows carries it from `ExitProcess` to `GetExitCodeProcess`
after a handle wait. Fuchsia (recalled) takes `zx_process_exit(return_code)` and answers
`zx_info_process` after `ZX_TASK_TERMINATED`. All make the kernel the carrier, because the exit
path is the one moment a child may not be able to write memory reliably. On output, every system
routes through channels the parent built before spawn. This tree's difference from all of them is
that the parent owns frames by construction, which is why grant-routed output was never contested
and why a status-in-a-frame was defensible at all. The ruling keeps the kernel in the loop
anyway, for the art's reason: the status is where every parent already has to look.

## The options

| | shape | what it costs |
|---|---|---|
| **A** | The child's runtime writes one status word to a frame the parent owns before exiting; the parent receives §26's death message, reads the word, then reaps. | No kernel change, and the status is a convention the kernel never sees. It is the one shape here where the kernel is not told the code, and the exit path is the one moment a child may not be able to write memory. |
| **A′** | `SYSCALL_EXIT` carries the status word; the kernel forwards it in the `EVENT_EXIT` message's currently-zero words. Chosen. | A semantic commitment of words that were reserved-zero on two existing wires, and one kernel touch on the exit path. No new surface, no second channel, and the status arrives where the death already does. |
| **B** | `ThreadControlBlock::WAIT`, a blocking method on the TCB, returning an exit status the kernel records at exit. | New syscall surface, and §26's message stays the supervisor's channel while the status travels on another one: two ways to learn about one death. |
| **C** | A job object: spawn returns one, `Job::WAIT` collects status and the granted output ends, `Job::DESTROY` reaps. | The most machinery: a new object kind, retype and slot costs, and a second lifetime beside the region and the endpoint, exactly the bookkeeping [§142 (what a spawner retains over a child)](0142-spawn-retention.md)'s decision just finished paying down for the TCB. |

The effort test, stated because the skill asks: would A′ still be chosen if it cost the same as
A? Yes. A′ puts the status where the art puts it and where the parent is already blocked, and it
keeps one death channel.

## Whether the premise is true

Checked 2026-10-15. `crates/supervision_protocol` has no wait-and-collect shape, and its death
protocol words carry no status. `SYS_EXIT` discards its arguments. The §26 message is the whole
record of a death. The block's sizing note stands confirmed from the other direction: the
composition and the kernel-carried shape turn out to share everything but where the word rides.

## How reversible it is

The commitment is the meaning of words that exist on both sides of an existing call. Renaming or
re-sloting them before a consumer ships is cheap; after `cargo` reads them, they are ABI. The
status vocabulary (what zero means, what nonzero classes mean) lives in one crate and is
replaceable until a second consumer relies on it.

## What is blocked until consumers arrive

Nothing waits on this decision now that it is made: milestone 172 is startable, and 173
(`cargo`/`rustc` self-hosting) behind it, whose build model runs this loop per compilation unit.
The `posix_spawn` shim [§264 (fork is declined, and spawn is supported)](0264-fork-is-declined-and-spawn-is-supported.md)
promised foreign programs reads its codes from the same death message. The prior art for the
whole loop is the workload itself: spawn with channels already granted, receive the death, read
the code.
