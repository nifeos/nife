# The overflow of 2026-08-15, which was a different shape

*An appendix to [`notes/stack.md`](../stack.md), which explains the stack and is the page to read.
This file holds the account of the 2026-08-15 overflow and why growing the stack fixed that shape.
It moved here on 2026-09-25 (UTC) under §212 (a prose budget). The move then edited it only to meet
§213 (writing standards), splitting long sentences and dropping bold, with the meaning unchanged.
The five appendices were one file until then, in the order the main page's table lists them, so
"above", "below" and "this file" in them mean that whole sequence. The directory `notes/stack/` and
this file's stem are provisional names; naming is calef's.*


One day after the section above was written, CI overflowed a thread stack again, on both ISAs, in
the same test neighborhood. The runs were aarch64 run 31907966383 attempt 1 (slot 87, `ESR`
`0x96000047`, `ELR` `0xffff000040130214`) and riscv64 `thead-c906` run 31910308865 attempt 1 (slot
102, `scause` `0xf`, `sepc` `0xffffffc080257aec`). Both were during
`supervision_tests::a_faulting_child_reports_to_its_supervisor_and_is_reaped_then_respawned`, both
immediately after the user-fault kill report, both on loaded 2-core runners. Both had `sp` exactly
4096 bytes past the bottom of the 16 KiB stack. Later attempts of the same runs passed.
`reap_region_objects` was already fixed and `script/stack-frame-check` was already gating at the
guard page, so this was not a recurrence of the 2026-08-14 mechanism.

## Symbolizing it, and what the addresses said

The CI binaries were debug builds at known SHAs, so the honest move was to rebuild them exactly. A
local rebuild at the same SHA did NOT reproduce CI's text layout (two tries, two different layouts:
incremental compilation and something host-specific both move functions). What worked was an
`ubuntu:24.04` arm64 container with the repo mounted at CI's own path (`/home/runner/work/nife/
nife`), `CARGO_INCREMENTAL=0`, and the pinned toolchain. Cargo then reproduced CI's artifact hash
(`kernel-7f83536acfad25b4`, `kernel-04da2562c61a7429`), which makes the symbolization exact rather
than plausible.

On aarch64 the two addresses cohere into a story:

- `ELR_EL1` = `exception_vectors + 0x214`, which is the fifth `stp` of the same-EL synchronous
  entry stub pushing its 0x110-byte frame. The faulting store is the exception entry itself.
- `FAR_EL1` = the guard page's lowest byte, with the fault mid-guard: the entry stub had already
  cascaded. A store into the guard raises a same-EL sync abort, whose entry pushes another frame
  0x110 lower, which faults again, and so on down the guard page. The dump we finally get is the
  cascade's last inner fault, printed once a frame lands whole in the mapped page below the guard.
- `x30` = `IrqSafeMutex<Option<Scheduler>>::lock + 0x190`, the instruction after that function's
  `bl spin_loop_hint`, a value that is only live inside the contended spin of `SCHED.lock`.
  The thread that died
  was spinning for the scheduler lock with interrupts already masked, within ~272 bytes of its
  stack bottom, and the deepest call of the spin loop is what first touched the guard.

The riscv64 report agrees on everything measurable (`stval` = guard base, same test, same moment)
except that its `sepc` points at an `auipc` in a test-runner print, an instruction that cannot
raise a store fault. `thead-c906` under QEMU is the model whose timing already surfaced one
unrelated flake that day; treat a c906 `sepc` in this failure class as approximate and lean on
`stval`.

## The arithmetic, which is the actual cause

No single frame was over the guard page; the gate is green on the failing SHAs. The stack was
consumed by an honest sum (aarch64 debug numbers, from `-Z emit-stack-sizes`):

| layer | cost |
|---|---|
| deepest standing path the suite reaches on a thread stack | ~11.7 KiB (the measured high-water) |
| blocking from that depth: `ipc_receive` 656 + `SCHED.lock` 256 + `schedule` 448 + the switch | ~1.4 KiB, resident while blocked |
| one preemption at the deepest instant: trap frame 272 + dispatch + GIC claim + `canary::check` 592 + `schedule` 448 + contended `SCHED.lock` 256 + spin | ~2.3 KiB |

Total ~15.5 KiB against 16384 bytes, and the load correlation falls out of the last row. QEMU's
timer runs on host wall clock, so a loaded host delivers many more timer interrupts per guest
instruction. One of them eventually lands on the deepest frame of the deepest thread, with the
scheduler lock contended by the other core's death-report work. That is why the fault sits right
after the kill report. Every layer is doing its job; the budget was simply spent.

## Why growing the stack IS the fix for this shape

The section above says "growing the stack does not fix this shape", and both sentences are right
because the shapes differ. That rule is about a single frame bigger than the guard page, which
steps over the guard and corrupts silently; growing the stack leaves the silent step-over silent,
and shrinking the frame is the fix. Here every frame is modest, the guard page fired exactly as
designed, and the failure is the sum. For a sum, the levers are shrink the chain, bound the
chain, or grow the budget:

- Shrunk: `sched::canary::check` reserved its whole 592-byte frame before its disarmed early
  return, on every tick, on every thread stack. It is now a ~16-byte armed-check wrapper around an
  outlined `#[inline(never)]` body (a debug prologue reserves the whole frame no matter how early
  the return; an early `return` is not an early frame).
