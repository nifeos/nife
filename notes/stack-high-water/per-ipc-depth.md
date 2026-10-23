# Per-IPC depth: how much of its kernel stack one IPC touches (milestone 134, 2026-09-19)

An appendix to [notes/stack-high-water.md](../stack-high-water.md), which summarizes it. It was
measured for milestone 134 (the register of measures).

Everything in the parent measures the deepest a stack ever went over a whole suite, which is spawn
and teardown. Milestone 134's E1 needed a different number: how many bytes of a thread's kernel
stack one IPC reaches. Its prediction of where IPC latency bends against thread count was arithmetic
on "roughly 1 to 2 KiB" per IPC, and that figure was an estimate.

## The instrument

`kernel/src/ipc_stack_depth.rs` (module, feature and line prefix all provisional). It is the same
paint and scan as the parent's, re-armed per operation. Just before one SEND, RECEIVE, CALL,
RECEIVE_CAP or REPLY, the thread paints its own kernel stack up to a margin below its live `sp`. Just
after, it scans. 256 samples per operation, reported as median, min and max, each as a distance from
the stack's top.

- Kernel threads, E1's own shape (`bench.rs`'s `ipc_rtt`, `call_reply` and
  `ipc_thread_scaling` are all kernel-thread pairs): the thread wraps each call.
