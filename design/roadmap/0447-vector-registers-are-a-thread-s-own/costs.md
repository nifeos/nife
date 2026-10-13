# Milestone 447 appendix: what it costs, measured

An appendix to [milestone 447 (a thread's vector registers are its
own)](../0447-vector-registers-are-a-thread-s-own.md). The block is complete without it: its [cost
summary](../0447-vector-registers-are-a-thread-s-own.md#what-it-costs-measured) carries every
headline figure. This holds the tables and the working.

## The IPC fastpath: within bound on every ISA

`script/fastpath-footprint`, before and after, on this tree:

| ISA | `ipc_send_recv` | `ipc_call_reply` | `syscall_entry` |
|---|---|---|---|
| aarch64 | 6256 → 6300 (+0.7%) | 8190 → 8234 (+0.5%) | 1701 → 1701 (0) |
| riscv64 | 4632 → 4700 (+1.5%) | 5936 → 6004 (+1.1%) | 1870 → 1912 (+2.2%) |
| x86_64 | 5356 → 5432 (+1.4%) | 7028 → 7104 (+1.1%) | 1504 → 1504 (0) |

Inlined into `schedule`, `hand_over`'s expensive half would put riscv64's `ipc_send_recv` 7.5% over
the 5% bound and `ipc_call_reply` 5.8% over, for code that no thread in this tree executes.

The gate measures the transitive closure of non-cold calls, because Liedtke's argument is about
what a round trip evicts from L1i. Five hundred bytes of register-file machinery that no IPC runs
evicts nothing. So the half that touches registers is `#[cold]` and out of line, and so is
`enable_for_current`.

`#[cold]` here is a claim about this tree rather than a hint about taste, and it is checkable.
`crate::fp::ENABLES` counts the threads that have ever asked for the unit, and it is zero on every
shipping boot.

`enable_for_current` being cold matters a second time on RISC-V. It is called from
`riscv_trap_body`, one of the symbols this gate measures flat. The trap decoder's bytes are on
every `ecall`, so an arm that inlined a register-file load there would be charged to every syscall
this kernel serves. The decoder's own compare and branch is riscv64's `syscall_entry` row above.
That row is +2.2%, measured on 2026-09-20 on the shipped shape (commit `28e165e26`).


## The benchmarks: the switch costs one to three percent more instructions

`script/bench`, deterministic icount, against the committed baselines. Re-recorded in the commit
that moved them, per `bench/baseline-*.txt`'s own header.

| | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `yield_switch` | 1101149 → 1134719 (+3.0%) | 184875 → 188666 (+2.1%) | 18903108 → 19288726 (+2.0%) |
| `ctx_switch` | 2922971 → 3007320 (+2.9%) | 495050 → 504958 (+2.0%) | not measured on this ISA |
| `ipc_rtt` | 1026311 → 1045670 (+1.9%) | 169382 → 172373 (+1.8%) | 17068218 → 17252344 (+1.1%) |
| `ipc_rtt_el0` | 10755621 → 10949404 (+1.8%) | 1829296 → 1847912 (+1.0%) | not measured on this ISA |
| `null_syscall` | 405004 → 410004 (+1.2%) | 72200 → 73029 (+1.1%) | not measured on this ISA |
| `spawn_reap` | 210224 → 214922 (+2.2%) | 33756 → 34277 (+1.5%) | 2779705 → 2830510 (+1.8%) |
| `coremark` | 20915599 → 20913256 (-0.01%) | 3654349 → 3654349 (0) | 306261408 → 306259744 (-0.001%) |

Every figure is inside the 10% tripwire. Read the table as three facts rather than one:

- A context switch costs about seventeen more instructions on aarch64 (550.6 → 567.4 per
  `yield_switch` iteration) and about 1.9 on riscv64. That is `hand_over`'s two loads and branch,
  plus the two `fp_state_of` offsets `schedule` computes under the lock.
- `spawn_reap` went the other way when the register file moved out of `Thread`: 215841 with the
  field inline, 214922 with it in the page. Spawning copies a `Thread`, and a smaller struct is a
  smaller copy. So the shape the stack gate forced is also the cheaper one to spawn. It is still
  +2.2% against `main`, because the initial write of the register file is real work a spawn did
  not do before.
- `coremark` did not move, on any architecture, and on riscv64 it is identical to the digit. A
  compute workload switches rarely, so the per-switch cost is invisible to it. The two numbers that
  went very slightly negative are code layout, not an improvement; nothing here makes arithmetic
  faster.

`null_syscall`'s +1.2% on aarch64 is the new `ec::FP_SIMD_ACCESS` arm in the exception decoder. It
is a quarter of an instruction per syscall: one compare, amortized over the arms that precede it.
It is identical between the two shapes, as it should be. Nothing about where the register file
lives is on the syscall path.

## The memory: none, and that is not a rounding of 544 down

The register file costs no memory at all. A `Thread` is always constructed at the start of a whole
4096-byte TCB page it exclusively owns. `Threads::insert_at` and `insert_at_in_place` both take
`phys_to_virt(page) as *mut Thread`, and every route into the table goes through one of the two.
`size_of::<Thread>()` is 1152, so 2,944 bytes of that page were already allocated and idle. The
register file (544 on aarch64, 528 on x86_64, 272 on riscv64) goes there.

`thread::fp_state_of` is the accessor, and one line is the whole of the mechanism that keeps the
fit true:

```rust
const _: () = assert!(FP_STATE_OFFSET + size_of::<FpState>() <= PAGE_SIZE);
```

Grow `Thread` past the point where the register file no longer fits beside it and the kernel does
not build. The one thing the page does not do for free is zero itself. So `Threads`' two inserts
call `thread::init_fp_state` right after the `Thread` is written. A `kmem` page carries whatever
its last owner left, and `live` is the first thing `hand_over` reads.

It is reached through the page pointer, never through a `&Thread`. A reference's provenance stops
at the end of the struct and this address is past it. So `Threads::pointer` hands back the page
cast unnarrowed, and both the context slot and the register file fall out of that one pointer.
That is one table probe per side, where the first shape's `get_mut` plus a field borrow was one as
well, so nothing was paid for the move.

## What the gate caught: a struct that gets copied through a caller's frame

The first shape put `FpState` inline in `Thread`, and `script/stack-frame-check` failed it. Not a
flake and not a tuning problem. The gate exists because a frame larger than the 4096-byte guard
page can move `sp` from inside a stack to below the guard in one step. It touches nothing in
between, so the guard never faults and the write lands in the neighboring thread's stack.

| | on `main` | inline `FpState` | in the TCB page |
|---|---|---|---|
| `spawn_into::<std_service::start_on_full>` | 3552 | 4672 | 3552 |
| `spawn_into::<fs_service::spawn_fs_server>` | 3536 | 4656 | 3536 |
| `Thread::spawn::<sched::init>` | 3504 | 5152 | 3504 |

The delta is 1120 for a 544-byte field, which is the finding. An unoptimized build materializes
the `Thread` value and then copies it, so a byte added to the struct costs two bytes of frame.
`Thread::spawn` wraps `spawn_into` and paid 1648, half again as much, because the value passes
through one more frame on the way.

[Milestone 124 (a thread is born where it lives: the spawn path's
copies)](../0124-a-thread-is-born-where-it-lives.md) is the block that already measured this
shape. It took `sched::spawn_on` from 3888-4592 down under the guard by handing the destination
pointer down. It stopped one hop short of removing the last temporary. 447 spent that remaining
headroom in one field.

Building the `Thread` in place, field by field, was priced and refused; it is a `## Follow-on`
entry in the block. It would fix the rest of the 3552 as well as this milestone's 1120. But it
converts a struct literal the compiler checks for completeness into twenty-five raw writes it does
not check. That is the wrong direction on `AGENTS.md`'s ladder, in a lane about floating point, on
the spawn path. Moving the register file one struct outwards, into memory that was already there,
gets the same numbers for none of that risk. That is why the table's third column is the first
column exactly.

`EXCEPT` was never a candidate. The check offers it for a function that provably cannot run on a
kernel thread stack, and `Thread::spawn` is the opposite of that case.

## The instruction clock: unchanged, byte for byte

`script/icount` boots under `-icount shift=0,sleep=off` and asserts the timer handler's instruction
count. Measured at this branch's base commit (`01d1cbf3`) and at its tip:

| | before | after |
|---|---|---|
| aarch64 `arrival_instructions` | min 1040 mean 1040 max 1040 | identical |
| aarch64 `handler_instructions` | mean 1088 max 1088 | identical |
| riscv64 `arrival_instructions` | min 300 mean 300 max 400 | identical |
| riscv64 `handler_instructions` | mean 800 max 900 | identical |
| missed ticks, early arrivals | 0, 0 | 0, 0 |

Not a surprise, and worth recording as the control it is. The timer handler does not switch
threads. §9 (locking: `IrqSafeMutex`, plus a discipline) puts the rule in a table row: "interrupt
handlers record and defer; they do not do work". So the switch happens a frame out, on the
interrupted thread's stack. Nothing this milestone added is inside the window that gate measures.
A number that had moved would have meant something was on a path it had no business being on.
