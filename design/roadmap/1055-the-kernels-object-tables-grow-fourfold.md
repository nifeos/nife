---
status: BUILT
built: 2026-11-10
promoted_from: the-kernels-object-tables-grow-fourfold
raised: 2026-11-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1055. The kernel's object tables grow fourfold

Raised by lane/record-follow-on-items.

Reuse: the existing constants and `registry_footprint` in `kernel/src/user/address_space_registry.rs`,
`script/stack-frame-check` and `cargo xtask bench`. No new tool.

## Background

The kernel holds each kind of object in a fixed array sized at build time:
`sched::ipc_tables::MAXIMUM_THREADS` (256), `MAXIMUM_RENDEZVOUS` (512),
`sched::notification::MAXIMUM_NOTIFICATIONS` (256), `sched::timer::MAXIMUM_TIMERS` (256),
`memory_region::MAXIMUM_REGIONS` (256), and the process and address-space tables beside them. A
larger table used to cost boot stack, because installing the scheduler's tables built the whole
aggregate in one frame. The tables are now copied from a `static` into place, so their size no
longer reaches a stack frame, and `script/stack-frame-check` holds that function under the guard
page.

## The work

1. Raise each table to four times its size, one kind per commit.
2. Check every identifier and index field that carries a slot number for width, so no id wraps.
3. Measure, before and after, the kernel's `.bss`, boot time to the shell on each architecture, and
   the bench's instruction counts, and record them in this block.

## Done when

- Each table is four times its current size on all three architectures, with the measurements
  recorded.
- `script/stack-frame-check` passes, and no bench row leaves its band.

## Measured

The tables hold 1,024 threads, 2,048 rendezvous, 1,024 notifications, 1,024 timers, 1,024 regions and
1,024 processes, with 1,056 address-space registry slots (the thread table plus 32). Each table's
names carry a 32-bit slot, so no id wraps; the trace ring keeps 48 bits of a thread id and the
hot-swap survey word counts a three-child domain, so neither depends on the table size. The kernel's
object carve (`kmem::KERNEL_OBJ_PAGES`) is 8,192 pages, a const assertion keeps it at seven pages a
thread slot, and the `ps`, `pgrep` and `top` row buffers (1,024 rows) live in `.bss`. The ASID
allocator hands out 255 numbers, so those bound the live address spaces below the registry's size.

Section sizes of the default kernel (`cargo xtask boot-check`, debug profile), in bytes. The tables
live in `.data` (each static is not all zero bytes) with a second copy of the empty aggregate in
`.rodata` that `init` copies from, so the growth is there and `.bss` stays within one page.

| architecture | section | before | after |
|---|---|---:|---:|
| aarch64 | `.text` | 913,408 | 913,408 |
| aarch64 | `.rodata` | 168,507 | 259,251 |
| aarch64 | `.data` | 217,088 | 647,168 |
| aarch64 | `.bss` | 1,974,272 | 1,974,272 |
| riscv64 | `.text` | 720,896 | 720,896 |
| riscv64 | `.rodata` | 192,512 | 286,720 |
| riscv64 | `.data` | 217,088 | 647,168 |
| riscv64 | `.bss` | 1,957,888 | 1,966,080 |
| x86_64 | `.text` | 1,069,056 | 1,064,960 |
| x86_64 | `.rodata` | 180,224 | 270,336 |
| x86_64 | `.data` | 323,584 | 753,664 |
| x86_64 | `.bss` | 40,960 | 45,056 |

Boot time to the shell prompt, seconds under TCG, four boots each (`boot-check` reports it). The spread
between boots is larger than the difference between the medians.

| architecture | before | after |
|---|---|---|
| aarch64 | 3.9, 3.9, 4.3, 4.8 (median 4.1) | 3.9, 4.4, 4.5, 5.6 (median 4.5) |
| riscv64 | 4.5, 4.7, 5.0, 5.9 (median 4.9) | 4.6, 5.8, 5.8, 5.9 (median 5.8) |
| x86_64 | 5.4, 5.5, 6.3, 7.3 (median 5.9) | 4.8, 5.1, 5.2, 5.9 (median 5.2) |

Bench instruction counts, `cargo xtask bench --check` on each architecture (every row within its
band; the largest move is `spawn_reap` on aarch64):

aarch64

