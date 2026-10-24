# The IPC decision core

*Name: provisional, minted 2026-10-23 (UTC) by comments/929-crates-03.*

`crates/inter_process_communication` holds the decisions the kernel's IPC makes, with no kernel in
them, so they can be proved on the host. It has four parts: the synchronous rendezvous, the
notification, the timer and the futex wait queues. The kernel wraps each one with what the crate
cannot see: mailboxes, run queues, the one-shot Reply, the thread table and the locks. The note
name is provisional.

## The rendezvous

A rendezvous owns two wait queues, senders and receivers, and a count of pending signals. It
decides what a send, a receive or a signal does with them. The scheduler calls it for that
decision rather than writing the same branch at each of its call sites.

The queues are intrusive (`crates/intrusive_fifo`). In the kernel a queue entry is the TCB,
threaded through the same link the run queues use. A blocked thread therefore waits on exactly one
rendezvous, because it has one link, and queueing never allocates.

The invariant everything rests on is that at most one wait queue is ever non-empty. A sender that
finds a receiver meets it instead of queueing, and a receiver that finds a sender collects it, so
a thread queues only when nobody was waiting for it. A receive drains a pending signal before it
collects a sender, so a signal is never lost behind a later send.

### Tokens

The operations take and return `Unqueued` tokens, and none of them is `unsafe`. A token is the
proof that its node is on no queue. A verdict that leaves the caller off a queue hands its token
back, and the one verdict that queues the caller keeps it. A token comes from one of three places:
a pop, a removal, or thread creation.

In the kernel, `me` is always the running thread. Its token sits on its own TCB
(`Thread::own_token` in `kernel/src/thread.rs`) until a send or receive takes it, and a returned
token goes back there through `sched::hold_token`. A thread queued on a rendezvous is `Blocked`,
and the reaper never frees a `Blocked` thread. Suppose a verdict handed back the partner's token in
the caller's place. The running thread would then hold a token for a thread its waker is about to
queue. That is two tokens for one node, the double queueing the token exists to rule out.

The verdict enums compare and print by hand rather than by derive. A derive would demand
`T: PartialEq` and `T: Debug`, the kernel's TCB is neither, and only the pointer is compared.

### A rendezvous bound to an interrupt

§101 (notification objects), as amended on 2026-09-26, rules that an endpoint bound to an
interrupt refuses every send. `bind_to_interrupt` sets a flag that is never cleared, and `send`
then answers `Send::Refused` and touches nothing. So the only thing a receiver there can be handed
is a signal, and an interrupt's `w0 = 1` cannot be forged by anyone who holds a `WRITE`
capability. The flag sits in the padding after the pending count, so the object did not grow.

The refusal is tested before a receiver is popped. The receiver on an interrupt's rendezvous is
the driver, and handing it a sender's words is the forgery. Binding has no inverse because the
kernel has none: `bind_irq` in `kernel/src/sched/interrupt_route.rs` only ever overwrites a route.
A rendezvous that once carried an interrupt is safer left refusing than reopened to senders a
driver does not expect.

`Refused` is a variant rather than a check at each call site. Every kernel path that deposits into
a rendezvous must then say what it does with it: `SEND`, `SEND_CAP`, `CALL`, and the death message
of §26 (the fault endpoint). A fifth path cannot forget.

### Taking a thread back off a queue

An intrusive `Fifo` offers no arbitrary remove, and the rendezvous needs one in two cases.

A corpse can be a queued sender. A supervised thread that dies with nobody in `RECEIVE` parks on
its supervision rendezvous's sender queue, its death message in its mailbox. That is item 2 of
§26's "Implementation" section: delivery reuses the synchronous-send rendezvous, and the corpse is
the parked sender. Its supervisor may
then reap it without collecting the message, through §32 (a supervisor may collect a corpse
without being able to build one) or the `DESTROY` of §16 (object revocation). Freeing a TCB that is still linked leaves a
dangling pointer the next receive would follow, so the reap unlinks it first with
`remove_sender`.

A receiver can need removing too. Milestone 133 (ending a permanently blocked thread, and deciding
who may) has `MemoryRegion::DESTROY` end a resident thread that is permanently `Blocked`. The
commonest such thread is a server parked in `RECEIVE` on a rendezvous that belongs to somebody
else. Its TCB is linked there, so the reclaim unlinks it with `remove_receiver` before freeing the
page the TCB sits on.

Both removes drain the queue and push the survivors back, rather than adding a remove to `Fifo`.
That keeps the queue's own contract, one link and no arbitrary remove, and its proved invariants
are the only ones in play. The cost is linear in the queue's length, on a teardown path, and FIFO
order among the survivors is kept. Both are safe: the victim is compared by pointer and never
dereferenced, and every other node goes back with the token its own pop handed over.

They are two methods rather than one taking a queue. A single method would make the caller name
the queue, which means trusting the victim's recorded `WaitRole`. The kernel records a `CALL`
caller that met no server as a `Reply`, and that caller is on the sender queue, so the role is a
diagnostic rather than the fact. The kernel asks both queues and believes neither.

## The notification

A notification is the asynchronous half of IPC, built by milestone 151 (notification objects:
async multiplexing without wait-any) to §101's rules. A rendezvous is a meeting: a sender blocks
until a receiver takes its words. A notification is a doorbell: a signaler ORs bits into a word
and never blocks, and a waiter takes whatever has accumulated. Two signals before anyone waits
are one wake carrying both sets of bits. A notification says what happened since you last looked,
not how many times. The word is a bitfield of binary semaphores, as seL4's is.

