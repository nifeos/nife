# riscv64 virtual memory: Sv39, one `satp`, and the device window

An appendix to [notes/riscv-port.md](../riscv-port.md). It carries the design behind
`kernel/src/arch/riscv64/mmu.rs` and its `tests.rs` and `proofs.rs`. The portable page-table
format is `paging::Sv39` ([notes/page-tables.md](../page-tables.md)). The ASID and shootdown
design is [notes/riscv-tlb-shootdown.md](../riscv-tlb-shootdown.md).

## The layout

The kernel lives in the Sv39 high half, `KERNEL_VA_BASE = 0xffff_ffc0_0000_0000` (bits 63:38 all
one). It is chosen like aarch64's base, so `VA = PA | KERNEL_VA_BASE` is exact and a kernel address
shares its physical address's page-table indices. Kernel thread stacks live 64 GiB up, far above
the direct map, so a stack address never collides with a physical address's virtual name.

`boot.s` turns paging on with a coarse boot table and jumps to the high half. `mmu::init` then
builds the fine W^X tables and switches `satp` to them. The fine tables map the same kernel
addresses to the same frames, so the next fetch after the `csrw satp` resolves identically.

## One `satp`, so every process root carries the kernel

aarch64 has `TTBR0` for the user and `TTBR1` for the kernel, so a process switch swaps only
`TTBR0`. RISC-V has one `satp` for the whole address space. So every process root carries copies of
the kernel root's top-level entries 256 to 511 (`share_kernel_half`). They point at the shared
kernel intermediate tables, so copying the entries shares the whole kernel map. A thread with no
user space runs on the kernel root itself, whose empty low half faults every user address. That
emptiness is held only by `map_everything` mapping nothing low. If it did, a stray low kernel
pointer would succeed instead of faulting, which a test checks.

`switch_user_root` returns early when the live `satp` already equals the target. Every switch
between two kernel threads names the reserved root, and `satp` carries the ASID in the same word,
so "same address space" is one compare against what the hardware is walking. `switch_user_root`
and `write_satp` are `#[inline(always)]`. LLVM outlined them once a cold caller appeared, and
merged `write_satp` with the identical `activate_user`, and `script/fastpath-footprint` caught a
hot symbol outside `.text.hot`. Forcing both keeps the hot path independent of cold callers.

## The device window

Under Sv39 the high half is 256 GiB, so `pa + KERNEL_VA_BASE` can name only the first 256 GiB of
physical space. The T-Head TH1520 puts every peripheral at `0xff_d800_0000` and up, 1 TiB up. So
the kernel names one fixed gigabyte rather than growing a device allocator like Linux's `ioremap`:
physical `0xff_c000_0000..0x100_0000_0000` appears at `0xffff_ffff_c000_0000`, root index 511.
The direct map is capped at 255 GiB (`DIRECT_MAP_LIMIT`) so the two cannot meet. Devices in the
window are mapped page by page exactly as low ones are; the window decides only where they are
named. A machine with devices in two distant gigabytes would need a different answer.

`phys_to_virt` is one branchless formula for both, because it is inlined all over the IPC fast
path, and a compare and branch measured +122 bytes on `ipc_call_reply`. The window's physical base
is 255 GiB modulo 256 GiB, exactly where its virtual base sits above `KERNEL_VA_BASE`. So keeping
the low 38 bits and OR-ing in the high half maps RAM below 255 GiB to `pa + KERNEL_VA_BASE`, and
the window to its own virtual base.

That formula rests on every address handed to it being below 255 GiB or inside the window; any
other silently aliases another frame. It is checked where there is no cost per call.
`refuse_unnameable_ram` runs first in `init` and panics on RAM reaching the limit, and
`direct_map` asserts every device range. The boot table maps only low gigabytes, so before that
check a high frame faults rather than aliases. QEMU `virt` has a few GiB from `0x8000_0000`, radon
8 GiB from `0x4000_0000`, and the TH1520 16 GiB from 0. A machine with more RAM needs a second
window or Sv48.

## The boot table

