# The current-CPU page

`crates/current_cpu_contract` defines the page through which a thread reads which CPU it runs on.
[§204 (how userspace asks where a thread runs)](../design/decisions/0204-how-userspace-asks-where-a-thread-runs.md)
decided the mechanism: observing another thread is a rendezvous selector, and a thread observing
itself reads a per-thread page shaped like Linux's `rseq(2)`. This note carries the crate's design;
its doc comments link here.

## Why a page, and why one crate

The consumer is a per-CPU allocator, which asks once per allocation. A crossing would cost a decade
more than a load. It is a page and not a register because EL0 has no core-id register on `aarch64`
or `riscv64`, and §19 (architectural parity is a tenet) wants one mechanism. A page also has room
for more words, and it now carries the allowance and the scheduler tick.

The crate is the page's one definition. The kernel is the one writer, and
`user_mode_runtime::current_cpu` is every reader, so neither can drift on the layout or the address
(AGENTS.md rule 7).

## One writer, and it is the reader's own core

The kernel writes the CPU word in `schedule()`, on the destination core, on the way in.
`sched::adopt_address_space` makes the same write, arriving late. Writer and reader are therefore
the same hardware thread, and program order suffices. A migration hands a thread between two
writers, and the scheduler's own release/acquire pair on the run queue orders that handoff. So the
store in `publish` and the load in `CurrentCpuPage::cpu` are both `Relaxed`, and an aligned 64-bit
access does not tear on any target. A fence of its own would add nothing.

A reader can still be stale the instruction after it reads. A per-CPU cache built on this must be
correct, not merely fast, when the answer is the last core's.

`publish` does not check the magic. It runs on every context switch, and the page it names was
built by `build_page` two mappings earlier. The reader checks the magic on every call rather than
once at startup. The page is one cache line, so the check is in the line the answer is in and costs
a compare rather than a miss.

## Unset is not zero

Zero is a valid CPU id on every machine this tree runs on except the VisionFive 2, and "a wrong
number is worse than no number" (calef, 2026-09-21). So unset reads as `None`. A frame nobody
prepared lacks `MAGIC`, and a page whose thread never ran holds `UNSCHEDULED` (`u64::MAX`). The word
is the kernel's own cpu id, and the online set is not `0..count`. On the VisionFive 2 it is
`{1, 2, 3}`, so a per-CPU array is sized by `CPU_ID_BOUND` (see `crates/cpu_set`).

`CPU_ID_BOUND` is the kernel's compile-time `MAX_CPUS`, not the online set. It is right for sizing
and useless for iterating, and iterating is the selector's question.

## The magic and the layout

The page is the magic, then three little-endian `u64` words: the CPU, the allowance and the tick.
It is far under one frame and inside one cache line, so the reader is a single load on a line no one
else writes. The layout grows only by appending a word that reads zero where a kernel never wrote
it, so a new field never changes the magic. Every published program's `std` carries its own copy of
the magic and compares it first. No nife-built reader would notice a change, since each takes its
copy from the crate. Only the non-required `rg-threads` job runs a published one.

## The address

The page is `address_space_map::CURRENT_CPU_PAGE`, `0x7FFF_F000`, the last of the `PROCESS_PAGES`
band that the kernel maps into every process unasked
([notes/address-space-map.md](address-space-map.md)). It sits one page above the stack's top page,
in the same 2 MiB, so it shares the stack's L1, L2 and L3 tables. The mapping buys no table, only
its leaf.

The cost is in the page-table walk. On `aarch64` a 4 KiB-granule walk indexes L1 on bits 38:30, L2
on 29:21 and L3 on 20:12. An address sharing no gigabyte with anything the process maps needs three
fresh page-table frames, retyped and zeroed, on every spawn. `spawn_el0` measured that at 1,245
ticks per spawn, and 741 with only the L3 its own
([notes/benchmarks/spawn-el0.md](benchmarks/spawn-el0.md)). Milestone 206 (a program image has
under 896 KiB) and DECISIONS §171 (where a program image starts) option D set the map that put the
page beside the stack.

The same number serves all three architectures. `0x7FFF_F000` is inside every low half, Sv39's
included: its `SPLIT_SHIFT` is 38, so its low half ends at `0x0000_0040_0000_0000`. The host build
gets the same constant and maps nothing.

## Threads, and the shape that gets no page

A process has at most `THREAD_PAGE_SLOTS` thread pages (§269 (how threads share a process) fork 6),
one page apart below `PAGE_VA`. Slot 0 is `PAGE_VA` itself, so a process's first thread reads where
every thread always has. Other threads read the address `CONFIGURE` answered, which `std` keeps in
thread-local storage.

A userspace-built address space never bound to a TCB has no page, and
`user_mode_runtime::current_cpu` there faults rather than returning `None`. The page is deliberately
not mapped in `user::user_address_space_create`, and that function's comment says why.

## Why the page type is neither `Send` nor `Sync`

`counter_frequency_contract::TimebasePage` is the same bytes for every thread, so handing one across
a thread boundary means something. `CurrentCpuPage` is this thread's answer, and carrying it to
another thread would carry a wrong one. The raw pointer makes the type neither by default. Every
hand-written `unsafe impl Send` or `Sync` in this tree is a claim that the compiler is wrong, and
`script/lint` counts them.

## The read-only `&AtomicU64`

The reader forms an `&AtomicU64` over read-only memory. Miri rejects that shape over an immutable
binding, so the tests reach every buffer through `as_mut_ptr`, read-only tests included. Whether the
shape is sound on a read-only mapping is unsettled. A raw atomic load that never forms the reference
is the fallback. The tests also give their buffers the frame's 8-byte alignment by hand, because a
bare byte array is 1-aligned and Miri reports the unaligned `&AtomicU64` in `publish`.
