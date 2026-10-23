# The x86_64 kernel page tables

*Name: provisional, like every stem in this directory.*

This page holds the reasons behind `kernel/src/arch/x86_64/mmu.rs` and the files under `mmu/`: the
two kernel bases, the direct map's claims, the 32-bit MMIO hole, and the user address spaces. The
W^X kernel map itself and the direct map in blocks are in
[interrupts-and-the-fine-map.md](interrupts-and-the-fine-map.md). The cross-core TLB shootdown is in
[x86-tlb-shootdown.md](../x86-tlb-shootdown.md).

## Two kernel bases

This is the one structural difference from aarch64 and RISC-V. `code-model: kernel` pins every
symbol in the top 2 GiB, so the image cannot share a base with a direct map of more than 2 GiB of
RAM. So, as on Linux:

| What | Base | Who needs it there |
|---|---|---|
| The kernel image | `KERNEL_VA_BASE`, `0xffffffff80000000` | the code model, non-negotiably |
| The direct map of physical memory | `DIRECT_MAP_BASE`, `0xffff888000000000` | `phys_to_virt`, with room for 64 TiB |

They are separate PML4 entries (511 and 273), both canonically high. `virt_to_phys` inverts both
bases, because the kernel asks it about image addresses too, as Linux's `__pa()` does.
`memory::image_start` hands it a linker symbol, and `sched` and `kmem` hand it direct-map pointers.

`DIRECT_MAP_BASE` is Linux's `page_offset_base`, taken rather than invented because a reader who
has met one x86_64 kernel has met the number. `THREAD_STACK_AREA`, PML4[402], is Linux's
`VMALLOC_START` for the same reason. The other two architectures compute their stack area as
`KERNEL_VA_BASE | 0x10_0000_0000`, which works where the kernel base is a half base with room above
it. Here `KERNEL_VA_BASE` already has every bit of `0x10_0000_0000` set, so the OR is the identity
and every kernel thread stack would land on the image's own base, over `.text`. The mapper's
overwrite refusal would report that as `kmem: no memory to wire one`, far from the cause.

`phys_to_virt` means one thing for the machine's whole life. `boot.s` gives the boot tables a
PML4[273] entry over the low 4 GiB before the first Rust instruction, so the frame bitmap, the PVH
structure and the ACPI tables keep the pointers they took early. `mmu::init` widens the direct map
rather than introducing it, and it drops the boot identity map, which was an alias of all physical
memory in the half user programs get. `boot.s` writes the PML4 index as the literal 273, because a
32-bit assembler cannot compute it, and a compile-time assertion in `layout.rs` keeps the two equal.

## What the direct map claims, and in which order

`claims::direct_map_claims` lists every range the fine map gives the direct map. The kernel image's
frames are skipped, because at a different base a direct-map entry for them would be a second,
writable alias of `.text`, and W^X that a second mapping undoes is not W^X. One gap inside the
image range is mapped by name: the AP trampoline's load address, which
[smp-bring-up.md](smp-bring-up.md) explains.

The device windows come before the firmware reservations, and the order is load-bearing. The
reservation fill is guarded and the device claims are not, so whichever runs first decides the
memory type of a page both want. Only one answer is correct: a cacheable mapping of a device window
is a write that may sit in a cache line and never reach the device. Naming the windows first makes
device typing win by construction, without depending on the fill ceiling being right about where
the hole starts. Each window's address is the machine's own where ACPI gives one: the MADT's local
and IO APICs, the DMAR's register files, the MCFG's ECAM window. `IA32_APIC_BASE` is relocatable,
and a mapped constant with the hardware elsewhere is a page that reads all-ones.

The framebuffer is the largest window by three orders of magnitude: 1920x1080 is 8 MiB, which is
2026 leaves, paid once. It must be mapped because the console has written to it through the boot map
since the tour's first line, and an unmapped framebuffer would fault inside `println!`, whose fault
handler prints. It is mapped write-combining, which [cpu-state.md](cpu-state.md) explains.

The initial ramdisk is claimed by its own recorded bounds. Attaching an NVMe controller grows the
ACPI tables enough that the reserved entry above the top of RAM widens over the ramdisk's last few
hundred bytes. PVH's loader places the ramdisk at a fixed offset below the top of guest memory,
sized for a smaller device set. Raising the fill ceiling is not the fix, because the ceiling is what
keeps the fill out of the MMIO hole.

## The 32-bit MMIO hole

`firmware_fill_ceiling` decides how high the cacheable fill may follow the loader's map. It starts
at the low megabyte and extends while some entry begins at or below the current top and ends above
it. DRAM and the carve-outs firmware takes from it (ACPI tables, NVS, the SMM and graphics
reservations) are contiguous, so the chain covers them. The MMIO hole above is a gap the map does
not describe, so the chain stops at its floor. There is no 4 GiB constant: the hole sits below 4 GiB
because 32-bit BARs must be addressable, but its floor moves with how much memory firmware stole.
xenon's map is not sorted (it lists `0xa0000..0x100000` after a 16 GiB region), so the loop passes
until nothing extends.

