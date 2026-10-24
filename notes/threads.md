# Threads, the context switch, and preemption

Milestone 6. The thing the whole project has been arguing about since before a line of it
existed.

## A thread is a stack plus a set of register values

From [registers.md](registers.md), milestone 1:

> That is not a metaphor. It is the complete and literal definition.

And here it is, written down:

```rust
pub struct Thread {
    pub id: ThreadId,
    pub state: State,
    pub context: *mut Context,     // <- 8 bytes. The ENTIRE saved CPU state.
    pub stack: Option<KernelStack>,
}
```

A suspended thread's whole CPU state is one stack pointer. Everything else: the registers,
the return address, the frame pointer: is sitting on the stack it points at, pushed there by
`switch_to`. Eight bytes.

## The context switch is fifteen instructions

```asm
switch_to:                      // x0 = &prev.context,  x1 = next.context
    sub  sp, sp, #96
    stp  x19, x20, [sp, #0]     // push OUR callee-saved registers
    ...
    stp  x29, x30, [sp, #80]

    mov  x2, sp
    str  x2, [x0]               // prev.context = sp    <- the whole of "saving a thread"

    mov  sp, x1                 // and now we are on somebody else's stack

    ldp  x19, x20, [sp, #0]     // pop THEIR callee-saved registers
    ...
    ldp  x29, x30, [sp, #80]
    add  sp, sp, #96

    ret                         // <- returns into a DIFFERENT THREAD
```

The last instruction is the trick. `ret` jumps to `x30`, and by that point `x30` has been
loaded from the *other thread's* stack. So it does not return to the caller. It resumes a
different thread, at the point where that thread last called `switch_to`, possibly seconds
ago.

A context switch is a function call that returns somewhere else.

### Why only twelve registers, when the trap frame saves thirty-three

`vectors.s` saves all 31 general registers, because an exception lands between two
arbitrary instructions and the interrupted code has no idea.

`switch_to` is an ordinary function call. AAPCS64 already says the caller must assume
`x0`–`x18` are destroyed by any call it makes: the compiler has already spilled anything it
cared about. So we save only what the convention promises a callee will preserve: `x19`–`x28`,
`x29` (frame pointer), `x30` (link register), and `sp`.

That isn't an optimization we invented. It falls out of the calling convention, and it is why
real kernels have both a trap frame *and* a much cheaper voluntary switch.

(And no floating-point registers, because the kernel is `softfloat`: a decision made at
milestone 1 for a completely unrelated reason. `d8`–`d15` simply do not appear.)

## Starting a thread that has never run

A new thread has no saved registers to restore. So we fake them: write a `Context` onto the
fresh stack with `x30` pointing at `thread_trampoline`.

Which means the very same `ret` that *resumes* an existing thread also *starts* a new one.
There is no separate first-run path. The trampoline just happens to be what `x30` points at.

The closure travels in `x19`: a callee-saved register, chosen precisely because `switch_to`
restores it on the way in.

> `Box<dyn FnOnce()>` is a fat pointer (data + vtable), two words, and we have one register.
> So box it twice: `Box<Box<dyn FnOnce()>>` is a *thin* pointer to a fat one, and fits.

### The trampoline must unmask interrupts, but AFTER `finish_switch`, not before

A brand-new thread must unmask interrupts by hand. We arrive from inside the timer IRQ handler (or a
`schedule()` that ran there), IRQs masked. Unlike a *resumed* thread there is no `SPSR_EL1` to
restore the interrupt state from: the thread was never interrupted, it has never run at all.
Without unmasking, the first thread you spawn runs with interrupts masked forever, never preempted:
a cooperative scheduler with extra steps.

The trap, and a real intermittent hang it caused (found by a watchdog dumping thread states, see
notes/deadlock.md): the trampoline first called `thread_entry`, whose first act is `finish_switch`,
and it unmasked interrupts before that call. `finish_switch` reaps the predecessor and completes
any wake it deferred, by reading this core's `switched_from`. If a timer IRQ lands between an early
unmask and `finish_switch`, it runs `schedule()`, which overwrites `switched_from` with the fresh
thread: the predecessor is stranded. Its `on_cpu` never clears, so every future wake for it is
deferred forever (`wake_pending` set, never completed). It blocks on some later IPC and never wakes.

So the ordering is load-bearing:

