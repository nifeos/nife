# The MMU (Memory Management Unit)

The piece of hardware that makes an operating system possible at all.

## The problem

Without an MMU, `load x0, [0x40001000]` reads physical byte `0x40001000` from the RAM
chip. Directly. Every program can read and write every byte of memory, including the
kernel's and every other program's. There is no such thing as a "process" in any
meaningful sense, because there is no wall between them.

## What it does

The MMU is hardware between the CPU and the memory bus. When it's on, every address the
CPU emits is a virtual address, and the MMU translates it to a physical address
before it reaches RAM.

Translation is driven by **page tables**: a tree the *kernel* builds in RAM and hands to
the hardware by writing its physical address into `TTBR0_EL1` / `TTBR1_EL1`. The kernel
writes the rules; the hardware then enforces them on every memory access, at full speed,
forever.

Three things fall out, and they are the foundation of modern computing:

Isolation. Each process gets its own page tables. Process A's virtual `0x1000` and
process B's virtual `0x1000` map to *different physical RAM*. Neither can name the
other's memory. Not "isn't allowed to." *Cannot express the address.*

Protection. Each mapping carries permission bits: readable, writable, executable.
Map code read-execute and data read-write, and a buffer overflow can no longer turn
injected data into running code.

Illusion. A process sees a clean, contiguous address space even though the physical
pages backing it are scattered wherever RAM was free. This later buys demand paging,
copy-on-write `fork`, memory-mapped files, and swap.

## Page faults

Touch an unmapped address, or violate the permissions, and the MMU raises an exception
and hands control to the kernel. That's a page fault.

Sometimes the kernel fixes it and resumes (the page was swapped out; copy-on-write needs
to duplicate it). Sometimes it kills the process. That's a segfault.

## Mechanics

Translation happens per page (4 KiB, typically), not per byte, because a per-byte
lookup table would be larger than the memory it describes.

Page tables are a radix tree. On aarch64 with 4 KiB pages and 48-bit addresses: 4
levels of 9 bits each (512 entries per table), plus 12 bits of offset within the
page. 9 x 4 + 12 = 48.

Walking a 4-level tree on every memory access would be brutally slow, so the CPU caches
recent translations in a TLB (translation lookaside buffer). Which means: **when the
kernel changes a mapping, it must invalidate the stale TLB entries.** Forgetting to is
one of the great classic kernel bugs, because everything works fine until it
catastrophically doesn't.

## The aarch64 two-register trick

aarch64 has two page table base registers, not one:

- `TTBR0_EL1` translates low addresses → userspace
- `TTBR1_EL1` translates high addresses → the kernel

The kernel lives permanently in the high half. On a context switch we swap only `TTBR0`;
the kernel's mappings never move. So a syscall requires no address space change at all.
x86_64 has one such register and achieves the same effect by convention. This is one of
the places aarch64's clean-sheet design visibly pays off.

## Turning it on is the sketchiest moment in the kernel

The MMU is off at boot. Addresses are physical.

The instant you flip the enable bit in `SCTLR_EL1`, the *very next instruction* is fetched
through the MMU. If you have not mapped the code that is currently executing, the CPU
tries to fetch from an address that no longer means anything, and the machine simply
stops existing.

You get one shot, and you cannot printf your way out of it. This is milestone 4, and it
is where the GDB stub earns its keep.

## What `kernel/src/arch/aarch64/mmu.rs` holds to

`boot.s` turns the MMU on with a coarse map: two 1 GiB blocks, all of RAM executable, nothing
read-only, enough to survive until Rust runs. `mmu::init` builds the fine map in `TTBR1_EL1`. It
holds the direct map of all RAM (read/write, never executable), the kernel image section by section
at its linked addresses (W^X), every stack with its guard page left unmapped, and the device windows
the device tree names. It walks the tables in software before installing them. The code doing the
switch, its stack and the UART must be mapped and every guard page must not be, because after the
switch a wrong map is a machine that stops without a word. A secondary core starts
on the coarse map and only repoints `TTBR1_EL1` at the same fine map, since its `TCR_EL1` and
`MAIR_EL1` already match.

The direct map puts RAM in the largest leaf that fits and device windows in 4 KiB pages. The mapper
refuses to overwrite a mapping, which makes break-before-make the only way to change one: aarch64
can raise a TLB conflict abort on a valid-to-valid change.

"Nobody is home" is a page table, not a control bit. With no user space installed, `TTBR0_EL1`
points at one zeroed L0 table, and a stray low address from EL1 takes a translation fault walking
it. The alternative, `TCR_EL1.EPD0`, gives the same protection but would make every switch between a
user thread and a kernel one a read-modify-write of the register that shapes translation itself.
With the table, installing or removing a space is one `msr ttbr0_el1`. This is Linux's design.