| row | before (ticks) | after (ticks) | change |
|---|---:|---:|---:|
| `yield_switch` | 1,121,753 | 1,122,754 | +0.09% |
| `ipc_rtt` | 1,056,317 | 1,057,566 | +0.12% |
| `relay_rtt` | 2,108,067 | 2,110,567 | +0.12% |
| `call_reply` | 1,092,191 | 1,093,317 | +0.10% |
| `broker_rtt` | 2,180,691 | 2,182,942 | +0.10% |
| `spawn_reap` | 216,926 | 217,604 | +0.31% |
| `map_new` | 16,429 | 16,432 | +0.02% |
| `coremark` | 20,905,120 | 20,905,121 | +0.00% |
| `null_syscall` | 355,003 | 355,003 | +0.00% |
| `ctx_switch` | 3,014,379 | 3,016,878 | +0.08% |
| `ipc_rtt_el0` | 10,608,817 | 10,619,799 | +0.10% |
| `sink_throughput` | 4,606,813 | 4,610,875 | +0.09% |
| `map_el0` | 302,758 | 302,946 | +0.06% |
| `spawn_el0` | 1,421,397 | 1,423,178 | +0.13% |

riscv64

| row | before (ticks) | after (ticks) | change |
|---|---:|---:|---:|
| `yield_switch` | 188,861 | 188,861 | +0.00% |
| `ipc_rtt` | 177,551 | 177,591 | +0.02% |
| `relay_rtt` | 354,331 | 354,410 | +0.02% |
| `call_reply` | 185,421 | 185,441 | +0.01% |
| `broker_rtt` | 370,291 | 370,330 | +0.01% |
| `spawn_reap` | 35,473 | 35,482 | +0.03% |
| `map_new` | 2,538 | 2,538 | +0.00% |
| `rfence_self` | 6,073 | 6,073 | +0.00% |
| `coremark` | 3,653,243 | 3,653,243 | +0.00% |
| `null_syscall` | 67,800 | 67,800 | +0.00% |
| `ctx_switch` | 514,300 | 514,300 | +0.00% |
| `ipc_rtt_el0` | 1,819,368 | 1,819,568 | +0.01% |
| `sink_throughput` | 781,940 | 782,040 | +0.01% |
| `map_el0` | 48,027 | 48,027 | +0.00% |
| `spawn_el0` | 231,979 | 231,984 | +0.00% |

x86_64

| row | before (ticks) | after (ticks) | change |
|---|---:|---:|---:|
| `yield_switch` | 19,014,060 | 19,014,060 | +0.00% |
| `tss_iomap_switch` | 23,814,063 | 23,814,063 | +0.00% |
| `tss_iomap_lazy_switch` | 23,647,045 | 23,647,045 | +0.00% |
| `tss_iomap_lazy_nop` | 19,526,060 | 19,526,060 | +0.00% |
| `ipc_rtt` | 17,560,066 | 17,560,066 | +0.00% |
| `relay_rtt` | 35,061,066 | 35,061,066 | +0.00% |
| `call_reply` | 18,211,063 | 18,211,063 | +0.00% |
| `broker_rtt` | 36,378,063 | 36,378,063 | +0.00% |
| `spawn_reap` | 2,944,398 | 2,944,398 | +0.00% |
| `map_new` | 192,082 | 192,082 | +0.00% |
| `coremark` | 306,137,561 | 306,137,561 | +0.00% |

`script/stack-frame-check` passes on all three architectures, with no function over the guard page.
At the new sizes the suite peaks at 96 of 1,024 threads, 213 of 1,024 regions and 274 of 2,048
rendezvous. No Kani harness or falsification patch in the tree names a changed constant: the proofs in
`crates/memory_regions` and `crates/inter_process_communication` take their sizes from the harness,
so a table's production size does not reach a proof.

## Follow-on

- **Proposed.** `design/roadmap/proposals/live-address-spaces-pass-255-with-sixteen-bit-asids.md`:
  the ASID allocator's 255 numbers bound the live address spaces below the registry's size.
- **Recorded.** `kernel/src/sched/ipc_tables.rs`: the tables sit in `.data` with a second copy in
  `.rodata`, about half a megabyte of image, in the `BUGS` of `MAXIMUM_THREADS`.

## Index row

The kernel's object tables are small enough to bind early. This grows each fourfold on all three architectures, with the memory and stack measurements recorded.