- EL0 threads, the shape every service runs (`os_primitives_benchmarker`'s SEND/RECEIVE pair and
  `soaker`'s CALL/REPLY pair). A user thread cannot paint. So `syscall::dispatch` ends with a call
  that, for a registered thread only, scans what the finishing syscall reached and paints for the
  next. An EL0 thread's trap frame sits at the very top of its kernel stack, so the distance from the
  top is the whole of what one syscall touches.

### It measures its own reach first

A `null` series wraps an operation that does nothing. Its median must not read deeper than its floor
(the ceiling of the paint), or the margin is too small for the build and every other line is
contaminated. It is not "must equal", because equality holds under TCG for a reason unrelated to
the margin. The words between the instrument's own reach and the ceiling are never repainted. So a
single interrupt trap frame that lands on the ceiling word early in the run sets every later sample
to exactly the floor.

On the physical core (the HVF leg) the 256 samples usually finish before that happens. There the
median reads the true reach, 120 bytes shallower than the floor in the debug build, so an equality
check fails on every HVF run. Measured on 2026-09-24 (UTC): median 856 against a floor of 976. The
instrument's own callees stop 392 bytes into the 512-byte margin, and the words above them keep the
paint `KernelStack::new` laid down. Over three TCG runs the first 29 to 76 samples read the true
reach (920 against a floor of 1040 on that build), and every sample after one trap frame read the
floor. Whether that moment comes before sample 128 is timing, so equality was a timing assertion.

That check found both of this lane's own mistakes. A single 512-byte margin put every release series
exactly on its floor: the release IPC path is shallower than 512 bytes below the measuring frame.
And 256 bytes was too small for the debug build, whose paint loop keeps real calls. The margin is now
512 in debug and 64 in release, and both are checked on every run rather than trusted.

### What it costs

Nothing in any default build. Every piece is `any(test, feature = "ipc_stack_depth")`, including the
call in `dispatch`. So `script/fastpath-footprint`, `script/bench` and the icount tripwire build none
of it. In the test build an unregistered syscall pays one relaxed load. A measured operation pays a
paint of up to 24 KiB before it. That is why this is never on in a build that times anything, and
why a series stops painting once it is full. The frames the instrument itself occupies sit above the
paint and are never counted; the null line is the proof.

### What it cannot see

- Interrupt nesting. A tick that lands mid-operation puts a trap frame on the thread's stack, so
  every series has a tail. That tail is the `max` column; the per-IPC figure is the median. The null
  series shows the tail's size directly. In the debug suite (four cores, a busy machine) its median
  sits at the floor and its max 800 to 1,000 bytes below it, with nothing running but the
  instrument. In the single-hart release boots no null sample showed a tail at all.
- Anything shallower than the instrument. An operation that never goes below the measuring frame's
  own margin reads `AT FLOOR: at most N`, an upper bound rather than a depth. In release, the
  non-blocking EL0 operations do this (the SEND or REPLY that finds its partner waiting). What is
  true of them is that they reach no deeper than about 420 to 700 bytes, trap frame included.
- Real hardware. Depth is a property of the code, not the clock, so QEMU is a valid place to measure
  it, and TCG is fine: nothing here is a time. The build is what matters, which is why both are
  measured. One is `script/test`'s debug kernel. The other is the release kernel with `bench` that
  radon boots, built exactly as `script/board-image` builds it minus `board` and `single_hart`,
  which the board needs and QEMU cannot boot. One of them does move the trap path, by 16 bytes
  of frame; "radon against QEMU at one commit" below has it.
- Which bytes are hot. A depth is how far the path *can* reach on this stack; it is not a count of
  lines touched, and it is not a miss count. That is milestone 134's M7.

## The numbers

The tables below are from 2026-09-19, at the commit that added the instrument. The riscv64 path has
changed since, so they no longer reproduce to the byte: "radon against QEMU at one commit" has the
2026-10-20 figures.

Release kernel, `bench,ipc_stack_depth`, single hart (the bench runner's `NIFE_SMP=1`), QEMU TCG,
two boots per architecture, 2026-09-19. Bytes from the stack's top. Each role's figure is the median
of its deeper operation, because in a strict ping-pong every thread blocks once per round trip and
the blocking path is the deep one. A range is the two boots disagreeing (below).

| shape | role | aarch64 | riscv64 | x86_64 | of which below the call site |
|---|---|---|---|---|---|
| kernel threads, SEND/RECEIVE (E1's shape) | client | 560 | 608 | 440 to 504 | 320 / 352 / 168 to 248 |
| | server | 544 | 576 | 424 to 472 | 320 / 352 / 168 to 248 |
| kernel threads, CALL/REPLY | client | 640 | 672 | 584 | 416 / 448 / 344 |
| | server | 576 | 592 | 488 | 400 / 400 / 296 |
| EL0, SEND/RECEIVE | each end | 736 to 832 | 832 to 928 | 472 | (all of it) |
| EL0, CALL/REPLY | client | 928 | 1,024 | 648 | (all of it) |
| | server | 912 | 976 | 600 | (all of it) |

Debug kernel (`script/test`, `-smp 4`), same method, same day, one or two runs per architecture:

| shape | role | aarch64 | riscv64 | x86_64 |
|---|---|---|---|---|
| kernel threads, SEND/RECEIVE | client / server, deeper op | 2,088 to 2,408 / 2,216 | 2,056 / 1,880 | 2,216 / 2,024 |
| kernel threads, CALL/REPLY | client / server | 2,232 / 2,248 | 2,184 / 2,232 | 2,040 / 2,104 |
| EL0, SEND/RECEIVE | deeper op, either end | 3,272 to 3,608 | 3,480 | 2,712 to 3,048 |
| EL0, CALL/REPLY | client / server | 3,496 / 3,688 | 3,704 / 3,896 | 2,936 / 3,144 |

The ranges are the SEND/RECEIVE shape only, and they are about which operation blocks rather than
how deep any one path is. In a SEND/RECEIVE ping-pong, either the SEND or the following RECEIVE of a
thread finds its partner not yet waiting and takes the blocking path. Which one does is settled by
where each thread happens to be when the loop starts. The two paths differ by a few hundred bytes,
so the "deeper op" moves between them from boot to boot, even on one hart. The same depth values
recur: the min and max columns of one run are the medians of the other. What does not recur is the
assignment. CALL/REPLY has no such freedom, since CALL always blocks, and every CALL/REPLY row
repeated to the byte on every run.

### Three readings

1. The estimate was high by about 2x for the build that matters. E1 ran the release kernel on
   radon. There, one round trip reaches about 600 bytes of each thread's kernel stack (riscv64
   kernel threads, E1's own shape), roughly ten 64-byte lines, not 1 to 2 KiB. The debug build
   reaches 2 to 2.4 KiB, so an estimate calibrated against debug-build frames would have been
   right for the wrong kernel.
2. At 600 bytes a thread, capacity does not explain E1's knee. Stacks alone would fill a 32 KB
   L1d at about 32,768 / 600, roughly 54 threads. radon's knee is between 8 and 16 threads
   (notes/footprint-perturbation.md), where the stacks total 5 to 10 KB.
3. Page alignment might. Every thread's stack top is a page boundary (`KernelStack`, slots
   `STACK_SLOT_SPAN` apart), so every thread's ~ten hot lines sit at the same ten page offsets.
   radon's U74 L1 D-cache is 32 KiB, 4-way, virtually indexed, 64-byte lines (SiFive U74-MC Core
   Complex Manual 21G3.02.00, "L1 Data Cache", read 2026-09-19). So a way is 8 KiB, and the set
   index is VA bits 6 to 12. Bit 12 of a stack's top page alternates with the slot index (the stride
   is seven pages, an odd number). So there are two colors of four ways: at most 8 threads' hot
   stack lines can be resident at once, whatever the total footprint. The general form holds for any
   cache of at most 8 ways and 64-byte lines: a 32 KiB cache holds at most 32 KiB / 4 KiB = 8 lines
   that share a page offset. That puts a knee at 8 threads, which is where radon's curve starts
   bending (1.20x at 8, 1.68x at 16, then flat).

