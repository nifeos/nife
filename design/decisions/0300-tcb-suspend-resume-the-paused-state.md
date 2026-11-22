---
status: DECIDED
raised: 2026-10-15
decided: 2026-10-15
ratified_by: calef
---

# 300. `ThreadControlBlock::SUSPEND` and `ThreadControlBlock::RESUME`: the paused state, a TCB verb, silent

Raised by [milestone 652 (twelve `DECISION` gates cite only sections that are already ruled)](../roadmap/0652-twelve-decision-gates-cite-only-ruled-sections.md). The fork is
[§24 (interrupting the foreground)](0024-interrupting-the-foreground.md)'s own deferral, older
than the block that waits on it. Milestone 48 (job control: `jobs`, `wait`, `kill`, `fg`, `bg`, and
a stopped state) has gated on it since it was recorded. §24's trigger list in
`design/open-design-ideas.md` names real job control as trigger 2, and the shell's job-control work
has fired it.

## The decision

The kernel gains two verbs on the `ThreadControlBlock`, `SUSPEND` and `RESUME`, and nothing else.

- **Authority is the TCB capability and nothing narrower**, seL4's answer. It has more force here
  than there. Every spawner in this tree holds the TCB it built through `CAP_INSERT` and START,
  and [§142 (what a spawner retains over a child)](0142-spawn-retention.md)'s retention field
  exists to say when a spawner keeps it past START.
- **Suspend is silent.** No message to the supervision endpoint, no event code, no new wire. §26
  stays the channel for *unbidden* news, and suspension is always *bidden*: the kernel never
  self-suspends a thread, some capability holder orders it, and the orderer observes it by acting.
- Resume continues at the PC. Registers are preserved; the thread runs on from where it
  stopped, seL4's `restart()` behavior, not a re-run.

Two rules ride with the decision, so the door stays open without paying for it now:

1. **Unknown event codes are never acted on.** `crates/moho_abi/src/fault.rs` carries
   `EVENT_FAULT = 1` and `EVENT_EXIT = 2`, and every consumer dispatches on those known codes. A
   supervisor that acted on a code it did not know would reap a merely-paused child as dead. This
   rule is what makes a stopped event *additive later* rather than a migration across every
   supervisor in the tree.
2. **The stopped event has a precondition.** A `EVENT_STOPPED` code joins §26's message only when
   a consumer exists that must observe a stop it did not order. Today's consumers are all
   actor-equals-observer: the shell suspends for `Ctrl-Z`, a debugger suspends to inspect, a pager
   suspends to page.

## What this tree already does in the analogous case

§24 built the interrupt design this extends, and it is two tiers: the cooperative tier is a message
on an endpoint the child was spawned holding, and the forcible tier is
[§16 (object revocation)](0016-object-revocation.md). Pause is
deliberately absent, deferred "for a consumer that does not exist yet". One now exists: the shell
wants `Ctrl-Z`, `bg` and a stopped state, and a stopped job that reads the foreground terminal has
to stop rather than be killed or warned.

[§26 (the fault endpoint)](0026-fault-endpoint.md) is the analogous mechanism, and its news is
pushed because it is unbidden: `(event code, tid, fault pc, fault address, reserved)`, delivered
whether or not anyone ordered the death. The kernel's TCB surface today is `CONFIGURE`,
`CAP_INSERT`, `START` and `SET_THREAD_POINTER`, all refusing anything but an embryo. Nothing can
park a started thread and let it continue, and §26's message cannot express "stopped" either: it
has an event code for fault and exit and no paused value.

Prior art, read rather than recalled: `notes/blocked-thread-teardown/prior-art.md` records seL4's
answer, and the part this decision follows is not only the verb. seL4 pushes thread-state news
too, as fault IPC to a fault-handler endpoint, but only for unbidden changes; a deliberate
`seL4_TCB_Suspend` into `ThreadState_Inactive` tells nobody, because the suspender is the
witness. Fuchsia reaches the same shape (recalled and marked as such): `zx_task_suspend` is a
silent handle verb, debuggers poll, and the one pushed signal is termination. Unix's `SIGSTOP` is
the other teacher, and §24 already refused its lesson: signals are ambient authority delivered by
pid, which is the model this kernel does not have.

## The options, and why the silent verb wins

| | shape | what it costs |
|---|---|---|
| **A** | `ThreadControlBlock::SUSPEND`/`ThreadControlBlock::RESUME`, authority the TCB capability, no message. Chosen. | The smallest surface, and seL4's exact shape. A supervisor holding only the supervision endpoint (the §26 shape, and [§32 (a supervisor may collect a corpse)](0032-reap-without-build.md)'s reaper) cannot see or cause the pause. |
| **B** | The same verbs, and suspend delivers a stopped event on the supervision endpoint, the fault endpoint's own channel, so one vocabulary covers "the kernel turned a thread's state into a message". | A third event code on a wire two programs agree on, a policy rule for what a supervisor may do on receiving it (nothing, which is a smell: a notification nobody can act on), and a migration tail across every consumer that dispatches on known codes. The observer it serves, a supervisor that is not the suspender, names no consumer in the tree. |
| **C** | Endpoint-only: suspend as a message on the supervision endpoint that the kernel acts on, no TCB method. | New endpoint semantics on the wire two programs already agree on, and it inverts §26's direction, where the kernel is the sender. A supervisor that holds no endpoint, as `build_child`'s callers sometimes do not, cannot pause at all. |
| **D** | Refuse until a debugger exists; build job control's phase one, which needs no kernel surface. | Leaves trigger 2 fired and unanswered, and the shell simulates `Ctrl-Z` by killing, which is the old behavior wearing a new key. |

The effort test, stated because the skill asks: would B still be chosen if it cost the same as A?
No. B costs more, so the question is whether its extra buys anything. It buys news for an observer
who does not exist. The block's "one surface with the fault endpoint" phrasing is read as conceptual: design suspend in §26's frame rather than ad hoc. A satisfies it. The kernel
witnesses the state change, the authority is named, and the endpoint keeps the news that is
actually news.

This is a system-call-surface fork, decided rather than recommended: the surface verbs are the one
irreversible part, and they are now named.

## Whether the premise is true

Checked 2026-10-15. `kernel/src/sched` has no paused or suspended thread state; the states are the
runnable and blocked set, `Finished` and `Dead`. The SUSPEND tracker in
`design/open-design-ideas.md` still carries the deferral with its three triggers, and job control
is the fired one. Nothing in the tree has grown a way to stop a thread resumably since §24
deferred it.

## What is blocked until consumers arrive

Nothing waits on the verbs but milestone 48's phase two: the stopped state, `Ctrl-Z`, `bg` on a
stopped job, and zsh's `suspend`. Phase one (`jobs`, `wait`, `kill`, `&`, `fg` on a running job)
needs no kernel surface and stays startable. The other two triggers in §24's list, the userspace
pager and a debugger, inherit the verbs without waiting for it: the pair is designed for all
three rather than for job control alone.
