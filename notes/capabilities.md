# Capabilities, and why the kernel has no `open()`

The milestone 7 decision. [DECISIONS §10](../design/decisions/0010-capability-microkernel.md) is the record of *what* we chose
and what we rejected. This is the note on *what the words mean*.

## The one sentence

> A capability is a file descriptor that can point at anything, not files.

That is not an analogy that mostly works. It is the mechanism, generalized.

## Which means Unix already has capabilities

Look at what an `fd` is:

- Unforgeable. You cannot invent `fd 7` and get a file. There is nothing to guess.
- Per-process. Your `fd 3` and my `fd 3` are different files.
- A token you hold, not a name you can say.
- Delegable. You can pass one to another process over a Unix socket.

That is a capability. Every property. Unix had them all along.

The difference is that Unix has a back door.

```c
int fd = open("/etc/passwd", O_RDONLY);
//            ^^^^^^^^^^^^^
//            A NAME. In a namespace every process can see.
```

You did not *hold* anything. You said a name, and the kernel checked whether *you* (your
uid) were allowed. Your authority came from who you are, not from something you were
handed. That is ambient authority: authority that surrounds you, that you carry
everywhere, that you never had to be given.

And it lets a process mint a capability out of thin air. Say the name, get the fd.

We are not building the back door. That is the whole decision.

## What it is in memory, because there is no magic here

No cryptography. No unguessable number. The unforgeability is boring, and that is the point.

Each process has a capability table (a `CapabilityTable`): a table of slots living in kernel
memory. Userspace never sees a single byte of it. Userspace sees an integer.

```
  What the process says            What the kernel has
  ---------------------            -----------------------------------------------
      cap 0            ────────►    { Rendezvous,         obj: 0x4012_3000,  rights: SEND       }
      cap 1            ────────►    { PageFrame,          obj: 0x4008_1000,  rights: READ|WRITE }
      cap 2            ────────►    { ThreadControlBlock, obj: 0x4009_2000,  rights: ALL        }
      cap 3            ────────►    { empty }
```

You call `send(3, msg)`. The kernel looks in your CapabilityTable at slot 3, finds it empty, and
returns an error.

You cannot forge a capability for exactly the same reason you cannot forge a file
descriptor: the table is not yours to write. That is it. That is the entire security
mechanism, and it is one array bounds-check away from what we already know how to do.

> "But could I not write to my own CapabilityTable?"
>
> Only if you hold a capability to your CNode. Which someone would have had to hand you.

The recursion bottoms out at boot, where the kernel hands the first process a capability
to everything, and that process gives away slices and never gets them back.

## The argument, in one story: the confused deputy

This is why anyone cares. It is a 1988 paper by Norm Hardy and it has never stopped being
relevant.

A compiler service runs on a shared machine. It has permission to write its own billing log
at `/var/log/billing`, because it bills you for compiles. You invoke it:

```
compile foo.c -o /var/log/billing
```

And it does it. It destroys the billing log.

Not because of a bug. Because it *had* permission and it acted on *your* request using its
own authority. You deleted a file you have no right to touch, and every line of code
involved behaved exactly as designed.

The compiler is the deputy, and it was confused about whose authority it was acting
under. Ambient authority is what made the confusion possible: the compiler's right to write
that file came from *who it was*, and nothing in `-o /var/log/billing` told it that this
particular write was on *your* behalf.

**With capabilities the story cannot be told.** To have the compiler write your output file,
you must hand it a capability to that file. And you cannot hand it one you do not have.

The bug is not defended against. It is not constructible.

That is the same move as [`TlbFlush`'s `Drop`](page-tables.md) and the [lock
ranking](deadlock.md): make the bad state *unrepresentable* rather than checking for it at
runtime and hoping.

## Why we cannot just add this to Unix later

We can. It has been done. It does not work, and the reason is the whole asymmetry.

FreeBSD's Capsicum (2010) added `cap_enter()`: a process calls it and drops into capability
mode with no ambient authority. No `open()` by path. Only what you already hold.

It works. It is in the base system. It has been there for fifteen years.

Almost nothing uses it.

Because every program on the machine assumes it may call `open("/etc/resolv.conf")`. Once that
assumption is baked into a million lines of userspace, you cannot take it back without
rewriting all of it. OpenBSD's `pledge`/`unveil`, Linux's `seccomp` and Landlock: same story,
all revoke-after-the-fact, all partial, none achieving no-ambient-authority, because by the
time they arrived the world was built on it.

> **Ambient authority, once granted, cannot be withdrawn.**

And in the other direction it is easy. Fuchsia's `fdio` is a userspace library that gives you
`open`, `read`, and `write` on top of capability handles. The convenience comes back whenever we
want it. It is not the primitive.

| Direction | Cost |
|---|---|
| capabilities to a Unix-shaped API | **additive.** A library. Nothing is thrown away. |
| Unix to capabilities | **a rewrite**, and it has never once succeeded. |

Which is the same table as [DECISIONS §5](../design/decisions/0005-preemptive-threads.md)'s threads-versus-async, and it decides
this the same way. When one direction is cheap and the other is a rewrite, take the one that
keeps the option open.

## The kernel has almost no syscalls

```
Send   Recv   Call   Reply   NBSend   NBRecv   Signal   Wait   Yield
```

Roughly that. There is no `open`. No `read`. No `mmap`. No `fork`.

Everything else is a message to a userspace process. To open a file you send a message to a
filesystem server, and it sends you back a capability. To map memory you invoke a capability on a
page table. To start a thread you invoke a capability on a TCB.

> **The kernel is not a service provider. It is a message router.**

## An interrupt is a message