Reading 3 is a hypothesis, not a finding, and it has a competitor this note cannot rule out. Every
thread's TCB is also on its own page (`sched::spawn_on`'s "own TCB page"), so hot TCB fields alias in
exactly the same way. The two predict the same knee. What separates them is cheap: offset each
thread's initial stack pointer by a per-slot color (slot index times about 640 bytes, modulo a page)
and re-run E1. If the knee moves right, the stacks were the cause, and a process kernel can buy it
back with coloring without becoming an event kernel. If it stays at 8, the TCBs (or something else
page-aligned) are, and an event kernel's shared stack would not remove it either. Recorded as a
proposed milestone in design/roadmap/0134-the-measurements-that-decide.md's Follow-on.

## Radon against QEMU at one commit (riscv64, 2026-10-20)

The question: do `board` and `single_hart`, the two features radon's card adds, move the IPC path's
codegen? One board boot answers it if a QEMU boot of the same commit is beside it. radon booted
`script/board-image --bench --tftp --extra-features ipc_stack_depth` at `849313eea`, SEALED,
features `board,bench,single_hart,ipc_stack_depth`, and printed `bench: cntfrq 4000000` and
`bench: done` (`bench/radon-2026-10-20/bench-ipc-stack-depth.log`). QEMU booted the release kernel
built with `bench,ipc_stack_depth` at `8b4431fdd`, whose kernel sources are identical to
`849313eea`'s. It booted twice, on one hart, under `helpers/qemu-bounded.sh`
(`bench/radon-2026-10-20/qemu-ipc-stack-depth-1.log` and
`bench/radon-2026-10-20/qemu-ipc-stack-depth-2.log`). Medians in bytes from the stack's
top; "floor" is where the paint stopped.

| series | QEMU (2 boots) | radon | reading |
|---|---|---|---|
| kernel null, the instrument alone | floor 240, median 240 | floor 240, median 240 | match |
| kernel CALL/REPLY, client CALL | 624, below caller 384 | 624, below caller 384 | match to the byte |
| kernel CALL/REPLY, server RECEIVE_CAP | 576, below caller 368 | 576, below caller 368 | match to the byte |
| kernel CALL/REPLY, server REPLY | at floor, at most 304 | at floor, at most 304 | match |
| kernel SEND/RECEIVE, client | RECEIVE blocks, 576; SEND at floor 336 | SEND blocks, 496; RECEIVE 352 | floors match; a different operation blocked |
| kernel SEND/RECEIVE, server | RECEIVE blocks, 544; SEND at floor 320 | SEND blocks, 480; RECEIVE 320 | floors match; a different operation blocked |
| EL0, floor of every series | 752 | 768 | 16 bytes deeper on radon |
| EL0 SEND/RECEIVE, deeper op | RECEIVE 944, or SEND 848 | RECEIVE 960 | 16 deeper (boot 1's assignment) |
| EL0 CALL/REPLY, client CALL | 1,008 | 1,024 | 16 deeper |
| EL0 CALL/REPLY, server RECEIVE_CAP | 992 | 1,008 | 16 deeper |

Two readings.

1. Kernel-thread series agree. CALL/REPLY, which has no freedom over which operation blocks,
   reproduced to the byte. SEND/RECEIVE did what the section above says it does: radon's boot
   took the blocking path on SEND, which neither QEMU boot did, so its medians are a different
   path rather than a different depth. The values recur across the two: radon's RECEIVE medians,
   352 and 320, are QEMU boot 2's minimums, QEMU's server RECEIVE median of 544 is radon's max,
   and QEMU boot 2's server SEND max is radon's median of 480.
2. Every EL0 series is 16 bytes deeper on radon, floor included, so the difference sits above
   `syscall::dispatch`. It is `riscv_trap_body`: its frame is 176 bytes in the board build and 160
   in QEMU's, with `s5` to `s7` saved as well. Building each feature alone settles which one:
   `bench,single_hart,ipc_stack_depth` gives the 176-byte frame, `bench,board,ipc_stack_depth`
   the 160-byte one. `single_hart` adds no code to the trap path; its one `cfg` returns early from
   `smp::bring_up_secondaries`. So the likeliest cause is the compiler inlining and allocating
   registers differently once the secondaries' code is gone. That is not checked. Either way it is
   a codegen change on the hot trap path that nothing expected. The QEMU bench runner and a radon card do not run the same trap prologue: the board
   does three more stores and three more loads per trap.

The timing rows of this boot are not results, since the instrument paints on every sample. Its
`cycles_per_tick` line read `250.00` (25,000,808 cycles over 100,001 ticks).