It lives in this crate because it is the same kind of object one type over: an intrusive wait
queue of TCBs and a decision core the kernel wraps. Its proofs have the same shape, so they share
the crate's row in `script/verify`.

Its invariant is the rendezvous's, restated for a word: a non-zero word and a non-empty wait queue
never coexist. A waiter queues only when the word is zero, and a signal that finds a waiter hands
its bits straight to it.

§101's binding makes a signal wake a thread blocked receiving on an endpoint. That is state this
crate cannot see, since it is the kernel's thread table and another object's queue. So `signal`
takes `bound_receiving`, which says the bound thread is parked in a receive right now, and answers
`Signal::ToBound` when the kernel should deliver there. The proofs here show no bit is lost on any
of the three paths. That the kernel computes `bound_receiving` correctly is argued at its call
site, `signal_locked` in `kernel/src/sched/notification.rs`. Why a signal of no bits wakes nobody
is in [notification-objects.md](notification-objects.md).

## The timer

The timer module is the arithmetic of §147 (a timer a userspace service cannot hold), built by
milestone 106 (a wait that ends on either the interrupt or the deadline). The kernel owns the
comparator on every architecture and signals a notification at the deadline on the holder's behalf.
[timed-wait.md](timed-wait.md) priced what that needs from the scheduler. It needs a deadline word
per timer, one cached earliest deadline, and a walk that runs only when the tick finds the counter
at or past the cache. The module is those three things. How the kernel uses them is in
[timer.md](timer.md).

The invariant is on the cache, not on a timer: the cached earliest is never later than any armed
deadline. It may be earlier, because a cancel does not raise it (raising it would take a walk),
and that costs one walk that fires nothing. A later cache would be a missed expiry nothing ever
notices, and it is the one bug this structure can have. The cache is lowered under the same lock
as the arm, so anyone holding it sees the cache and the timers agree.

A deadline at or before `now` is not armed. `arm` answers `Arm::DueNow` and the caller signals at
once. Otherwise a deadline that passed between the caller reading the clock and the kernel taking
the lock would still fire, but a tick late.

The walk fires one due timer per call and the kernel signals it before calling again. That is the
rescan shape the kernel's region sweeps use: no buffer of fired targets, so no bound on how many
can fire in one tick.

## The futex queues

A futex is a key, an address space and a virtual address, not an object. That keeps `std`'s
`Mutex::new` `const` and free of syscalls. The module is `B` buckets of intrusive FIFOs, a key
hashed to a bucket, and oldest waiter first within a key. Keys can collide in a bucket, so every
lookup asks the caller each waiter's key, which is the kernel's own record. The kernel side is in
[futex.md](futex.md).

## How it is proved

The harnesses are Kani proofs, listed in [verification/core-harnesses.md](verification/core-harnesses.md).
They are inductive steps: assume an arbitrary valid state, apply one operation, check. A
non-empty queue is modeled with a single waiter, because every decision and both invariants
depend only on whether a queue is empty. An operation pops, which only shrinks a queue, and pushes
only to a queue that was empty. So the pattern of empty and non-empty queues changes the same way
for one waiter or many. FIFO order within a queue is `intrusive_fifo`'s own proof.

The one `unsafe` left in each harness mints the tokens. Two obligations hold for every harness,
and they are properties of the harness that Kani does not check:

- Every node outlives the object. A harness declares its nodes in one `let` before the object, and
  Rust drops locals in reverse order of declaration, so the object goes first. A node still on a
  queue when the harness returns was valid for its whole time there.
- Every node is on no queue when it is minted, and is minted once. A new node has a null link and
  a new object is empty. That is why the harnesses carry a separate `me`, and in one case a `me2`,
  rather than reusing a seeded node.

Kani would catch a dangling dereference that a proof reached, but nothing proves the obligations
are kept. The host tests keep the same two, with each node in a `Box` declared before its object.

### The rendezvous harnesses

`seed` makes the arbitrary state: at most one queue holding one waiter, a symbolic pending count,
and a symbolic interrupt binding. Every harness therefore covers a bound rendezvous, including one
bound after a sender queued.

- The send harness folds the interrupt refusal in, as the same decision's other branch. Removing
  the refusal from `send` turns it red at both of its `!bound` assertions. Its replayable record is
  a different defect, since the convention holds one patch per harness.
- The token harness checks both operations over every seeded state. The verdict that holds no
  token for the caller is exactly the one that grew the caller's queue. Every other verdict hands
  back a token naming the caller and leaves its queue as it was.
- The collected-sender harness is the rendezvous half of the one-shot Reply of §12 (Call/Reply
  IPC). A caller queues as a sender and blocks, and a receive that collects it pops it, so the
  rendezvous holds no name for it afterwards. The kernel-minted Reply capability is then the only
  name for the caller, and `crates/capability` proves that name is consumed on use. A blocked
  thread cannot run, so it cannot queue itself twice. The harness states this through emptiness,
  since a membership scan would hand the solver an unbounded loop.

The remove harness is the only bounded one. Its interesting case is the miss: the kernel asks
both removes on every victim, and a drain over a queue the victim is not on must rebuild that
queue exactly. It runs one remove per run, chosen symbolically, because the removes share no state
between calls. A run of both removes twice each did not finish in seventeen minutes, and
`unwind(3)` with one remove takes 0.2 seconds. The bound is sound: Kani checks the unwinding
assertion, so a loop that needed a fourth pass would fail the proof. `seed` queues at most one
waiter, so two passes are the most a drain can take. A queue of many waiters is argued here, as in
every other harness, not proved.

### The timer harnesses

They run over three timers. That is enough for the two orders a walk can get wrong: the first due
timer is not the only one, and a due timer sits between two that are not.
