# What phases 1 to 3 measured, 2026-09-04

An appendix of [milestone 188 (the IPC fastpath)](../0188-ipc-fastpath.md), moved out of it on
2026-10-12 (UTC) under §212 (a prose budget). The main page is complete without it: it keeps the
outcome of each phase and the after table. This file keeps the re-measurement, the before and after
for every architecture, the control run and the cost.

*Built 2026-09-04, one lane, pull request #732. Every figure here is from a release kernel in that
lane's worktree; the control runs are named where one was needed.*

## The premise held, and it had drifted a little

Re-measured at `61e30dcb`, the SEND/RECEIVE closure is 5,888 on aarch64 (the block said 5,788 at
`fc7b04e7`). The CALL/reply closure is 7,576 (the block said 7,516). `exception_vectors` is still
2,020 bytes of sixteen entries of which a system call fetches one. Both findings the milestone was built
on are true.

Two of the block's supporting counts were low and are worth correcting, because they were counted
honestly and the tree grew. Files importing `user_rt::call` is 35 rather than 25, `receive_cap` 23
rather than 15, `reply` 20 rather than 11. The one that moves the argument is `send`, at 64 files.
A bare `SEND` is widely used, and it is a genuinely different shape (one-way notification) rather
than half of a round trip. That is the argument for reporting two numbers rather than replacing one
with the other, and it is what the gate now does.

## Phase 1: the gate reports and checks two shapes

`ipc_send_receive` and `ipc_call_reply`, provisional names. `ipc_fastpath` survives as a derived
figure: the worse of the two, rather than the average or the sum, because one round trip is one
shape or the other. It keeps its old name because a dozen places in the tree cite it. It is recorded
rather than checked, since it is the max of two numbers that are each already checked.

`notes/benchmarks.md`'s "`SEND`, `RECEIVE`, `SEND`, `RECEIVE`" claim is corrected there: three
system calls to seL4's two, not four.

## Phase 2: aarch64's entry figure is honest, and nothing got faster

The figure now counts one 128-byte vector slot instead of all sixteen. It adds `exception_restore`
(92 bytes), which runs on every system call's return leg and was in neither figure. aarch64
`system_call_entry` went 3,304 to 1,508. riscv64 needs no equivalent (`stvec` is direct-mode, one
handler, no table) and x86_64 needs none (no IDT entry at all). So the asymmetry the milestone
predicted is confirmed in both directions, and is now argued in the script beside the `ENTRY` table.

## Phase 3: the cheap method does not transfer, and finding that out is the result

The `#[inline(never)]` extraction of milestone 156 (extract the rest and ratchet both ways),
applied to the closure, made the number bigger. Four
cold arms went out of line and aarch64's `ipc_send_receive` moved 5,888 to 6,220, because a closure
walk follows the new call and counts the same bytes under a new name. 156's method works on
`system_call_entry` because that half is flat. On a closure, extraction is a no-op unless the walk is
told the callee is cold.

So the gate now reads `#[cold]` out of the workspace's own source rather than from a
hand-maintained list inside itself. That is the AGENTS.md ladder moving up a rung. The claim lives
where a reader and the optimizer both already meet it, and a lane cannot widen the exclusion by
editing the gate. Outlining a genuinely cold arm is now credited automatically. `strand_callers_of`
and its two siblings dropped out of the regex and are carried by their own attributes.

The four arms: `finish_switch`'s reap, `schedule`'s killed-thread conversion (the *test* stays hot,
two loads and a compare; only the body moved), `schedule`'s self-pop heal, and `set_ipc_aborted`,
whose four call sites are all on these closures.

| | `ipc_send_receive` | `ipc_call_reply` | `system_call_entry` | total |
|---|---|---|---|---|
| aarch64, before | 5,888 | (7,576) | 3,304 | 9,192 |
| aarch64, after | 5,356 (-9.0%) | 7,028 (-7.2%) | 1,508 | 8,536 |
| riscv64, before | 5,122 | (6,390) | 1,828 | 6,950 |
| riscv64, after | 4,632 (-9.6%) | 5,936 (-7.1%) | 1,828 | 7,764 |
| x86_64, before | 6,767 | (8,657) | 1,637 | 8,404 |
| x86_64, after | 6,236 (-7.8%) | 8,122 (-6.2%) | 1,637 | 9,759 |

Parenthesized figures are what the same binary would have reported under phase 1's roots; they were
not measured before this milestone. A control was run to attribute the shrink. The new script logic
against the pre-extraction `sched.rs` reproduces the "before" row exactly on all three ISAs, so the
whole 6 to 10% is the extraction and none of it is the mechanism.

## The cost, measured rather than asserted

+0.17% retired instructions on `ipc_rtt` and +0.24% on `call_reply` (aarch64, icount, same tree with
and without the extraction). That is the outlined calls, and `script/bench --check` passes. No cache
effect was measured, because none can be: nothing in this tree models a cache and the HVF host's L1i
is several times the boards'.

## One of the block's open questions is closed structurally

`kmem::recycle` left the closure on every architecture. It was reachable only through
`finish_switch`'s reap arm, so a successful rendezvous cannot reach it. That is now a property of the
code rather than an assertion in a script.