The boot table is one root of RWX gigapage leaves, like aarch64's coarse boot map; it lives about
twenty instructions. Entries 0 to 2 identity-map the UART region, the VisionFive 2's first gigabyte
of DRAM, and the kernel's RAM. Entries 256 to 258 alias them at `KERNEL_VA_BASE`. `boot.s` loads
its physical address PC-relative, so it is a real static in `.data` with a stable symbol.

Gigapage 1 is there for the board. The JH7110's DRAM starts at `0x4000_0000` and U-Boot's default
`fdt_addr_r` is `0x4600_0000`, so without it a device tree at the default address faults before the
trap path can print ([notes/visionfive2.md](../visionfive2.md)). On QEMU `virt` that range is the
32-bit PCI window, which nothing touches while the boot table is live.

The TH1520 build maps all 16 GiB of its RAM, because its U-Boot places the device tree and the
archive wherever `bootm` decides and offers no prompt for `fdt move`. Its leaves carry T-Head memory
types, since firmware turns `XTheadMae` on and plain Sv39's zeros there are undefined: memory is
cacheable, bufferable and shareable, devices strongly ordered and shareable. These are Linux's
`_PAGE_PMA_THEAD` and `_PAGE_IO_THEAD`, the same values `paging::Sv39Mae` writes. Every other
build keeps those bits zero, because there they are reserved and a set one is a page fault. Entry
511 maps the device window, high alias only, so a console there can print before `mmu::init`.

## What the fine tables map

The direct map covers all RAM read/write and never executable, less the kernel image, whose
sections get W^X permissions. The mapper refuses to overwrite a live leaf, which turns an ordering
mistake into an error rather than a wrong permission. RAM goes in the largest leaf that fits;
device windows stay in 4 KiB pages, the choice the x86 port also makes (`arch/x86_64/mmu.rs`'s BUGS
on the MTRRs). The boot stack, each secondary stack and each interrupt stack are mapped slot by
slot, so every slot's bottom page stays an unmapped guard. `verify` asserts each guard is a hole
before the switch.

Every device window beyond the UART is mapped only when the device tree names the device. That
refusal to overwrite is why ([notes/visionfive2.md](../visionfive2.md)). QEMU's BAR window at
`0x4000_0000` is the JH7110's DRAM base, and its virtio window at `0x1000_1000` is TH1520 DRAM and a
JH7110 UART. Mapping either from QEMU's constants ends the boot at the overwrite. `virtio_slots`
reads whether the virtio window was mapped, so the probe never reads one that is not. The
`sifive_test` finisher is QEMU's alone and is not mapped under the `board` feature.

The ECAM mapping is bus 0 only before a survey (`pci::ecam_buses()`), 4 KB per function and 1 MB
per bus. QEMU `virt` is a flat root complex, and a 4 KB-page map of all 256 buses would cost 64K
PTEs for space that reads all-ones. No survey runs on this architecture. The BAR slice is 2 MB:
OpenSBI programs no BAR, so `kernel/src/pci.rs` places them, and a virtio register BAR is 16 KB.

The JH7110 windows are each guarded on the device tree. The STG clock-and-reset generator ungates
the TRNG's clocks. The SYS one ungates I2C5 for the soak's PMIC reset and gmac0's transmit and PTP
clocks. The SYS syscon holds the PLL words the I2C clock computation reads. The Ethernet port,
SD/MMC controllers and watchdog get only the registers their drivers touch.

## ASIDs, and when a switch flushes

`satp.ASID` is WARL: an implementation may hardwire any of its 16 bits to zero, all of them
included. `crates/address_space_identifier` hands out 255 numbers, below aarch64's mandated 8-bit
minimum; RISC-V mandates none. The VisionFive 2's U74 implements zero bits. On such a hart every
space carries ASID 0 and their TLB entries would alias.

`probe_asid_bits` writes ones into the live `satp`'s ASID field, reads back what stuck, and
restores it, each write followed by a sweep. Only the tag moves, so the worst case is misses that
re-walk the same tables. WARL bits need not be contiguous, so it counts them. `ASID_BITS_NEEDED` is
derived from `ASIDS`, so raising `ASIDS` moves the gate. `asid_tagging_is_trusted` is false until
the probe runs and settles once at boot, before any secondary or user space exists. When false,
`write_satp` sweeps the whole TLB on every switch: correct and slow, where a panic would refuse a
machine that works.

