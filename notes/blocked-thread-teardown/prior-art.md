# Prior art: how seven systems end a blocked thread

*Name: provisional. Named after the parent's own heading.*

The survey behind the table in
[`notes/blocked-thread-teardown.md`](../blocked-thread-teardown.md). The table itself stays there.
Seven systems, in the order that makes the argument. Every quote below was fetched and read in this
lane on 2026-08-17; where a claim is from memory rather than a source, it says so.

## seL4: suspension is cancellation, and the authority is a TCB capability

The closest relative, and the most instructive, because seL4 does not have a separate "abort an IPC"
operation at all. It has `seL4_TCB_Suspend`, and cancellation falls out of it. From the kernel
source, verbatim:

```c
void suspend(tcb_t *target)
{
    cancelIPC(target);
    if (thread_state_get_tsType(target->tcbState) == ThreadState_Running) {
        updateRestartPC(target);
    }
    tcbSchedDequeue(target);
#ifdef CONFIG_KERNEL_MCS
    tcbReleaseRemove(target);
    schedContext_cancelYieldTo(target);
#endif
    setThreadState(target, ThreadState_Inactive);
}
```

Source: [`seL4/src/kernel/thread.c`](https://raw.githubusercontent.com/seL4/seL4/master/src/kernel/thread.c)

`cancelIPC` is the closed enumeration, and it is the same enumeration this kernel has:

```c
switch (thread_state_ptr_get_tsType(state)) {
case ThreadState_BlockedOnSend:
case ThreadState_BlockedOnReceive: {
    endpoint_t *epptr = EP_PTR(thread_state_ptr_get_blockingObject(state));
    assert(endpoint_ptr_get_state(epptr) != EPState_Idle);
    tcbEPDequeue(tptr, epptr);
    ...
    setThreadState(tptr, ThreadState_Inactive);
    break;
}
case ThreadState_BlockedOnNotification:
    cancelSignal(tptr, NTFN_PTR(thread_state_ptr_get_blockingObject(state)));
    break;
case ThreadState_BlockedOnReply: {
#ifdef CONFIG_KERNEL_MCS
    reply_remove_tcb(tptr);
#else
    cte_t *slot = TCB_PTR_CTE_PTR(tptr, tcbReply);
    cte_t *callerCap = CTE_PTR(mdb_node_get_mdbNext(slot->cteMDBNode));
    if (callerCap) {
        cteDeleteOne(callerCap);
    }
#endif
    break;
}
}
```

Source: [`seL4/src/object/endpoint.c`](https://raw.githubusercontent.com/seL4/seL4/master/src/object/endpoint.c)

Four things to take from this, and the last is the one that changes the shape of the question.

The authority is a TCB capability and nothing narrower. `seL4_TCB_Suspend` "Suspend a thread",
capability required: "seL4_TCB capability to the target thread"
([API reference](https://docs.sel4.systems/projects/sel4/api-doc.html)). There is no suspend right, no
kill right, and no way to reach a thread you were not handed a TCB cap for. Holding a TCB cap is
holding the thread.

Cancellation is not a message; it is a state. The suspended thread is `ThreadState_Inactive`. It
does not resume, does not return an error to userspace, and does not run another user instruction. A
later `seL4_TCB_Resume` calls `restart()`, which sets `ThreadState_Restart` and re-executes the
interrupted syscall from the top. seL4 never hands a cancelled thread an error code, so it never
has to define one, and userspace never has to handle one.

The reply relationship is a first-class object with a back-pointer. Under MCS a `reply_t` holds
`replyTCB`, so finalizing a reply capability finds the blocked caller:

```c
case cap_reply_cap:
    if (final) {
        reply_t *reply = REPLY_PTR(cap_reply_cap_get_capReplyPtr(cap));
        if (reply && reply->replyTCB) {
            tcb_t *tcb = reply->replyTCB;
            switch (thread_state_get_tsType(tcb->tcbState)) {
            case ThreadState_BlockedOnReply:
                reply_remove(reply, tcb);
                break;
            case ThreadState_BlockedOnReceive:
                cancelIPC(tcb);
                break;
            default:
                fail("Invalid tcb state");
            }
        }
    }
```

Source: [`seL4/src/object/objecttype.c`](https://raw.githubusercontent.com/seL4/seL4/master/src/object/objecttype.c)

and `reply_unlink` ends it the same way suspension does:

```c
/* Unlink a reply from its tcb */
static inline void reply_unlink(reply_t *reply, tcb_t *tcb)
{
    ...
    reply->replyTCB = NULL;
    /* This means the value of the thread state reply reference no longer matters. */
    setThreadState(tcb, ThreadState_Inactive);
}
```

Source: [`seL4/include/object/reply.h`](https://raw.githubusercontent.com/seL4/seL4/master/include/object/reply.h)

So seL4 solves nife's stranded-caller problem structurally rather than by adding an operation.
Destroying the reply object *is* freeing the caller, in both configurations: MCS through
`replyTCB`, non-MCS through the mapping database that links the caller's `tcbReply` slot to the
server's `callerCap`. In nife the `Reply` capability is a bare `(tid)` with no back-link and no
bookkeeping in the caller, so there is nothing to destroy and nothing to find.

## Mach: the distinction between abort and abort-safely is exactly this hazard

Mach has two operations and the difference between them is the whole design question.

> The function `thread_abort` aborts page faults and any message primitive calls in use by
> target_thread [...] `thread_abort` will abort any non-atomic operation [...] at an arbitrary point
> in a non-restartable way.
>
> Source: [`thread_abort`](https://web.mit.edu/darwin/src/modules/xnu/osfmk/man/thread_abort.html), XNU
> `osfmk` man pages

with an explicit caution not to use it on a non-suspended thread, because it is very difficult to
know what system trap the thread might be executing. Against that:

> The basic purpose of `thread_abort_safely` is to let one thread cleanly stop another thread [...]
> when `thread_abort_safely` returns (if successful), the target thread will appear to have just
> returned from the kernel [...] `thread_abort_safely` will not abort any non-atomic operation [...]
> but will return an error instead.
>
> Source: [`thread_abort_safely`](https://web.mit.edu/darwin/src/modules/xnu/osfmk/man/thread_abort_safely.html)

`KERN_FAILURE` means "The thread is in the middle of a non-restartable operation."

The lesson for nife is sharper than it looks. Mach's split is between an abort that can leave the
victim's own invariants broken and one that refuses rather than do so, and it needed both because
Mach threads block inside arbitrary kernel work. This kernel's blocked threads are all parked at one
of three well-defined points, with no kernel state in flight, so every abort here is
`thread_abort_safely`-shaped and the dangerous variant has no reason to exist. That is a real
advantage of a microkernel with four syscalls, and it is worth stating out loud rather than
assuming.

## L4 (Fiasco.OC / L4Re): cancellation as a side effect of touching registers

L4 has no abort verb either. It has `ex_regs`, whose stated job is setting a thread's instruction and
stack pointer, and cancellation is a *flag* on it:

> L4_THREAD_EX_REGS_CANCEL = 0x10000UL, "Cancel ongoing IPC in the thread"
> L4_THREAD_EX_REGS_TRIGGER_EXCEPTION = 0x20000UL, "Trigger artificial exception in thread"
>
> "This method allows to manipulate and start a thread. The basic functionality is to set the
> instruction pointer and the stack pointer of a thread. [...] Additionally, this method allows also
> to cancel ongoing IPC operations and to force the thread to raise an artificial exception (see
> flags). If the thread is in an IPC operation or if L4_THREAD_EX_REGS_TRIGGER_EXCEPTION forces an
> IPC then changes in IP and SP take effect directly after returning from this IPC."
>
> Source: [L4Re, `l4_thread_ex_regs`](https://l4re.org/doc/group__l4__thread__api.html)

Unlike seL4, L4 *does* hand the victim an error, and it distinguishes the two sides:

> "If, for instance, the IPC was aborted using L4::Thread::ex_regs(), the sender gets an
> L4_IPC_SECANCELED error while the receiver gets an L4_IPC_RECANCELED error."
>
> Source: [L4Re, *L4 Inter-Process Communication (IPC)*](https://l4re.org/doc/l4re_concepts_ipc.html)

with a separate `SEABORTED`/`REABORTED` pair in the error table
([error codes](https://l4re.org/doc/group__l4__ipc__err__api.html)). The documentation does not say
what distinguishes cancel from abort, and it uses "aborted" in the sentence explaining the
*canceled* codes, which is a caution about inheriting a vocabulary: two words that a reader cannot
tell apart are worse than one.

The shape worth taking seriously is the third one on the list: L4 also has IPC timeouts, so an L4
thread need never block forever in the first place. That is a different answer to the same problem
and it is not one the milestone 23 lane sketched. It is milestone 106's fork.

## Fuchsia / Zircon: they had this, and they removed it

The most valuable entry, because Fuchsia shipped thread killing, lived with it, and deleted it.

> "In the past, `zx_task_kill` allowed usermode to kill individual threads. However, killing
> individual threads encourages bad practices and has a high chance of leaving the process in a bad
> state." [...] "There is no reasonable use for usermode to kill individual threads. Exposing such
> facility encourages bad practices."
>
> The dangers listed: "Locks can be left acquired, including global locks like ones controlling the
> heap." "Memory can be leaked. At the very least the thread stack, but often many other pieces."
> "Runtime left in an inconsistent state." "Killing a thread in its way to a syscall leaves the
> process in an unknown state." "Defeats RAII wrappers and automatic cleanup."
>
> Source: [RFC-0007: Zircon removal of thread killing](https://fuchsia.dev/fuchsia-src/contribute/governance/rfcs/0007_remove_thread_killing)

The current syscall documentation is the outcome:

> "This asynchronously kills the given process or job and its children recursively, until the entire
> task tree rooted at handle is dead. Killing a thread is not supported." Rights: "_handle_ must
> have `ZX_RIGHT_DESTROY`." Errors: "`ZX_ERR_NOT_SUPPORTED` handle is a thread handle."
>
> Source: [`zx_task_kill`](https://fuchsia.dev/fuchsia-src/reference/syscalls/task_kill)

Zircon's answer to a thread blocked in `zx_channel_call` is therefore not to touch the thread. It
closes the channel and lets the call fail:

> `ZX_ERR_PEER_CLOSED`, "The other side of the channel was closed or became closed while waiting for
> the reply."
> `ZX_ERR_CANCELED`, "_handle_ was invalidated (e.g., closed) while waiting for a reply."
>
> Source: [`zx_channel_call`](https://fuchsia.dev/reference/syscalls/channel_call)

That is precisely case (b) in the hung-component taxonomy: destroy the object the thread is blocked
on and the block resolves itself. Zircon can always do this because a channel handle is an ordinary
object with an owner, and killing the *process* closes all of its handles. The granularity at which
Zircon expresses "may end this" is the task, and the authority is a right on the object's own
handle, which is the closest match in the survey to how this tree thinks.

Zircon also documents the stale-transaction problem in its own vocabulary:

> "If a `zx_channel_call()` returns due to `ZX_ERR_TIMED_OUT`, if the server eventually replies, at
> some point in the future, the reply _could_ match another outbound request [...] This syscall is
> designed around the expectation that timeouts are generally fatal and clients do not expect to
> continue communications on a channel that is timing out."

Two systems, independently, documenting the hazard this note found in `ipc_reply`.

## QNX Neutrino: the client is unblocked by the server's death, and told so

QNX is the industrial synchronous-IPC system and its state machine is nife's, renamed:

> "If the client thread calls MsgSend(), and the server thread hasn't yet called MsgReceive(), then
> the client thread becomes SEND blocked." [...] "Once the server thread calls MsgReceive(), the
> kernel changes the client thread's state to be REPLY blocked, which means that server thread has
> received the message and now must reply." [...] "If the server thread fails, exits, or
> disappears, the client thread becomes READY, with MsgSend() indicating an error."
>
> Source: [QNX Neutrino, *Synchronous message passing*](http://www.qnx.com/developers/docs/7.0.0/com.qnx.doc.neutrino.sys_arch/topic/ipc_Sync_messaging.html)

The error is `ESRCH`, documented on `MsgSend` as "the server died while the calling thread was
SEND-blocked or REPLY-blocked"
([`MsgSend()`](https://www.qnx.com/developers/docs/6.4.1/neutrino/lib_ref/m/msgsend.html)).

This is the entry that most directly indicted the state of this kernel when the note was written.
QNX unblocks a REPLY-blocked client when its server dies. nife did not, because `Error::Gone`
reached only endpoint wait queues and a reply-parked caller left the queue at the rendezvous. A hung
*or dead* server stranded its callers here in a way QNX has not since the 1990s. Milestone 254
closed the dead half on 2026-09-04; the hung half (a server alive and simply not replying) is
still open and is milestone 133's and 106's.

## Linux: the third state had to be invented, and the reason was social

Included for contrast, and the contrast is that Linux could not fix this by fiat.

> "If the expected wakeup event does not materialize, the process will wait forever and there is
> usually nothing that anybody can do about it short of rebooting the system. This is the source of
> the dreaded, unkillable process which is shown to be in the 'D' state by ps." [...] "Matthew
> created a new sleeping state, called TASK_KILLABLE; it behaves like TASK_UNINTERRUPTIBLE with the
> exception that fatal signals will interrupt the sleep."
>
> On why the existing interruptible sleep could not simply be used: "Unix tradition (and thus almost
> all applications) believe file store writes to be non signal interruptible. It would not be safe or
> practical to change that guarantee."
>
> Source: [LWN, *TASK_KILLABLE*](https://lwn.net/Articles/288056/) (merged for 2.6.25)

Two lessons, and neither is "copy this".

A third state was needed because two were a false choice. `TASK_INTERRUPTIBLE` means every wait
site must handle `-EINTR` and every application must expect it; `TASK_UNINTERRUPTIBLE` means nobody
handles anything and the process is unkillable. `TASK_KILLABLE` splits the difference by making the
wait breakable *only* by something that ends the process anyway, so no correct program ever
observes the interruption, and therefore no wait site has to be rewritten to handle it.

That is directly transferable. An abort that only ever precedes teardown costs userspace nothing; an
abort that returns an error a program is expected to survive costs every `CALL` site in the tree.

And the constraint that forced it was compatibility, which this project does not have. Linux
could not change what a blocking write means because applications depend on it. nife has 54 user
programs, all in this tree. The window in which this decision is cheap is now.

## Windows NT: the wait opts in

A third shape again, and the only one where the *victim* has a say. An NT wait is alertable or it is
not; a user APC is delivered only to a thread waiting alertably, and such a wait completes with
`STATUS_USER_APC`
([`KeWaitForSingleObject`](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-kewaitforsingleobject),
[Waits and APCs](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/waits-and-apcs)).
A non-alertable wait is not interruptible by an APC at all.

Read as a design, this is `TASK_KILLABLE` with the choice moved from the kernel's wait site to the
caller's argument. It is the shape that fits worst here: an opt-in that a hung component chooses
for itself is exactly the cooperation a hang is defined by not giving. It is included because the
*inversion* is worth seeing, and because it is where an "interruptible `CALL`" design would end up if
the flag were the caller's rather than the destroyer's.
