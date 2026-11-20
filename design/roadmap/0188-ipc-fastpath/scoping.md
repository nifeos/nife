# The scoping: the starting numbers and three findings, 2026-08-28 to 2026-09-04

An appendix of [milestone 188 (the IPC fastpath)](../0188-ipc-fastpath.md). The main page is
complete without it and keeps each finding as a summary. This file keeps the starting numbers, their
sources, and the working behind the three findings. Every figure here predates phases 1 to 3; what
those phases re-measured is in [phases-1-to-3.md](phases-1-to-3.md).

## The starting numbers, and where each comes from

`bench/fastpath-{aarch64,riscv64}.txt` on `main` at fc7b04e7, and `bench/fastpath-x86_64.txt` from
pull request #574, unmerged at the time of writing. Written by `script/fastpath-footprint`, which
milestone 132 (the fast path's footprint) owns.

| | `ipc_fastpath` | `system_call_entry` | total |
|---|---|---|---|
| aarch64 | 5,788 | 3,304 | 9,092 (8.88 KiB) |
| riscv64 | 5,106 | 1,870 | 6,976 (6.81 KiB) |
| x86_64 | 6,639 | 1,637 | 8,276 (8.08 KiB) |

The target is in `notes/benchmarks/kernel-footprint-and-caches.md`'s own words ("The target"). It is
not restated here because that section states it better than a paraphrase would. In short: under
4 KiB of fastpath instructions and under 1 KiB of data touched per IPC. It is expressed as a fraction
of the smallest L1i among the machines this project runs on, so that it tracks the board list rather
than a number somebody liked. Every architecture is over it on `ipc_fastpath` alone, before entry is
counted. Read that section and the Liedtke argument above it before touching any of this. Both
`script/fastpath-footprint`'s header and milestone 132's block carry the same argument in shorter
form.

Two of the figures this milestone was briefed with are stale, and the corrections matter.
`riscv64`'s `system_call_entry` is 1,870 and not 1,866. And milestone 132's "the largest single item is
`system_call::dispatch` at 2,024 bytes", which is where this milestone's whole premise came from, was
true on 2026-08-18 and is not true now. Measured in this lane's own worktree from a release aarch64
kernel built at fc7b04e7:

```
$ llvm-objdump -d --no-show-raw-insn target/aarch64-unknown-none-softfloat/release/kernel
   2020  exception_vectors
   1160  kernel::system_call::dispatch
    124  exception_dispatch
```

`dispatch` is 1,160 bytes, down 864 from 2,024, and aarch64's `system_call_entry` is down by exactly
that same 864. Milestone 156 (extract the rest and ratchet both ways) did that, by moving the rare
administrative arms out of `invoke` into `#[inline(never)]` functions. That is the cheap option this
milestone was asked to price, and it has already been built once and measured.

## Finding 1: the gate measures the SEND/RECEIVE shape, and userspace overwhelmingly uses CALL/reply

`script/fastpath-footprint`'s root list is `ipc_send`, `ipc_receive`, `schedule`, `finish_switch` and
`current_cap`. So `ipc_fastpath` is the closure over a SEND/RECEIVE round trip: two endpoints, a
one-way message each way, four `svc`s from EL0. That is exactly the shape of the `ipc_rtt_el0`
benchmark, whose own doc comment says so (*"a server (RECEIVE request, SEND reply) and a client that
self-times a loop of SEND-then-RECEIVE"*).

It is not the shape the system runs. Counting imports of `crates/user_rt`'s helpers across
`user/src` and `crates`:

| helper | files importing it |
|---|---|
| `call` (`Rendezvous::CALL`) | 25 |
| `receive_cap` (`Rendezvous::RECEIVE_CAP`) | 15 |
| `reply` (`Reply::REPLY`) | 11 |
| both `send` and `receive` in one program | 12 |

24 of `user/`'s 68 `[[bin]]` programs import `call`, plus one crate. The 12 that import both
`send` and `receive` include the sink protocol's streaming shape and the supervisors, which are not
request/response at all. The kernel's own benchmark suite agrees with userspace and not with the
gate. `call_reply`'s doc comment calls it *"the one-endpoint shape real services use"*. Milestone
23's queue-broker note adds that the steady state of a capability swap over a stable endpoint *"is
[`call_reply`] above, instruction for instruction"*.

Measuring the shape userspace uses makes the gap worse, not better. The same closure was re-run with
`ipc_call`, `ipc_receive_cap` and `ipc_reply` as roots in place of `ipc_send`/`ipc_receive`, on the
same binary, same cold list, same script logic:

| closure | aarch64 bytes | over the 4 KiB target by |
|---|---|---|
| SEND/RECEIVE roots (what the gate measures) | 5,788 | 45% |
| CALL/reply roots (what a quarter of the programs run) | 7,516 | 88% |

The difference is not a different code path bolted on. It is that `ipc_receive_cap` (1,828 bytes)
and `ipc_call` (1,692) are each larger than `ipc_receive` (1,320) and `ipc_send` (952). They carry
the reply-capability mint, the capability-table insert into the server, and the `WaitRole::Reply`
parking that DECISIONS §12 (a one-shot reply capability) requires.

And icount cannot see any of it, which is the Liedtke argument arriving as a measurement rather
than as a quotation. Per-iteration icount ticks from `bench/baseline-*.txt`:

| | `ipc_rtt` | `call_reply` | delta |
|---|---|---|---|
| aarch64 | 1,017.3 | 1,040.1 | +2.2% |
| riscv64 | 170.0 | 174.9 | +2.9% |
| x86_64 | 16,734.6 | 17,150.6 | +2.5% |

The CALL/reply shape costs 2 to 3% more instructions on all three architectures and occupies 30%
more instruction footprint. A tripwire on retired instructions is structurally blind to that gap,
which is precisely the hole milestone 132 was built to close, and it is currently pointed at the
wrong shape.

## Finding 2: aarch64's `system_call_entry` is dominated by a symbol that is 94% never fetched

`exception_vectors` is 2,020 of aarch64's 3,304, and it is not code in the ordinary sense. It is the
hardware's vector table: `kernel/src/arch/aarch64/vectors.s` builds it as sixteen `VECTOR_ENTRY`
macros at `.balign 0x80`, with the comment *"The hardware requires 2048-byte alignment. 16 entries x
128 bytes = 2048."* A system call from EL0 traps to exactly one of those sixteen entries (index 8, lower
EL AArch64 synchronous) and fetches at most 128 bytes of the table. The other 1,892 are page faults,
IRQs, kernel-mode traps and the AArch32 entries this kernel will never support.

So aarch64's honest entry footprint is about 1,412 bytes (one 128-byte vector entry, plus
`exception_dispatch` at 124, plus `dispatch` at 1,160), not 3,304. Two consequences, and the second
is why this is in the milestone rather than in a bug report:

