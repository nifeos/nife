# Stack high-water: measuring kernel stack depth

Milestone 84. The FS-server stack bug (notes/nifefs.md, notes/fs-server.md) was a kernel stack
overflow found the expensive way, and until this instrument existed nothing measured depth on any
kernel stack: "the stacks are big enough" was an argument. This note records the instrument, the
inventory it covers, and the numbers it measured.

## What "high water" means, because it inverts twice

In plain words, `high_water` is the most bytes the stack ever used. Not its depth now, not the
space still free: the maximum it ever reached, at some instant nobody was watching.

That is the whole reason painting works. A stack rises and falls thousands of times a second and no
sampling scheme would catch the peak. But the deepest frame *destroys the paint* at its low point,
and that damage outlives the frame by the whole run, so one scan afterwards recovers a maximum
nobody had to observe.

```text
  top     (high address)   <- sp starts here
    |
    |   overwritten: the paint is gone
    |   <- high_water measures THIS span
    |
    p     <- deepest any frame ever reached
    |
    |   still painted: nothing ever came this far
    |   <- this is the headroom
    |
  bottom  (low address)
```

Two things flip in the reading, which is why this section exists:

- The painted bytes are the ones that were never used. Paint is evidence of *absence*.
  `high_water` counts the bytes where the paint is gone.
- Stacks grow down, and the number counts up. The deepest point is the *lowest* address, and
  `high_water` returns `top - p`, a magnitude, so bigger means deeper means closer to the guard
  page.

The shortcut worth remembering: paint left is room to spare. The boot stack's 53,808 of 65,504
means 53,808 bytes were used at the deepest moment and 11,696 bytes still hold
`0x5AFE_57AC_5AFE_57AC` and never saw a frame.

## The instrument

The classic watermark, because the classic one is right. Paint every kernel-owned stack with a
64-bit pattern (`0x5AFE_57AC_5AFE_57AC`) before anything uses it, and run the suite. Then scan each
stack upward from the bottom for the first word that is no longer the pattern. Bytes between there
and the top are the stack's high-water mark. The scan is an iterative loop with no locals of size,
so it needs no meaningful depth itself.

Test builds only, deliberately. Painting 24 KiB on every thread spawn would perturb the spawn
benchmark, and the report goes through the test output channel anyway. (The two primitives, `paint`
and `high_water`, are compiled under the `ipc_stack_depth` feature since 2026-09-19, for the
per-IPC section below and [stack-high-water/per-ipc-depth.md](stack-high-water/per-ipc-depth.md); the whole-suite report stays test-only.) The code is in
`kernel/src/stack.rs` (paint, scan, report), with call sites in `kernel_main` (boot stack),
`smp::bring_up_secondaries` (secondary stacks), and `thread::KernelStack` (thread stacks, painted
at allocation, scanned in `Drop`); `sched::scan_live_thread_stacks` covers the stacks nothing ever
reaps. All of it is portable code; the only arch-specific piece is `arch::current_sp()`, which
already existed for the canary.

## The inventory, from the linker scripts and boot code

| Stack | Where declared | Size | Guarded? | Painted |
|---|---|---|---|---|
| Boot stack (boot core) | `link-aarch64.ld` / `link-riscv64.ld`, `__stack_bottom`..`__stack_top` | 64 KiB | guard page below | at `stack::init` time, canary to a margin below live `sp` |
| Secondary stacks (per core) | `SECONDARY_STACKS` in `kernel/src/smp.rs`, `.secondary_stacks` | 64 KiB x MAX_CPUS | guard page below (milestone 90 (a guard page under the per-CPU secondary stacks)) | whole stack, before `CPU_ON` |
| Kernel thread stacks | `KernelStack` in `kernel/src/thread.rs` | 24 KiB (6 pages) | guard page below | whole stack, at allocation |
| Interrupt stacks (per core) | `kernel/src/interrupt_stack.rs`, `.interrupt_stacks` | 16 KiB x MAX_CPUS | guard page below | whole region, at `interrupt_stack::init` |

The trap frame is built on the interrupted stack on both ISAs. aarch64's `vectors.s` builds its
272-byte frame on `SP_EL1`, which is whatever kernel stack is live (the hardware banks `SP_EL0`
away, so a user program's `sp` never enters into it). RISC-V's `trap.s` stays on the interrupted
`sp` for an S-mode trap. It has to: a preempted thread's frame must survive until that thread runs
again. So the frame's depth is measured as part of whichever stack the trap interrupted, which is
how a preemption gets billed to the thread it preempted (notes/stack.md). The handler above the
frame, on a trap taken from kernel mode, runs on the interrupt stack in the last row. A trap from
user mode does not switch at all.

The boot core's slot in `SECONDARY_STACKS` exists and is never used (the boot core runs on the
linker-script stack); the report skips it. On RISC-V the boot hart is whichever one OpenSBI's
lottery picked, so "the boot core's slot" is not always slot 0, and the skip follows
`arch::boot_cpu_id()`.

## The guard page under each secondary stack (milestone 90)

