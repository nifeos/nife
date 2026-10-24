# Milestone 7e: IPC, and the scheduler learning to wait

*Name: provisional.*

An appendix to [`notes/capabilities.md`](../capabilities.md). It is the record of milestone 7e, which added endpoints.

7d gave a process one thing it could do to a capability: invoke a `Console` and have the kernel
act. 7e adds the capability that lets a process talk to another process, which is the whole
point of a microkernel and the thing milestone 8 cannot happen without.

## An endpoint is a rendezvous

```text
  invoke(cap, SEND, w0, w1, w2)   ->  0     blocks until a receiver takes the message
  invoke(cap, RECEIVE, _,  _,  _)    ->  w0    (with w1 in x1, w2 in x2) blocks until one arrives
```

Synchronous. There is no buffer, no queue of messages, no "mailbox that fills up." A `SEND` and a
`RECEIVE` on the same endpoint meet, the three words move from one thread's registers toward the
other, and both go on their way. If one side arrives first, it waits.

Three words, in registers, and memory is never touched on the way. That is the fastpath, and
it is DECISIONS §10's rule taken literally: *IPC carries control; shared memory carries data.*
Bulk data will move later by handing over a frame capability, not by copying bytes into a message.
The moment we copy a buffer through the kernel we have rebuilt Mach, and Mach was slow.

## Which end you are is a matter of rights, not of the endpoint

`SEND` needs `WRITE`. `RECEIVE` needs `READ`. So the same endpoint, handed to two processes with
opposite rights, is a one-way pipe neither side can run backwards. The client holds a
`WRITE`-only capability: it can send and it *cannot express* receiving. Nobody had to tell it
which end it is. It could not be the other end if it tried.

This is [`capability::Rights`](../../crates/capability/src/lib.rs) doing real work, and it is a distinction Unix
does not have: a pipe fd is a pipe fd.

## The scheduler learned a third thing a thread can be

Threads were `Ready`, `Running`, `Finished`. Now they can be `Blocked`, and blocking is the
substance of this milestone.

A blocked thread is in no run queue. It sits in an endpoint's wait queue instead, and the
thread on the other side of that endpoint is the only thing in the universe that will move it back
to `Ready`. That is what makes "wait for a message" a real thing a thread does, rather than a spin
loop that asks "is it here yet" and burns a core.

And it took one line in `schedule()`, which is the interesting part:

```rust
let runnable = state == Some(State::Running);
// ...only requeue `current` if runnable...
```

`schedule()` can be called from the timer IRQ *while a thread is halfway through blocking itself*:
it has marked itself `Blocked` and joined an endpoint queue but has not yet reached its own
`schedule()` call. The timer must not helpfully requeue it. One equality check is the whole
defense, and a test (`a_receiver_blocks_until_a_sender_arrives`) fails loudly without it: with the
check relaxed to "anything not Finished is runnable", the blocked receiver gets rescheduled and
returns from `ipc_receive` holding a message nobody sent.

## Where the message actually lives

A thread has a `mailbox: [u64; 3]`. It is a `Thread` field and not a stack local because the
two halves of a rendezvous run at different times on different stacks. A sender deposits its
message and blocks, and a receiver, running later, reaches into the *sender's* `Thread` to collect
it (or the reverse). The mailbox is the one place both threads can agree on when only one of them
is running at a time.

For a user thread, the received words have to end up in its EL0 registers. Word 0 rides back the
way every syscall result does, in `x0` (the dispatcher writes it from the return value). Words 1
and 2 the kernel places into the trap frame's `x1`/`x2` directly, because a syscall return is one
register and a message is three. Writing the trap frame is writing the user's registers ([see
7a](../userspace.md)).

## Single core was a gift here, and the bill arrived

Every worry that makes IPC hard on a real machine: a sender and receiver racing on the queue, a
message half-delivered: cannot arise on one core, because only one thread runs at a time. Between a
thread releasing the scheduler lock and calling `schedule()` to block, the *only* thing that can run
there is the timer IRQ, and the one-line `runnable` check handles that. nife runs on many cores
under §11 (SMP).

What it costs now that cores are real is written down rather than left to inference.
[scheduler.md](../scheduler.md) has the per-core run queues and the message-shaped stealing that
replaced the one big lock. [memory-ordering.md](../memory-ordering.md) has which sites rely
on the weak model. [interleaving.md](../interleaving.md) has the protocols loom searches, because
reading them was no longer enough.

## What it looks like

The demo hands a user program a `WRITE`-only endpoint capability in slot 1, and spawns a kernel
thread blocked on the other end:

```text
      hello from EL0, through a capability in slot 0.
      and it refused to read the kernel's memory on my behalf.
      and sent a word to a server through slot 1.

      a server thread received 0x5eed1e55 from a user program that holds
      one capability for it and no other way to find it.
```

## The server was a kernel thread, for one more milestone

*(Milestone 8 has since shipped; this is the record of the last piece of scaffolding and why it
came down in that order. See [userspace-drivers.md](../userspace-drivers.md).)*

The thing on the other end of that rendezvous was a kernel thread. Milestone 8 (the console driver leaves the kernel) made it a userspace process: the console driver leaves the kernel,
`Object::Console` becomes a `Rendezvous` to a console *server*, and `write` becomes an ordinary
`SEND` to it. Everything 7e built (rendezvouses, blocking, the rendezvous guarantee, message-in-registers) is
exactly the machinery that move needs, which is why it came first.