The rule it replaced was "the top of RAM", which works only for a machine whose RAM ends below the
hole. xenon has 16.7 GiB, so its top of RAM is `0x42e000000`, above every MMIO window it has, and the
fill claimed the local APIC's page cacheably before the device claim asked for it device-typed.
[x86-uefi-boot.md](../x86-uefi-boot.md) has that boot. `mmu/map_tests.rs` keeps xenon's map as a
fixture, so QEMU's 2 GiB machine tests the rule against it.

`memory_mapped_io_window` is where the kernel may place PCI BARs, milestone 256 (x86_64 places PCI
BARs in a hardcoded window, and on xenon that window is RAM). The hardcoded `0xc000_0000` it
replaced is q35's conventional hole. On xenon low DRAM runs to `0xc894_0000`, straight through it.
A machine whose RAM ended just below the constant would have had BARs relocated over memory the
allocator owns, arriving as corruption somewhere unrelated. The answer has two sources that must
agree: the firmware map's gap and Intel's `TOLUD` register in the host bridge. QEMU's q35 does not
model `TOLUD` (zero on both the PVH and the OVMF path, measured 2026-09-04), so there the firmware
map is the only source. That is one of the machine's two answers, never a fallback to a constant.
The function takes `ecam` as an argument, because the boot tour calls it in order to fill
`memory::pci_regions`. The window itself is 2 MiB and mapped in 4 KiB leaves, which is why it is not
sized to the hole.

`layout::PCI_ECAM_PHYS` stays as the value `machine::print_acpi_summary` prints the MCFG's answer
against. On aarch64 and riscv64, which only boot QEMU `virt`, that second witness is an equality
invariant. Here firmware places the window: OVMF puts it at `0xe000_0000`, which both of
`script/test --arch x86_64`'s real-firmware boots print, and xenon's MCFG says `0xf000_0000` over
128 buses. The two agree only on q35 under SeaBIOS, which is why the comparison names q35
(milestone 314 (the x86_64 ECAM second witness stops reading as a defect on real hardware)).

## User address spaces

x86 has RISC-V's shape rather than aarch64's. `CR3` names the whole address space, as `satp` does,
so a process root must carry the kernel's high half, or the `mov cr3` unmaps the kernel executing
it. aarch64 needs no copy, because `TTBR1` is a second root every process shares. The kernel half
is PML4 entries 256 to 511, the `SPLIT_SHIFT = 47` line `Ia32e::is_in_half` tests. Both kernel bases
live there, so `share_kernel_half` shares the image, the direct map and every device window with one
copy. The entries point at kernel intermediate tables, so the map itself is shared rather than a
snapshot of it.

PCID, the ASID equivalent, lives in `CR3[11:0]` and is honoured only with `CR4.PCIDE` set. It is off,
so bits 11:5 and 2:0 are reserved-zero and a tag cannot be written at all. `ttbr0_value` drops the
ASID, and `flush_asid` flushes the whole non-global TLB on every core, which is the only
implementation that keeps `crates/address_space_identifier`'s reuse contract. With PCIDE on,
`ttbr0_value` would become `root | (asid & 0xfff)`, and that contract would gain a meaning here.
`CR4.PGE` is also off, so every `mov cr3` drops the kernel's translations too. That is why
`switch_user_root` skips a reload that would change nothing, comparing against the register itself.

## BUGS

- RAM is mapped with the largest leaf that fits (`largest_leaf`) without reading the MTRRs, trusting
  firmware to keep usable RAM write-back throughout. Device windows and firmware reservations stay
  in 4 KiB leaves, where mixed types live. On QEMU's 256 MiB the page-sized part was 560 KiB of
  tables.
- `mmu::init` must allocate its tables below 4 GiB, which is all the boot map reaches. A machine
  whose RAM starts above 4 GiB would fault in `map_everything`.
- `CR4.PGE` is off. TCG flushes on every `CR3` write regardless and icount cannot see a refill, so
  the cost is unmeasured (notes/benchmarks.md). Turning it on is two lines in `init` and
  `init_secondary`.
- `CR4.PCIDE` is off, so `flush_asid` flushes everything. It is not a bit to flip: with PCIDE on,
  `invlpg` and `CR3` writes act on the current PCID only, so `unmap_user_at`, `flush_asid` and the
  NMI shootdown would leave other spaces' stale entries alive until `INVPCID` covers every path.
- The cacheable fill follows the firmware map only while it is contiguous. ACPI tables above an
  undescribed gap inside low DRAM would fault.
- An aperture that firmware also reports as usable RAM panics, naming both ranges, on purpose. The
  fix is excluding it from both the direct map and the allocator, not a second mapping, because two
  mappings of one frame with different memory types are undefined on x86.
- A gap in the firmware map only promises that firmware did not call it RAM. That is the property
  that keeps device registers off memory the allocator owns, and it is weaker than knowing what the
  host bridge routes. Windows no table names are a residual risk.
- SMAP is off, so ring 0 can load and store through user pages, as aarch64 never sets `PSTATE.PAN`.
  Turning it on is worth doing, for the reason RISC-V leaves `SUM` clear: a kernel bug that strays
  into the low half should fault rather than succeed. Nothing has measured its cost on the syscall
  path, and a protection turned on without a number is the change this tree asks evidence for.