The inventory found that the boot stack and every kernel thread stack had an unmapped page beneath
them, and the per-CPU secondary stacks did not. A secondary that ran deep overwrote `.bss` instead of
faulting. The stacks now live in their own page-aligned `(NOLOAD)` region, one 4 KiB guard and one
64 KiB stack per slot, and the mapper skips each guard. It cost 16 KiB of address space and no
physical frames. A page-table walk on both ISAs proves each guard does not translate, and
`mmu::verify` refuses a map that lost the holes. On the coarse boot map, before
`mmu::init_secondary`, the guard is still mapped. The layout, the proof and the re-measurement are
in [stack-high-water/secondary-guard-page.md](stack-high-water/secondary-guard-page.md).

## Honest limits

- A watermark sees only exercised paths. An unexercised deep path stays invisible, the same
  limit coverage has. The static complement (`-Zemit-stack-sizes` worst-case accounting) breaks on
  indirect calls and has not been built.
- The boot stack has a floor. It is painted from `kernel_main`, a few frames deep, up to a
  512-byte margin below the live `sp` (the margin keeps the paint loop's own callee frames, real
  calls in a debug build, out of the painted region). Depth used before that moment and never
  reached again is invisible, and no measured value can come out below the floor. The report prints
  the floor next to the number.
- A frame whose deepest word happens to equal the paint pattern reads one word shallow. A
  64-bit pattern makes this vanishingly unlikely.
- Live-stack scans race their owners. A secondary or a live thread may deepen its stack after
  the scan passes; the snapshot is a lower bound taken at end of suite. Reaped thread stacks are
  scanned in `Drop`, after the owner is provably off them, so those are exact.

## Why the numbers are host-load-immune

Depth is a property of the code and the suite: the same calls push the same frames whether the host
is idle or thrashing. The one timing-dependent contribution is where on a stack an interrupt's
frame lands, which varies with interrupt arrival, so the numbers jitter by roughly a trap frame
plus the handler path, not with runner load. The measured spread across runs is below, and the
assertion margin has to cover it.

## The numbers

Debug build (the test profile), QEMU `virt`, `-smp 4`, full suite (223 tests on aarch64).

| Stack | aarch64 run 1 | aarch64 run 2 | riscv64 run 1 | size |
|---|---|---|---|---|
| boot | 53808 (82%) | 53808 (82%) | 54216 (82%) | 65504 painted |
| core 1 | 8504 (12%) | 8504 (12%) | 8448 (12%) | 65536 |
| core 2 | 8504 (12%) | 8504 (12%) | 8448 (12%) | 65536 |
| core 3 | 8504 (12%) | 8504 (12%) | 8448 (12%) | 65536 |
| thread max | 11352 (69%, 420 stacks) | 11352 (69%, 420 stacks) | 11672 (71%, 415 stacks) | 16384 |

Boot paint floor: 640 bytes (aarch64), 1024 bytes (riscv64); every measured boot number is far above
its floor, so the floor is not what is being read.

A second riscv64 run (the gate run for the assertion below) reproduced its column exactly, from a
*different boot hart*. OpenSBI's lottery booted hart 3 rather than hart 0, and the report skipped
the boot hart's unused slot as designed. The three secondary numbers were 8448 again.

Three things the table says beyond the values. The numbers are exactly reproducible. The two
aarch64 runs agree byte for byte on every stack, including all three secondaries. They were taken
under host load averages of 33 and 9 (a concurrent cargo-mutants lane was saturating all eight cores
during the first). Depth really is a property of the code and the suite, not the runner; the
interrupt-timing jitter the design worried about does not reach the deepest byte on this suite.
The two ISAs agree to within about 400 bytes on every stack, which is what "same code, same
suite, different frame layouts" should produce. And the boot stack is at 82%, much closer to its
guard page than anything else in the kernel. If the suite's deepest test chain grows, the boot stack
is where the growth lands (see the gate below for what fails first).

The boot stack is the deep one, and the reason is structural: `test_main` runs on the boot
context, so the boot stack carries the deepest call chain of the entire suite, every test body
included. The secondary stacks carry only idle loops, trap frames, work stealing, and the SMP
probes. Thread stacks carry every spawned kernel thread and every process's kernel side.

The move to the secondary-stack region changed no number, byte for byte, on either ISA: depth is
decided by which calls run, and moving a stack's base changes no call. The table is in
[stack-high-water/secondary-guard-page.md](stack-high-water/secondary-guard-page.md).

The FS server's *user* stack has its own watermark already (`the_redoxfs_servers_stack_still_has_headroom`,

The FS server's *user* stack has its own watermark already (`the_redoxfs_servers_stack_still_has_headroom`,
in `system_tests/src/user/filesystem_tests.rs` and its RISC-V twin in `riscv_virtio_tests.rs`); this instrument is the
kernel-stack complement.

## The gate

The numbers were stable enough to gate on immediately (identical runs under a 3.5x load difference;
~400-byte cross-ISA spread). So the threshold assertion landed in the same milestone. It is checked in
`report_high_water` after the printing, so a trip always arrives with its numbers. One shared set of
limits on both ISAs, per the parity gate:

