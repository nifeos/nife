---
status: BUILT
built: 2026-10-04
raised: 2026-10-04
milestone_dependencies: 753
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 754. The capability table grows to 64 slots

Raised 2026-10-04 (UTC) by milestone 753 (trace the progenitor's login block peak) as its option A,
which calef chose that day on pull request #1608. *(Title and slug are drafts.)*

## Why

A gpu-and-keyboard boot under QEMU reaches 31 of the progenitor's 32 capability slots, and the next
capability added at boot halts that boot with no message. Milestone 753's trace itemized the 31 and
replayed the alternatives. calef chose to grow the table rather than move holdings into a new
process: the fixed table is the real constraint, and a builder process that exists to move slot
accounting is not worth a new program and protocol. Milestone 753 records that reasoning in full.

## What it would do

- `kernel::cap::CAPABILITY_TABLE_SLOTS` and `abi::CAPABILITY_TABLE_SLOTS` go from 32 to 64.
- The table's free-slot word widens from `u32` to `u64`, and `capability_table::MAX_SLOTS` follows it.
  Every proof and test that checks the word against the array runs at the new width.
- `abi::fault::FAULT_EP_SLOT` is `CAPABILITY_TABLE_SLOTS - 1`, so it moves from 31 to 63. Every
  supervisor and every child agrees on that number, and they are all rebuilt from this tree.
- The same on aarch64, riscv64 and x86_64, per §19 (architectural parity is a tenet).
- The size assertion beside `CapabilityTable` in `kernel/src/cap.rs` records the new layout, and
  `CAPABILITY_TABLE_SLOTS`'s doc gets the raise and its reason, as each earlier raise did.

## The test

- `script/swish-check`'s gpu-and-keyboard boot reads 31 of 64 at peak, and `swish-check` passes on
  every leg.
- The TCB still fits in its page. `crate::thread`'s page-fit assertion is the check, and it fails the
  build if not. Measured before the change at 1,984, 1,968 and 1,712 of 4,096 bytes used (aarch64,
  x86_64, riscv64); 32 more slots add 1,024 bytes.
- `script/test` passes on all three, including the supervision tests that read the fault slot.

## What was built

All of the list above, on all three ISAs, and three things it did not predict.

- **The table is 1,032 bytes larger, not 1,024.** The wider word is eight bytes, so a table is
  2,064 bytes against 1,032 (`kernel/src/cap.rs` pins it). The TCB page still fits on all three: the
  size assertion holds, and the figures go from 1,984, 1,968 and 1,712 to 3,016, 3,000 and 2,744 of
  4,096 (arithmetic from the table's size).
- **The run-unvouched slot is 62.** `grant_plan::spawnproto::RUN_UNVOUCHED_SLOT` is a
  hand-written number that three binaries assert is `FAULT_EP_SLOT - 1`, so it moves with the fault
  slot at every raise, and `script/swish-check`'s census expectation (`21 62`) moves with it.
- **Two kernel stack costs had to be fixed.** See below.

## What it cost, measured

No new memory: the table lives in each thread's TCB page, which is already allocated whole.

IPC fastpath footprint (`script/fastpath-footprint`, bytes, before to after, on aarch64,
riscv64 and x86_64). No closure went over its band and the budget was not touched.

| Closure | aarch64 | riscv64 | x86_64 |
|---|---|---|---|
| `ipc_send_receive` | 5,612 to 5,612 | 5,002 to 5,118 | 6,832 to 6,832 |
| `ipc_call_reply` | 6,912 to 6,904 | 6,094 to 6,218 | 8,467 to 8,467 |
| `syscall_entry` | 1,640 to 1,640 | 2,008 to 2,008 | 1,797 to 1,797 |

x86_64 `ipc_send_receive` stays at +4.4% of its 5% band. riscv64 `ipc_send_receive` went from
+2.0% to +4.4% against its baseline, still inside, and is now the second one to watch.

Kernel stack temporaries.

- `script/stack-frame-check` failed at 64 slots on aarch64 and riscv64: `Thread::write_kernel_thread`
  at 4,960 bytes and `sched::adopt_secondary_idle` at 5,040, both over the 4,096-byte guard page,
  because an unoptimised build kept `CapabilityTable::new()` (2,064 bytes) as a temporary beside the
  `Thread`. Fixed by taking the empty table from a constant (`NO_CAPABILITIES`, name provisional) and
  by writing the idle thread into its TCB page in place (`Thread::write_adopted_current`). After:
  `write_kernel_thread` 2,896 (aarch64), 2,896 (riscv64), 2,856 (x86_64) and the gate passes.
- The system suite's boot stack went from under its 61,440-byte gate to 59,736 and 60,024 of 65,504
  on aarch64 and riscv64 and 64,912 on x86_64, which tripped the gate and was 592 bytes from the
  guard. The cause was the syscall fuzzer's model (`ACTORS` times `SLOTS` capabilities) living in
  `run_seed`'s frame, plus 2 KiB tables copied by value through its answer path. It is now a static
  with an in-place reset and the tables are read where they sit. After: 54,104 (aarch64), 55,160
  (x86_64) and 49,296 (riscv64) of 65,504 at the end of each suite run.
- A finding for whoever next raises the table: the fuzzer's `SLOTS` and every frame that holds a
  `[Option<Cap>; SLOTS]` scale with it, and `kernel/src/stack.rs`'s gate is what caught it.

`script/swish-check`: the gpu-and-keyboard boot's peak reads 31 of 64 on aarch64 (24, 28, 31
across its three boots), and the leg exits 0. `script/test` passes on aarch64, riscv64 and x86_64.
`CAPABILITY_TABLE_PEAK_MEASURED` stays 31. The record of the change is in
[notes/capability-peak-trace.md](../../notes/capability-peak-trace.md); the ABI fact (the fault slot
is 63) is on `abi::CAPABILITY_TABLE_SLOTS` and in notes/abi.md.

## BUGS

- The `script/swish-check` legs for riscv64 and x86_64 ran in CI, not locally; only aarch64 was read here.

## What it is not

Not growable tables. Whether a process's table should be sized for that process, as seL4's are, is
`design/roadmap/0778-capability-tables-sized-per-process.md`. Sixty-four slots buy the time to
answer that question with a measurement rather than at the next wall.

## Follow-on

- **Done.** The fastpath footprint and the stack temporaries the block said were unmeasured are
  measured and recorded in this block and in `notes/capability-peak-trace.md`.
- **Done.** The Kani harnesses that range over slots now cover 64 values: all five verify, in 0.03
  to 3.1 s, no slower than at 32 (`notes/process-view/the-machine-and-your-share.md` has the table).
  Two `crates/capability_table` falsification patches were re-cut for the `u64` word and replayed.
- **Recorded.** riscv64 `ipc_send_receive` is at +4.4% of its 5% band and x86_64's is at the same
  figure, so the next change to the fastpath meets the budget: the table in
  `design/roadmap/0754-the-capability-table-grows-to-64-slots.md`.
- **Refused.** Answering whether tables should be sized per process: it stays with
  `design/roadmap/0778-capability-tables-sized-per-process.md`, which this block deliberately
  did not decide.

## Index row

The progenitor's capability table sat at 31 of 32 on a gpu-and-keyboard boot. BUILT: the table is 64 slots on all three ISAs, the fault slot is 63 and the run-unvouched slot 62, and the same boot reads 31 of 64. The fastpath stayed inside its band. Two stack costs the wider table caused were fixed.
