---
status: BUILT
raised: 2026-10-04
built: 2026-10-04
milestone_dependencies: 132, 188
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 758. The IPC fast paths shrink back inside their band

*(Number provisional until the merge queue lands it. Title and slug are drafts.)* Asked for by
calef on 2026-10-04 (UTC). `script/fastpath-footprint` fails a closure that grows more than 5% over
its recorded baseline. x86_64's `ipc_send_receive` sat at +4.4% on `main` (6,832 bytes against
6,544). riscv64's was +3.8% once #1611 (plain `RECEIVE`) and #1617 (64 capability slots) are
merged beside it. The growth was mostly #1600 (overflow checks in release builds, which calef
ruled on), then those two. The next IPC change would have failed the gate.

The goal is every measured closure on all three ISAs well inside the band, at or below baseline,
by shrinking code. Raising the tolerance, re-saving the baseline upward, and exempting a crate or
profile from overflow checks are all out of scope: the first two hide the growth and the third is
calef's call.

## Method

Measure first. Each closure was disassembled with line tables
(`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`, a separate target directory) and every byte
attributed to the source line it came from, summed over the closure. The ranking that came out
names the levers below. Each is one commit, so a conflict with a lane in the same files is one
lever's to resolve.

## What landed

Four levers, one commit each, all three ISAs. Figures are bytes from `script/fastpath-footprint`
on the lane's tip, against `main` at 6ac73e9f6, nightly-2026-10-04.

| ISA | closure | baseline | `main` | after | against baseline |
|---|---|---|---|---|---|
| aarch64 | `ipc_send_receive` | 5,644 | 5,612 | 4,840 | -14.2% |
| aarch64 | `ipc_call_reply` | 7,036 | 6,912 | 6,016 | -14.5% |
| riscv64 | `ipc_send_receive` | 4,902 | 5,002 | 4,048 | -17.4% |
| riscv64 | `ipc_call_reply` | 6,066 | 6,094 | 4,996 | -17.6% |
| x86_64 | `ipc_send_receive` | 6,544 | 6,832 | 5,335 | -18.5% |
| x86_64 | `ipc_call_reply` | 8,227 | 8,467 | 6,650 | -19.2% |

`system_call_entry` is unchanged by this milestone on every ISA (1,640, 2,008 and 1,797); on aarch64
and riscv64 it was already 1.7% and 1.6% over its baseline, inside the band. With #1611 and
#1617 merged beside it the closures are 4,828 and 6,088 on aarch64, 4,160 and 5,172 on riscv64, and
5,303 and 6,682 on x86_64, from 5,600, 6,940, 5,090, 6,260, 6,832 and 8,547 without it.

By lever, `ipc_send_receive` saved, aarch64 / riscv64 / x86_64:

1. The lock-order panic moves out of line (`sync::lock_order_violation`, provisional name):
   196 / 258 / 320. The `assert!` built its `fmt::Arguments` at every inlined `lock()`. Shrinking
   `lock()` let LLVM inline `current_cap` into `system_call::dispatch`, 804 bytes on aarch64's
   `system_call_entry`, so `current_cap` is pinned `#[inline(never)]` in the same commit.
2. `trace::record` moves out of line: 576 / 696 / 909. Each of an IPC round trip's events
   carried its own per-CPU read, `RINGS` bounds check and panic pad, sequence bump and store.
3. x86_64 reads its per-CPU pointer with one `gs`-relative load instead of `rdmsr IA32_GS_BASE`:
   236 on x86_64. `PerCpu`'s x86 trap scratch now holds the block's own address, written by
   `set_percpu`, the shape Linux's `this_cpu_off` has.
4. x86_64 pads `PerCpu` to 256, so `PERCPU[id]` is a shift there too and the power-of-two
   assertion holds on every ISA: 32 on x86_64.

The gate is symmetric (milestone 156 (extract the rest and ratchet both ways)), so a shrink past the band fails until it is saved. The
closures are saved at their new sizes. `system_call_entry` is not: aarch64 and riscv64 keep their old
floors by hand, because this milestone did not move them.

No overflow check was removed, exempted or replaced; the checks #1600 put in release builds are all
still there. No system call semantics, ABI or wire format changed.

## Cycles

Both instruments, before (`main` 6ac73e9f6) and after, on patagonia.

- `script/bench` (icount) builds a debug kernel, so it cannot see the release fast path these
  levers change, and it read accordingly. aarch64: `ipc_rtt` -1.8%, every other row within 0.4%.
  riscv64: `ipc_rtt` +2.2%, `ipc_rtt_el0` +0.6%, the rest within 0.4%. Bisected, the riscv64 move
  comes from lever 1, whose debug `lock()` is 23 instructions shorter. That points at where timer
  ticks land rather than at the code. x86_64 fell 2 to 3% on every switch and IPC row, which is
  lever 3: a debug `rdmsr` is a call through `read_msr`. All rows sit well inside the 10% tripwire.
- `script/bench --release` on HVF, five runs each, median ns per iteration: `ipc_rtt` 50 / 50,
  `call_reply` 69 / 70, `relay_rtt` 112 / 118, `broker_rtt` 141 / 139, `null_system_call` 30 / 30,
  `ctx_switch` 133 / 131, `ipc_rtt_el0` 442 / 423. A tie. The `relay_rtt` gap is run-to-run spread:
  levers 3 and 4 do not build into an aarch64 kernel, and runs of the lever-2 commit read 114 to 118.

x86_64 and riscv64 have no cycle measurement here; radon and xenon were not used.

## Follow-on

- **Milestone 787.** Milestone 787 (the IPC primitives look each thread up once). The next lever by size, resolving each thread name once per critical section,
  waits for #1611, #1617 and #1614 to stop rewriting the same bodies:
  `design/roadmap/0787-the-ipc-primitives-look-each-thread-up-once.md`.

## BUGS

- Line-table attribution assigns an instruction to one source line, and LLVM interleaves inlined
  bodies, so the per-line figures are close rather than exact. The before and after totals come
  from `script/fastpath-footprint` itself and are exact.

## Index row

The IPC fast-path closures shrink back inside `script/fastpath-footprint`'s 5% band on all three
ISAs, by outlining cold arms and dropping redundant checks, never by moving the budget.