| Stack | limit | over observed max | what a trip means |
|---|---|---|---|
| boot | 61440 | +7224 (13%) | the suite's deepest chain grew ~7 KiB; one page left before the guard |
| secondary | 16384 | ~2x | something new is running deep on an idle-and-traps stack |
| thread | 18432 | +6760 over observed; ~+3 KiB over worst-case stacking | some kernel thread is 6 KiB from its (24 KiB stack's) guard |

The margins are deliberately margins over *observed* depth, not fractions of the stack: the
observed spread is a few hundred bytes, so a few thousand bytes of allowance absorbs toolchain
drift while still failing long before the guard page would. If a nightly bump trips one of these
with an honest, reviewed growth, raise the limit with the new measurement in hand; that is the
gate working, not failing.

The thread limit is the one that has to be sized against stacking, not against the observed
number, and the 2026-08-15 CI overflows are why (the full story is in notes/stack.md). The
observed high-water is what the suite's runs happened to catch. The honest worst case is the
deepest standing path (~11.7 KiB), plus a blocked thread's resident residue (`ipc_receive` +
`SCHED.lock` + `schedule` + the switch, ~1.4 KiB), plus one preemption landing at the deepest
instant (~2.3 KiB). That is about 15.5 KiB. It is why two CI runs overflowed a 16 KiB stack that a
green high-water report said was 71% used. A loaded host does not change any depth; it multiplies
timer interrupts per guest instruction until one lands on the worst-case alignment. The instrument
measures truly; it just only measures the alignments that occurred. This is the same lesson as the
reap-frame incident one section down, one level up: there a single frame outran the margin, here
the *sum of independent layers* did, and neither is visible in a single row of this table.

The secondary row's original entry read "an idle-and-traps stack that has no guard page", and
said in the same breath that this assertion was the only tripwire there. Milestone 90 made that
false. The honest restatement is that all three rows now do the same job. They are the alarm that
fires in the run that *drifts*, tens of kilobytes before the MMU would fire in the run that dies. That is worth having on top of a guard page, not instead of one, and it is the only one of
the two that a release build does not get.

## The other half of the instrument: what one frame costs, statically (2026-08-13)

A watermark says how deep the suite went, not which function is expensive. `-Z emit-stack-sizes`
on the test build says what each frame costs, with no emulator. It found
`sched::reap_region_objects` at 6816 bytes, 2104 more than the thread stacks' entire measured
headroom, which is what faulted the branch of milestone 108 (the drivers move onto frame capabilities). A `[T; MAX]` local sized to a table maximum
is a stack allocation wearing the clothes of a bound. The frame now carries no table-sized local, and
`script/stack-frame-check` fails the build on a frame over the 4,096-byte guard page. The method and
the account are in [stack-high-water/stack-frame-sizes.md](stack-high-water/stack-frame-sizes.md).

## Per-IPC depth: how much of its kernel stack one IPC touches (milestone 134, 2026-09-19)

The same paint and scan, re-armed around one IPC operation, measures how much of a thread's kernel
stack one IPC reaches. In the release kernel a round trip reaches about 600 bytes on riscv64 kernel
threads and about 1 KiB at EL0, half the 1 to 2 KiB that E1 of milestone 134 (the register of measures) estimated. The debug
kernel reaches 2 to 2.4 KiB. At 600 bytes a thread, cache capacity does not explain E1's knee at 8
threads. Page-aligned stacks aliasing in radon's 4-way L1 might; so might page-aligned TCBs, and a
cheap coloring experiment would tell them apart. On radon at one commit, kernel-thread CALL/REPLY
reproduces QEMU to the byte and every EL0 series reads 16 bytes deeper, because `single_hart` grows
`riscv_trap_body`'s frame. The instrument, the tables and the readings are in
[stack-high-water/per-ipc-depth.md](stack-high-water/per-ipc-depth.md).

## BUGS

Each entry is in full in [stack-high-water/limitations.md](stack-high-water/limitations.md).

- The per-IPC depth instrument copies two fixtures' role numbers rather than reading them from a
  crate.
- Per-IPC depth is not in a gate; the test asserts each line is a measurement, not its value.
- Depth reached before `paint_boot_stack` runs is invisible, bounded by the paint floor.
- Static frame sizes are per function. `script/stack-depth-check` composes them into a worst-case
  chain, which is still a lower bound: indirect calls and assembly are uncounted.
- `script/stack-frame-check` gates frame size at the 4096-byte guard page, since 2026-08-13.
- The thread-row threshold and `script/stack-depth-check`'s warning are the same number for
  different reasons; raise them together.
- A stack whose deepest word stored the paint value reads one word shallow.
- Live scans at end of suite are snapshots; reaped thread stacks are exact.
- The instrument is `cfg(test)` only. The guard pages are in every build.
- The guards are absent on the coarse boot map, and nothing checks them after boot except the
  suite.
- The boot core's slot in the secondary-stack region is mapped and never used.