## TLB maintenance

`sfence.vma` orders the preceding page-table write and completes locally, so no separate barrier is
needed, unlike aarch64's `tlbi`. RISC-V may need one to see a freshly valid leaf, so a new user
mapping is flushed too. There is no hardware broadcast, so other harts are reached through SBI
RFENCE, skipped until a second hart is online. `flush_asid` runs `sfence.vma x0, asid` locally
first, which leaves the global kernel half alone, then the remote RFENCE, which returns once every
other hart has run it.

A by-address invalidate is a cross-core shootdown per page, 683 of them for a scanout. So
`UserMapBatch` fences each page locally and every other hart once over the whole span, from `Drop`,
so no early return skips it. `unmap_user_pages` discharges every page with one `flush_asid`, since
each was a translation with that tag; until it returns another core may hit a stale entry.
`cut_user_table` flushes by ASID because the walk caches above the leaves must go too.

Instruction memory has the same problem. `fence.i` orders only the local hart's fetch after its own
stores, with no address range and no broadcast form, so `sync_icache` pushes it to every other
online hart through RFENCE as well. TCG's icache is coherent and never shows the difference; on the
U74, a freshly built child process hung without it.

## Walking the live tables

`translate_user` builds its `Mapper` for the low half, so it answers `None` for any kernel address
before reading a table. `user_can_read`, `is_mapped_in_current_space` and the fault report walk
both halves, because a user thread reaching for kernel memory names a high-half address. RISC-V has
no `AT S1E0R`, so walking and checking `U` is the only way to ask what U-mode may read.

`is_mapped` walks the installed root without `KERNEL_MMU`, for fault handlers that may not block on
a lock whose holder may be the thread that just died. aarch64's twin needs no lock at all, because
`TTBR1_EL1` is separate.

RISC-V forbids S-mode loads and stores through `U` pages unless `sstatus.SUM` is set; aarch64
permits EL1 to read EL0 pages because this kernel does not enable `PAN`. A test that reads through
a user address to observe the TLB therefore needs `SUM` here. It is built only in tests. No syscall
dereferences a user pointer, so a shipping kernel that strays into the low half faults.

## What the tests can and cannot see

Base Sv39 has no memory-type field; that is the Svpbmt extension. `paging::Sv39` keeps the device
flag in an RSW software bit, and QEMU `virt` derives the real type from the physical address. So
the UART test checks the kernel's bookkeeping, not what the hardware was told. It still catches the
mistake worth catching, a UART mapped as kernel data. On aarch64 that mistake lets the CPU
speculatively read MMIO, and a speculative read of a UART FIFO consumes the byte.

Sv39 has one `X` bit whose privilege `U` decides, so asserting kernel `.text` is not
user-executable cannot fail today. It states the property for a format with separate bits.

## The model the proofs assume

The Kani harnesses in `proofs.rs` stub `csrr satp`, `csrw satp` and `sfence.vma`. The model is a
claim about the hardware, written from the privileged specification and unchecked against silicon:

- `satp` holds what was last written, except that ASID bits the hart does not implement read as
  zero. MODE and PPN are fully implemented, true on every hart this kernel boots and not required.
- `sfence.vma` is counted and does nothing else; the model has no TLB. A proof says whether a
  sweep is issued, never what it does.
- Nothing else runs: no interrupt, no other hart.

A hart whose `satp` write is not visible to the next `csrr`, or whose WARL behavior depends on
anything but the bit written, would falsify the model rather than the code. The QEMU and board
suites test the model; the harnesses test the code against it
([notes/kernel-proofs/stubbing-an-instruction.md](../kernel-proofs/stubbing-an-instruction.md)).

## BUGS

- A device tree near the top of an 8 GB VisionFive 2's RAM (`$fdtcontroladdr`, above 4 GiB) is
  outside the boot table's reach. The bench runbook's `fdt move` is the answer
  ([notes/visionfive2.md](../visionfive2.md)).