The switch flushes nothing. Every user mapping is `nG`, each space owns one ASID for life, and
the ASID rides in `TTBR0_EL1[63:48]` with the root, so an old space's entries stop matching rather
than being discarded ([address-space-identifiers.md](address-space-identifiers.md)).
`switch_user_root` compares the composed value and returns early when it is unchanged, which skips
two barriers. Invalidation happens at two places: revocation by VA across all ASIDs (`tlbi
vaae1is`), and teardown or a batch by ASID (`tlbi aside1is`). Both lead with `dsb ishst`, so the
other cores' table walkers see the page-table stores before the invalidate is broadcast; RISC-V's
`sfence.vma` orders the hart's own writes by definition. A newly valid leaf needs no invalidate,
only a barrier before the process touches it.

`switch_user_root` is an `unsafe fn` and not a type. Its obligation is liveness: the root must
outlive every EL0 instruction until the next `TTBR0_EL1` write, or the next user instruction runs in
a stranger's pages. A newtype only `AddressSpace::ttbr0` and `reserved_root` could mint would be a
`Copy` over a `u64`, and a copy outlives the `AddressSpace` whose frames get recycled. A borrow
would express it, and the scheduler cannot hold one: `sched::switch` reads the root under
`IPC_TABLES` and releases the lock before the switch, on purpose.

`TCR_EL1` has one trap worth naming: 4 KiB is `0b00` for `TG0` and `0b10` for `TG1`. The
`aarch64-cpu` crate spells both `KiB_4`. `IPS` comes from the ISA record, and `arch::isa::init`
refuses a part with no 4 KiB granule, so `TG0` and `TG1` are a checked assumption.

Asking the silicon whether EL0 could read an address: `AT S1E0R` translates an address as EL0
would and reports a permission fault into `PAR_EL1`, so `user_can_read` never re-implements the
permission model. `PAR_EL1` is one shared register, so interrupts are masked from the `at` to the
`mrs`. No syscall in today's ABI dereferences a user pointer, so only the system tests call it, and
they assert the silicon refuses a kernel address and allows the process's own text
([capabilities.md](capabilities.md) has the confused deputy this answers).

EL1 may read an EL0 page here because this kernel never sets `PSTATE.PAN`; RISC-V forbids it unless
`sstatus.SUM` is set, which is why `permit_kernel_access_to_user_pages` exists as a no-op on aarch64.

## Aside: why some chips can't run this OS

Microcontroller-class RISC-V and ARM parts (ESP32-C3, many Cortex-M) have no MMU.
No MMU means no virtual addresses, no isolation, no user mode as we mean it. They can run
an RTOS, but not the kind of OS we are building. This is why the RISC-V hardware we
considered had to be JH7110-class or better.

## BUGS

- **A user thread killed at a pc it was just running, under the reserved root, had no space in its
  scheduler record when it was switched in.** The switch installs `reserved_root()` (the kernel root,
  ASID 0) for a thread whose `space` is `None`, and its first user fetch faults. One path is known to
  cause it: a split read of the caller's own name in `sched::adopt_address_space`, which binds the
  new space to another thread. That thread then runs in a stranger's space. The read is now
  indivisible ([scheduler.md](scheduler.md), "The costs migration made real").
  - riscv64, cpu matrix run 38672377106 (2026-10-14 UTC), `rv64`. A credentialer service, spawned
    through `user::run`, was killed after the `ecall` in `user_mode_runtime::syscall::trap6`, under
    ASID 0, with no space in its record. It died starting up: the first `identity_provisioner` sat
    in its `CALL` on the provision endpoint, which had no receiver. Nothing else that clears or
    withholds a space applies to a kernel-spawned service, so this is that path.
  - riscv64, merge-group run 38284891615 (2026-10-12 UTC), `thead-c906`, and `x86_64`, merge-group
    run 38506029610 (2026-10-14 UTC), the AMD-Vi leg. `network_time_client` is spawned through
    `user::run`, so it fits either side. `date` gets its space from the progenitor's `CONFIGURE`,
    so it fits only as the thread handed another program's space. Neither report could tell, so
    neither is proven.

  The next sighting says which it is. In a `system_tests` build the riscv64 and `x86_64` kill
  reports compare the live root with the record's, and say when the record holds no space. A
  sighting under the thread's own root is some other cause. aarch64's kill report prints neither the
  root nor a walk, and should before the next occurrence lands there.

---

*Add to this file as new memory concepts come up.*