- Grown: `thread::STACK_PAGES` 4 to 6 (24 KiB), sized against the sum above with ~8 KiB over the
  measured worst case. The thread high-water tripwire moved from 14336 to 18432 so it alarms ~3 KiB
  past the measured worst-case stacking and 6 KiB before the guard. The old limit could pass a run
  whose true worst case was already past the stack, which these two CI runs proved by example.
- Bounded, on 2026-08-16: the structural fix is a per-CPU interrupt stack, so a preemption stops
  billing the interrupted thread ~2.3 KiB at its deepest instant. This bullet said "not yet bounded"
  and named it as wanting a lane of its own. It got one the next day, on both ISAs, and the last
  section of this note has the numbers and the honest account of how much they moved. What is still
  billed to the interrupted thread is the trap frame and the deferred `schedule()`.

What the enlargement cost elsewhere, which took three suite runs to find. Thread stacks come from
the kmem carve, not the frame allocator, and 6 pages x `MAX_THREADS` is 768 pages, the whole
768-page carve. The carve grew to 1024, which in turn took the last spare megabyte of the 128 MiB
test machine. The machine therefore grew to 256 MiB (both runners, and memory.rs's RAM assert moves
with them). Both exhaustions surfaced the same way: an unrelated test's spawn failing late in the
aarch64 suite with a message that ORed two causes. The refusal sites in `kmem::page` and the shell
wiring now print which budget said no.

The carve moved again on 2026-08-27, to 2048 pages, when `sched::MAX_THREADS` doubled to 256:
seven pages a live thread (six of stack, one of TCB) is 1792, endpoints are about 60, and the rest
is slack. The machine did not have to grow with it this time, because 256 MiB already had the room.
See `kmem::KERNEL_OBJ_PAGES` for the arithmetic and `sched::MAX_THREADS` for why the ceiling moved.

The repeated fault address is a signature, not a coincidence. Every guard-page fault in this family
lands on the guard page's base (aarch64) or base and base+8 (riscv64), across days and across fixes.
That is arithmetic rather than evidence of one recurring caller. `sp` is 16-byte aligned, the entry
stub's stores walk upward from `sp`, and the cascade only ends once a frame clears the page-aligned
guard base. So the terminal faulting store is always the first aligned address at or above the base.
aarch64's 16-byte `stp`s give exactly the base; riscv64's 8-byte `sd`s give base or base+8, which
are precisely the two values ever observed. A depth-driven overflow through the entry-stub cascade
therefore DOES repeat an address, exactly this one; do not read address stability as proof of a
single fixed-site writer.

The overflow report also got the instrument this diagnosis lacked: on a thread-guard fault,
`stack::warn_if_guard_page` now prints every word of the dead stack that points into `.text`,
deepest first. This kernel keeps no frame pointers, so that conservative scan is the only
backtrace it can produce, and it turns the next CI-only report into a symbolizable chain instead
of a container-rebuild archaeology project.
