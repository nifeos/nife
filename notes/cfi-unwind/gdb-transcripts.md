# Before and after: the GDB transcripts

*Name: provisional.*

The evidence [`notes/cfi-unwind.md`](../cfi-unwind.md) summarizes. Captured on aarch64 (the only
architecture `cargo xtask gdb` currently drives), with GDB 17.2, against two builds of the identical
source tree: commit `ece6d72c`, immediately before this milestone, and this branch's tip.
`cargo xtask gdb` boots the kernel under QEMU with `-s -S` and prints the same instructions
`notes/scripts.md` already documents. These are a real `gdb <elf>` / `target remote :1234` session,
not a paraphrase.

## Case 1: a brand-new thread's first run (both builds agree, and should)

```
(gdb) break thread_entry
(gdb) continue
(gdb) bt
```

Before:
```
Thread 3 hit Breakpoint 1, kernel::thread::thread_entry (closure=..., call=...) at kernel/src/thread.rs:913
#0  kernel::thread::thread_entry (closure=..., call=...) at kernel/src/thread.rs:913
#1  0xffff0000400d94a4 in thread_trampoline ()
Backtrace stopped: previous frame identical to this frame (corrupt stack?)
```

After:
```
Thread 3 hit Breakpoint 1, kernel::thread::thread_entry (closure=..., call=...) at kernel/src/thread.rs:913
#0  kernel::thread::thread_entry (closure=..., call=...) at kernel/src/thread.rs:913
#1  0xffff0000400d94a4 in thread_trampoline ()
```

Both stop at `thread_trampoline`, and that is correct in both cases: `Context::for_kernel_thread`
fakes this thread's very first frame with no real caller (its own comment: "no caller: the backtrace
ends here"). The before transcript stops there by accident, since no CFI existed to say anything. The
after transcript stops there on purpose (`.cfi_undefined lr`). The difference shows up as GDB no
longer printing "corrupt stack?" about a frame that was never corrupt, only undescribed.

This case was chosen first and reported honestly even though it does not show an improvement in the
*reach* of the backtrace. A fabricated frame that has no caller is not supposed to unwind further.
Claiming otherwise here would be exactly the invented-CFI failure this milestone was warned against.

## Case 2: resuming an already-running thread (the case that matters)

Breaking on `switch_to` itself and repeating `continue`/`finish`/`bt` across fourteen hits, once the
boot tour's initial thread-spawning settles down. The compiler's own CFI for ordinary Rust functions
was also being discarded before this milestone: the same link-script line discarded everything named
`.eh_frame*`. So GDB's frames 0-3 below come from its frame-pointer fallback heuristic, not from any
FDE; there was none. That heuristic is what makes the before transcript dramatic rather than merely
incomplete.

Before (second hit; this is not abbreviated for effect, the real output kept going until it was
killed at frame #9976, three minutes and forty-five seconds after the `bt`):
```
Thread 3 hit Breakpoint 1, 0xffff0000400d9450 in switch_to ()
#0  0xffff0000400d9450 in switch_to ()
#1  0xffff000040090970 in kernel::sched::schedule () at kernel/src/sched.rs:2145
#2  0xffff00004008c8e8 in kernel::sched::preempt_if_needed () at kernel/src/sched.rs:4639
#3  0xffff0000400b1718 in kernel::arch::aarch64::exceptions::exception_dispatch (frame=..., index=5) at kernel/src/arch/aarch64/exceptions.rs:351
#4  0xffff0000400d9ae0 in exception_vectors ()
#5  0xffff0000400d9ae0 in exception_vectors ()
#6  0xffff0000400d9ae0 in exception_vectors ()
[... repeats, identically, past #9976, never terminating on its own ...]
```

Frames 0-3 recover correctly because ordinary Rust functions still set up an AAPCS64 frame-pointer
chain (`x29`) regardless of CFI, and GDB's fallback walks that chain when it finds no FDE. Frame 4
is where the chain walks into `exception_vectors`, which builds a raw trap frame rather than a
conventional `x29` link. The heuristic, with nothing to tell it otherwise, reads some stale or
misinterpreted value as "the next frame pointer," gets the same address back, and loops forever.
This is the sharpest illustration in this whole milestone of why "no CFI" is not merely "less
information": GDB's own fallback is willing to trust a frame-pointer-shaped hand-written assembly
function that isn't one, and it does not know when to stop.

After (a representative hit, of a kernel thread parked in `sched::ipc_receive` waiting on an IRQ
endpoint, resumed and then re-scheduled by a timer preemption):
```
Thread 4 hit Breakpoint 1, 0xffff0000400d9450 in switch_to ()
#0  0xffff0000400d9450 in switch_to ()
#1  0xffff000040090970 in kernel::sched::schedule () at kernel/src/sched.rs:2145
#2  0xffff00004008fd70 in kernel::sched::ipc_receive (ep=2) at kernel/src/sched.rs:2704
#3  0xffff0000400bfdac in kernel::syscall::irq_wait (intid=79) at kernel/src/syscall.rs:1103
#4  0xffff0000400bf7fc in kernel::syscall::invoke (frame=..., slot=1, method=0, ...) at kernel/src/syscall.rs:452
#5  0xffff0000400bfcf0 in kernel::syscall::dispatch (frame=...) at kernel/src/syscall.rs:91
#6  0xffff0000400b15bc in kernel::arch::aarch64::exceptions::exception_body (frame=..., index=8) at kernel/src/arch/aarch64/exceptions.rs:432
#7  0xffff0000400b16f0 in kernel::arch::aarch64::exceptions::exception_dispatch (frame=..., index=8) at kernel/src/arch/aarch64/exceptions.rs:341
#8  <signal handler called>
Backtrace stopped: frame did not save the PC
```

This is the transcript that "proves it bought something." Frame 0 through 7 is new: seven real Rust
frames a debugger could not previously see past `switch_to` at all. `<signal handler called>` is
GDB recognizing `.cfi_signal_frame` on the vector-entry FDE and labeling it, rather than silently
misreading it as an ordinary call.

And the stop after it, "frame did not save the PC", is the honest limit the parent's hard case
describes. It is `.cfi_undefined x30` at the trap boundary, because GDB does not honor the
spec-correct `ELR_mode` column that would let it go one frame further, into whatever kernel code took
the IRQ.

Other hits during the same run showed the same shape resuming through `sched::preempt_if_needed`,
`sched::yield_now`/`run_idle`, and `sched::depart`/`exit`. Each unwound cleanly to its own real
caller and stopped at the same kind of boundary: a trap frame, or `kernel_main`'s own `-> !` entry,
correctly reported as the bottom.
