# Reading a guard-page report

*Name: provisional, for this stem. calef names files.*

A fatal kernel fault asks one question before anything else: is the faulting address in a kernel
stack's guard page, and whose? `kernel/src/stack.rs` answers it, in `guard_page_at` and
`warn_if_guard_page`. Both ISAs' fatal-fault paths call it with the faulting address. The answer
turns "unexpected trap at some address" into "this stack overflowed", and those two send a reader
to different places. This page says how the report is built and how to read it.

## The four kinds of guard page

Each kind of kernel stack gets its guard page a different way, which is why the report names the
kind.

| kind | where its guard page comes from |
|---|---|
| boot | the linker symbol `__stack_guard`, below the stack core 0 runs the test suite on |
| secondary | an offset into the `.bss` array of secondary stacks (`smp.rs`) |
| interrupt | the low page of a core's slot in `.interrupt_stacks` ([interrupt-stack.md](interrupt-stack.md)) |
| thread | the bottom page of a slot in the virtual stack area 64 GiB up (`thread::STACK_AREA`) |

A thread stack is reported by slot index, not by `ThreadId`. Turning a slot back into a thread
needs the scheduler lock, and the fault handler may not take one. The lookup takes no lock and
touches no allocator.

## The distance below the bottom

The report prints how far below the stack's bottom the access landed. A few bytes is an ordinary
overflow. Thousands of bytes means a single frame jumped most of the way across the guard. That is
the case where the next such frame jumps clean over it.

A `None` from `guard_page_at` does not mean the stack is fine. The guard is one page. A frame
larger than that can put `sp` below the guard without touching it and write into the neighboring
slot's top page, which is mapped and in use. That corrupts silently, and no lookup by address can
see it. `script/stack-frame-check` holds every frame to 4096 bytes for this reason, and
[stack-frame-sizes.md](../stack-high-water/stack-frame-sizes.md) has the frame that broke it.

## Why the trapped `sp` is printed beside the address

The faulting address and the stack pointer agree only when the fault really is `sp` walking off the
end. They come apart in two ways that look like an ordinary overflow.

- A store through a stray pointer that happens to name a guard page, with `sp` nowhere near it.
- A store just past the top of the stack in the slot below. Thread slots are contiguous and the
  guard is each slot's first page. So slot N's guard begins one byte past slot N-1's last usable
  byte, and a pointer that treats a stack top as inclusive lands there.

So the report prints `sp` in the same units, through `thread_stack_site`, and leaves the comparison
to the reader. The `sp` is the interrupted one, read from the trap frame. A trap from kernel mode
runs its handler on the core's interrupt stack, so the live `sp` would name an unrelated stack.
aarch64 computes it from the frame's own address; RISC-V reads the frame's saved `x2`.

One reading of "slot N-1" is a trap of its own. aarch64 has no double fault. If the vector's own
`SAVE_CONTEXT` store lands in a guard page, it re-enters the same vector with `sp` another 272 bytes
lower, again and again, until the frame fits. That leaves `sp` in the slot below and `FAR_EL1`
holding the first failing address, the same picture a store past a neighbor's top makes. The fault
PC separates them: inside the vector table means the walk, ordinary Rust means the store.
[guard-page-faults-2026-08-16.md](guard-page-faults-2026-08-16.md) worked this out on real dumps.

## The scan of the dead stack

For a thread or interrupt stack, the report ends by printing every word on that stack that points
into `.text`, deepest first. This kernel keeps no frame pointers, so a conservative scan is the only
backtrace it can produce. Most hits are return addresses. Some are spilled function pointers, or
stale words left by a previous tenant of a reused slot. Read them as candidates for
`llvm-addr2line`, not as a walked chain. The scan stops at 40 words so it cannot flood the serial
log the rest of the dump needs. The boot and secondary stacks do not get it yet.

A thread slot's pages are not always mapped. `KernelStack::drop` unmaps all of them and returns
the range to the free list, so a dead slot is a hole. The scan asks the page tables before each
page and reports the unmapped bytes rather than reading them. An unmapped stack under a fault means
the stack was freed while something still stood on it, which
[kernel-stack-freed-under-its-owner.md](kernel-stack-freed-under-its-owner.md) explains.

The scan runs last in the report. It is the only part that can itself fault, and a fault inside it
re-enters the handler and destroys `ELR_EL1` and `FAR_EL1`. Running it last means a surprise there
costs the scan and never the registers.