Where [milestone 2's exception model](exceptions.md) meets this one, and it is the prettiest
consequence.

A driver holds an interrupt capability, binds it to a notification, and blocks. The kernel's handler
does exactly one thing: signal the notification. The driver thread wakes up.

**The driver has no interrupt handler.** It has a loop:

```rust
loop {
    wait(interrupt_notification);          // sleeps until the device interrupts
    let packet = read_device_fifo();
    send(net_stack_endpoint, packet);
    ack(interrupt_cap);
}
```

Compare that to `handle_interrupt` in `arch/aarch64/exceptions.rs`, which runs in exception context
with
interrupts masked and has to be extremely careful about what it touches. This is ordinary code,
in a process, at EL0. If it deadlocks, it deadlocks by itself, and we restart it.

## Is it slow?

One IPC is not slow. seL4's fastpath is a few hundred cycles: comparable to a Linux syscall,
and *better* than one now that Spectre mitigations made Linux's syscall boundary expensive. Mach
was slow in the 90s and it poisoned the word "microkernel" for twenty years, but Liedtke showed in
1995 that this was an implementation failure, not a law, and it has stayed fixed.

The cost is that you need more crossings. A `read()` that was one syscall becomes six. And the
real bite is not the cycles, it is the cache and TLB pollution of landing in a different working
set. You can make the switch cost 200 cycles. You cannot make the other process's data be in L1.

The discipline that recovers most of it, which every serious microkernel arrives at independently:

> **IPC carries control. Shared memory carries data.**

Put the bytes *in* the message and you copy twice, and you are Mach, and you are slow. Put a frame
capability in the message and the receiver maps it. Zero copies.

And the honest 2026 note: `io_uring` exists because Linux's syscall boundary got expensive, and its
answer (a shared-memory ring, batched, stop crossing per operation) is this discipline under
another name. DPDK and SPDK moved the network and storage drivers into userspace for the same
reason. Those are microkernels. They had to bolt the isolation on afterwards with an IOMMU,
instead of getting it free from an address space they were going to have anyway.

**For us it is zero, and we should never let it argue either way.** We run on QEMU with no workload.
We will never measure it.

## What this is not: differentiation

Floated, and wrong, and written down here so it does not come back.

aarch64 is not virgin ground for capability microkernels. It is their home turf. seL4 is
primarily an ARM story. L4 runs on every Qualcomm baseband. An L4 derivative runs the Secure Enclave
in the phone in your pocket. QNX runs most cars. Trusty is on every Android phone. And
in the hobby-Rust space, Redox is already a Rust microkernel that runs on aarch64.

A capability microkernel on ARM is the single most ARM-shaped thing one could build.

The real reason is smaller and better: on the Unix path you transcribe, and on this path you
derive. xv6 has a canonical answer to every question the Unix path raises. There is no xv6 here.
Every design question is ours, and for a project whose purpose is understanding, that is not a cost.
It is the product.

---

*Add to this file as capability concepts come up: rights, delegation, revocation, the derivation
tree, untyped memory.*

---

# What was built: milestones 7d and 7e

The decision, §10 (process model: capability-based, microkernel), became code in two steps. Each has its own record in
[`capabilities/`](capabilities/README.md), and what each found is below.

## Milestone 7d: the syscall surface, and refusing to be a deputy

Milestone 7d (user mode: EL0, capabilities, the ELF loader, and IPC) shipped three calls: `exit`, `yield` and `invoke(cap, method, a0, a1, a2)`, which is everything
else. There is no `open`, `read`, `write` or `fork`. One call has been added since:
`SYS_CAP_DELETE` (2026-07-24, milestone 19d (run a real workload)), because forgetting something in your own table needs
no capability. The surface is five calls today <!--count:syscalls-->, and
[`notes/abi.md`](abi.md) is the current contract.

- The ABI is a crate, `crates/abi`, that the kernel and every user program depend on. If it
  changes, both sides fail to compile.
- The capability table, `crates/capability`, is pure logic and host-tested. Its one operation with
  teeth is `derive`, and rights only ever narrow. `GRANT`, the right to pass a capability on, is one
  Unix cannot express. A process now delegates over an endpoint ([delegation.md](delegation.md)).
- A new process holds nothing. `CapabilityTable::empty()` is the decision, and the demo shows the
  same binary printing with a console capability and unable to print one byte without it.
- The console `write` takes a user pointer, so the kernel could be a confused deputy and print its
  own memory. `syscall::user_slice` refuses by asking the hardware (`AT S1E0R`) whether EL0 could
  have read each page itself. A TOCTOU window is recorded in that function for when demand paging
  or shared address spaces arrive.

The whole record, with the demo output and why the console was still kernel-served at 7d, is in
[`capabilities/syscall-surface.md`](capabilities/syscall-surface.md).

## Milestone 7e: IPC, and the scheduler learning to wait

7e added the endpoint, a synchronous rendezvous: `SEND` blocks until a receiver takes three words
in registers, and `RECEIVE` blocks until one arrives. There is no buffer. `SEND` needs `WRITE` and
`RECEIVE` needs `READ`, so which end a process holds is a matter of rights. The scheduler gained a
`Blocked` state, and one equality check in `schedule()` stops the timer requeueing a thread halfway
through blocking itself. The message lives in a `Thread` field, because the two halves of a
rendezvous run on different stacks. The server on the other end was a kernel thread until
milestone 8 (the console driver leaves the kernel) moved it to userspace.

The whole record, including what single core made easy and where the SMP cost is now written down,
is in [`capabilities/ipc-endpoints.md`](capabilities/ipc-endpoints.md).