```asm
thread_trampoline:
    // NO early unmask here.
    bl   thread_entry          // finish_switch() runs FIRST, masked
```
```rust
extern "C" fn thread_entry(...) {
    finish_switch();                       // masked: nothing can overwrite switched_from
    crate::arch::interrupts::enable();      // NOW unmask; the closure is preemptible
    call(closure);
}
```

For a user thread the unmask is even freer: `finish_switch` runs masked, and the `eret` that
drops to EL0 restores an `SPSR` with IRQs on, so the EL0 thread is preemptible from its first
instruction with no explicit `enable`. Same rule either way: `finish_switch` runs masked. The
general statement is that the whole switch, from `schedule` masking IRQs to the successor's
`finish_switch` completing, is one atomic region; a fresh thread's entry is inside that region and
must not open it early.

## Preemption is four lines, because milestone 2 did the hard part

At the bottom of the IRQ handler:

```rust
gic::end_of_interrupt(intid);

if sched::take_need_resched() {
    sched::schedule();          // <- may not return here for a long time
}
```

We're still on the interrupted thread's kernel stack, with its full TrapFrame sitting below
us: `vectors.s` saved it before Rust ever ran.

`schedule()` may switch to another thread entirely. When it does, that call does not return
here. It returns *in some other thread*. We come back only when somebody schedules us again,
and then `exception_restore` pops the TrapFrame and `eret` resumes the instruction we
interrupted, **which never knew any of this happened.**

That is the whole of preemption. The register-saving machinery already existed, built at
milestone 2 for exception handling, with no thought of threads.

Note the EOI comes first. Switching away with the interrupt unacknowledged would leave the
GIC refusing to deliver anything of equal or lower priority to the thread we switch *to*.

## Three rules, and each is a bug if you get it wrong

1. Release the run-queue lock BEFORE switching. Switch away holding it and the lock is now
held by a thread that is not running. The next thread to want it spins forever, waiting for a
thread that can only be scheduled by taking the lock. A deadlock that would take a day to find.

2. Interrupts stay masked across the switch. Between "I decided to switch" and "I switched"
there must be no window for the timer to decide *again*.

And the saved interrupt state is a local, on this thread's stack, which is exactly what
makes it work. When somebody eventually switches back to us, `switch_to` returns into
`schedule()`, and that frame, with the right `was_enabled` in it, is still sitting where we left
it.

**3. A thread cannot free the stack it is standing on.** `exit()` marks itself `Finished` and
calls `schedule()`. The next thread reaps it, once we are safely off it. Every kernel has
something called a reaper, and this is why.

## A brand-new thread unmasks its own interrupts

A resumed thread gets its interrupt state back from the exception return, which restores
`SPSR_EL1` on aarch64. A thread that has never run has no saved state to restore. So
`thread_entry` unmasks interrupts itself, after `finish_switch` and never before it: a tick taken
before `finish_switch` would overwrite `switched_from` and strand the predecessor
(`arch/aarch64/context.s` has that hang). Without the unmask, the first thread spawned could never
be preempted, and the scheduler would be cooperative with extra steps.

`a_thread_that_never_yields_is_preempted_anyway` spawns a thread whose whole body is
`loop { count += 1 }`, with no yield, no syscall and no call, and it is preempted anyway.

## The address-space leak that hid behind a two-frame test failure

The reaper test failed with two frames leaked, not thirty-two. So the stacks *were* being
freed.

The two were page tables. The stack area is a fresh region of virtual address space, so the
first `map_page` there had to build an L2 and an L3. And `unmap_page` frees the leaf mapping but
leaves the intermediate tables standing (the TODO on `paging::unmap`).

Which exposed a real leak hiding behind it: stack virtual addresses were **bump-allocated and
never reused**. Threads come and go, but every 2 MiB of address space ever consumed keeps its
page tables forever.

The fix is a free list of dead threads' address ranges, so a new thread lands in page tables that
already exist. There is now a test asserting that a second batch of eight threads costs
exactly zero additional frames.

## The test

```rust
sched::spawn(|| {
    while !STOP.load(Relaxed) {
        SPINNING.fetch_add(1, Relaxed);
        // Deliberately nothing else. No yield. No syscall. Not even a function call.
    }
});
```

From [DECISIONS](../design/decisions/0005-preemptive-threads.md) §5, written before any of this existed:

> A userspace process is an arbitrary ELF binary. It has its own stack, **it never yields**, and
> it will loop forever because we will write a bug. Under cooperative scheduling, one bad user
> program hangs the machine permanently.

Under async/await, or Go before 1.14, or any cooperative runtime, that thread takes the CPU and
never gives it back. The **only** thing that can take it back is a timer interrupt landing
between two instructions of that loop and switching the stack out from under it.