- The three architectures' `system_call_entry` figures are even less comparable than pull request #574
  already recorded them to be. That note explains why x86_64's 1,637 is small (a `system_call`
  instruction reads `IA32_LSTAR` and consults no IDT entry, so its entry set is four symbols); it
  does not yet say that aarch64's 3,304 is large for the mirror-image reason.
- A lane could "improve" aarch64's number by 1,892 bytes without changing one fetched
  instruction. Counting one vector entry instead of the whole table is a correct accounting fix
  that would look like a 57% win. That is a gate that can be gamed, which is the one property
  AGENTS.md's ranking function says a measurement must not have.

`exception_restore` (92 bytes) runs on the return leg of every system call and is in neither figure. It
is a small under-count in the other direction and should be added when the table is fixed.

## Finding 3: where the bytes are, per symbol

The whole closure, aarch64, both shapes, so a reader can see what a fastpath would be skipping
rather than guess at it.

| symbol | SEND/RECEIVE closure | CALL/reply closure |
|---|---|---|
| `sched::ipc_receive_cap` | | 1,828 |
| `sched::ipc_call` | | 1,692 |
| `sched::ipc_receive` | 1,320 | |
| `sched::schedule` | 1,244 | 1,244 |
| `sched::ipc_send` | 952 | |
| `sched::finish_switch` | 824 | 824 |
| `sched::ipc_reply` | | 480 |
| `sched::wake` | 452 | 452 |
| `sched::current_cap` | 372 | 372 |
| `kmem::recycle` | 280 | 280 |
| `memcpy` | 272 | 272 |
| `switch_to` | 72 | 72 |
| total | 5,788 | 7,516 |

`schedule` + `finish_switch` + `wake` + `kmem::recycle` + `switch_to` is 2,872 bytes, 38% of the
CALL/reply closure. It is the general scheduler being asked to re-pick a thread that the IPC already
knows the identity of. That, and not `system_call::dispatch`, is the largest structurally skippable
block on this path. `system_call::dispatch` at 1,160 is now about an eighth of an honestly counted round
trip (7,516 of closure plus the ~1,412 of entry that is fetched). And 864 of its former bulk has
already been removed by the cheap method.