At boot:

```
  half a second later, having spawned two threads that NEVER yield:

    thread 1 (hostile) :    3304328 iterations
    thread 2 (polite)  :    3133498 iterations
    preemptions        :         48

  neither asked to be interrupted. both were.
```

The argument was right, and the kernel can host untrusted code.


## Kernel thread stacks

Every kernel thread stack is six pages, 24 KiB, with an unmapped guard page beneath it. The guard
turns an overflow into an instant, precise fault rather than silent corruption, and threads are
where deep recursion actually happens. `kernel/src/thread.rs` builds both with `mmu::map_page` and
`mmu::unmap_page`, whose break-before-make and TLB flush the stack relies on.

Linux's arm64 stack is 16 KiB, sized for optimized code. This kernel's suite runs at debug codegen,
where frames are severalfold larger, and the measured worst case did not fit 16 KiB:

- the deepest standing path the suite reaches on a thread stack, about 11.7 KiB
  ([stack high-water](stack-high-water.md));
- the residue of blocking from that depth (`ipc_receive` 656 bytes, `IPC_TABLES.lock` 256,
  `schedule` 448, plus the switch), about 1.4 KiB, resident while the thread stays blocked;
- one preemption landing at the deepest point (a 272-byte trap frame, dispatch, the interrupt
  controller's claim, `canary::check`, `schedule` and a contended `IPC_TABLES.lock` spin), about
  2.3 KiB.

That is about 15.5 KiB. CI on loaded two-core hosts overflowed 16 KiB on both aarch64 and riscv64:
the guard page caught an exception-entry push mid-cascade, with the interrupted context spinning in
`IPC_TABLES.lock`. A loaded host multiplies timer preemptions per guest instruction, so one
eventually lands on the deepest frame of the deepest thread. Six pages leave about 8 KiB above the
measured worst case, at a cost of two more frames per live thread. The high-water gate in
`stack.rs` still alarms well before the guard.

Stacks live in their own virtual area, far above the direct map, so a stack address can never
collide with the virtual name of a physical page. The area is each architecture's
`arch::mmu::THREAD_STACK_AREA`, because on `x86_64` the kernel base already carries the bit a
portable computation would have set, and every stack would have landed on the kernel image.

A dead thread's address range is reused. `unmap_page` frees the leaf mapping but leaves the
intermediate tables standing ([teardown](teardown.md)), so bump-allocating addresses forever would
cost an L2 and an L3 table for every 2 MiB consumed. A reused range lands in tables that already
exist, and a test asserts that a second batch of threads costs zero additional frames. The free
list is sized from `sched::MAX_THREADS`, so raising the thread ceiling cannot overflow it.

The physical pages come from the kernel's own budget (`kmem`), not the frame allocator, so the
kernel's stack spending is bounded by its boot carve. Unmapping a dead stack must invalidate the
translation, because the address goes to a different thread's stack next, and a stale entry would
let that thread read and write the dead thread's saved registers.

## Building a thread in place

`Thread::spawn_into` moves the closure, at its concrete type, to the top of the new stack, above a
faked `switch_to` frame. A monomorphized `call_closure::<F>` is handed over beside it to call it, so
there is no heap allocation and no vtable, and the closure's memory is freed by being the stack.
The closure slot is aligned down to 16 so the switch frame keeps `sp` 16-aligned, and bytes above
the initial `sp` are never touched by the thread's own execution. A capture over 1 KiB, or aligned
past 16, is refused at build time. If a thread is torn down before it ever runs, its closure's
destructors do not run and its captures leak in place; that is fine for what kernel threads
capture (ids and statics).

A `Thread` is large, and a debug build copies rather than elides at every move. Returned by value,
it traveled through several frames, each a real copy through a stack temporary. `sched::spawn_on`'s
instantiations measured 3,888 to 4,592 bytes. That is over the 4,096-byte guard page, where a frame
can step past the guard in one move ([the stack](stack.md)). So every constructor
writes through a pointer the caller already has, the TCB page `Threads::insert_at` claimed. The
struct literal itself sits in `write_kernel_thread`, one non-generic, never-inlined frame, because an
unoptimized build materializes the literal as temporaries worth about two `Thread`s, and inside the
generic `spawn_into` every closure type paid that on top of its capture
(`script/stack-frame-check`). It stays a literal so the compiler checks it for completeness.

---

*Add to this file as new scheduling concepts come up.*
