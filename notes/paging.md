# The paging crate

`crates/paging` builds page tables for every format this tree walks: aarch64, Sv39, Sv39 with
T-Head's memory types, x86's IA-32e, and the two IOMMU formats VT-d and AMD-Vi. Each format
supplies a few encode and decode operations through `PageFormat`, and the walk is written once in
`Mapper`. The aarch64 descriptor, the TLB obligation and break-before-make are in
[page-tables.md](page-tables.md). The format trait's origin is §17 (the second architecture:
RISC-V, and the page-table format trait), and [riscv-port.md](riscv-port.md) covers Sv39.

## The shape

Every format uses 512-entry, 4 KiB tables and 4 KiB pages. A table is exactly one page, so the
frame allocator supplies page tables and nothing else. The same arithmetic gives 2 MiB and 1 GiB
leaves one and two levels up, which a direct map of RAM wants.

The crate is pure logic, so its host tests build real tables in host memory. A `Box<PageTable>` is
4 KiB-aligned by its type, so its address serves as a pretend physical frame, and the walk reads it
back with the kernel's own pointer arithmetic.

## x86_64

On aarch64 and Sv39 an intermediate entry carries no rights, and the leaf is the single source of
truth. x86 is the opposite. The effective permission is the AND of every level's `U/S` and `R/W`
bits, and `XD` at any level forbids execution. A leaf that says "user, writable" under a PML4 entry
that says "supervisor, read-only" is supervisor-only and read-only.

So `Ia32e` makes intermediate entries maximally permissive and lets the leaf decide. That gives the
other two formats' semantics exactly, and the shared walk stays correct without knowing any of it.
The trade is real. The hierarchical bits can revoke a whole subtree in one store, and this format
gives that up for one meaning of "what a mapping grants" across three architectures. A
subtree-revocation optimization would have to bring it back, and then explain itself to
`translate`, which reads the leaf alone.

Two leaf facts depend on state outside the tables, and the crate cannot check either:

- Bit 63 means "no execute" only while `IA32_EFER.NXE` is set. With NXE clear it is reserved, and
  setting it makes the entry fault. The boot trampoline sets NXE before it builds any table.
- Sv39 refuses a supervisor fetch from a `U` page unconditionally. x86 does so only while
  `CR4.SMEP` is set; with it clear, a page whose `XD` is clear runs at ring 0 whatever `U/S` says.
  `arch::x86_64::init` sets SMEP on every core whose CPUID offers it, and says on the console when
  one does not.

The type is `Ia32e` because "IA-32e paging" is Intel's own name for the mode, a term a reader
knows from outside this tree, which is the naming tenet's protected class.

## VT-d

VT-d's second-level tables have the shape of IA-32e, because they were designed for the same
walking hardware. The leaf encoding differs, and reusing `Ia32e` would be wrong. Checked against
QEMU's `hw/i386/intel_iommu_internal.h`, a second-level leaf has two meaningful bits, Read and
Write. Bits 2 through 10 are reserved, and QEMU's model faults on them. `Ia32e` sets `US` (bit 2)
on a user page and `XD` (bit 63) on a non-executable one, and a DMA domain's `Flags::user_data`
triggers both on every leaf. Every such leaf would be a reserved-bit fault.

VT-d has second-level superpages (bit 7), but whether a unit walks them is `CAP_REG.SLLPS`'s
answer, which the driver does not read. So `Vtd` writes pages only, and bit 7 stays outside the
permitted set. A DMA domain is a few granted pages, so blocks win nothing yet.

`SPLIT_SHIFT` is not a VT-d concept, since IOVAs have no halves. It stays at `Ia32e`'s 47, so
`is_in_half` admits every physical address on a machine with less than 128 TiB of RAM. The address
mask is the CPU's 51:12, not the `CAP_REG.MGAW` width the hardware reports.

## AMD-Vi

Every AMD-Vi bit here is from AMD document 48882 revision 2.62, read for this format. A directory
entry's `NextLevel` field (bits 11:9) names the level of the table it points at, which lets the
hardware skip levels whose address bits are zero. This format never skips. An entry written at walk
level `L` (0 at the top) names AMD level `LEVELS - 1 - L`, so the top table's entries say 3 and the
bottom directory's say 1. That is why `PageFormat::table_entry` takes the level. A leaf says 0,
"the default page size of this level", which at the bottom is 4 KiB.

`IR` (bit 61) and `IW` (bit 62) are combined by AND down the walk and through the device table
entry, so a directory grants both. `FC` (bit 60) makes the IOMMU clear the PCI No Snoop attribute,
so DMA stays coherent with the CPU caches whatever the device asks. Linux sets it on every leaf too
(`drivers/iommu/amd/io_pgtable.c`, read at v6.12). Bit 60 is reserved in a directory entry, so only
leaves carry it.

The device table entry lives in this crate rather than in the kernel's driver. Its claims are bit
positions, so Kani can prove them on any host, where the kernel's x86_64 proofs compile only on an
x86_64 host ([kernel-proofs.md](kernel-proofs.md)).

## Reserved bits are spelled as literals

An encoder builds an entry as `(pa & MASK) | bits`. A check stated through the same constants,
`entry & !(MASK | R | W) == 0`, holds for every value those constants take. Widen the mask over bits
52 to 63 and the encoder and the check move together. The proof stays green while every DMA faults
on a reserved bit.

So every permitted-bit set (`VTD_PERMITTED_BITS` and the AMD-Vi sets) is a literal. The
architecture chooses the bit positions, and the crate spells them rather than citing itself. The
same holds one level down: a harness reads the address field out of the word by hand, since an
encoder and its own decoder agree even when both shift wrong. A harness makes no assumption on
`pa`, because masking it is the encoder's job. A host test feeds an address with bits above 51 set,
the only input that asks the encoder to mask.

## DMA domains

An IOMMU confines a device by translating every address it emits through tables the kernel
programs, in the CPU's own format: SMMUv3 walks the aarch64 format, and the RISC-V IOMMU walks
Sv39. So a domain is a `Mapper` filled with an identity map over the frames the device may reach.
The design is §20 (IOMMU-backed DMA isolation), and the software validator it backs is in
[dma.md](dma.md).

The map is the identity because the userspace virtio driver already puts physical addresses in its
descriptors (its DMA region's frames, and the kernel's shadow page). Behind an IOMMU the device
emits those numbers as IOVAs, and each translates to the same PA. The driver's ABI does not change.
An address it was never granted has no mapping, so the IOMMU faults. Both `virt` boards place RAM
in the low half, at `0x4000_0000` on aarch64 and `0x8000_0000` on riscv.

A single-stage RISC-V IOMMU translation (`iosatp`, no process context) faults on a leaf whose U bit
is clear. So a domain leaf is `Flags::user_data`, or `Flags::user_rodata` for a region the device
may only read. Each region says which, so the virtio shadow page and a firmware region the IVRS
marks read-only are not writable.

`grant_pages` refuses two grants rather than approximate them. An unaligned base is one: rounding
down maps bytes below it, and rounding up shifts the grant. `Mapper::map` would refuse its first
page anyway, but deciding it here makes the refusal provable. A `base + size` that wraps is the
other, since a region with no end has no "inside". That closes a proof obligation rather than a
reachable bug. A wrapped address needs a size within a page of 2^64, and the frame allocator runs
dry after a few thousand pages. What the check buys is a confinement property that holds for every
`DmaRegion` value, so no new caller has to re-derive it.

Soundness rests on `grant_pages` flooring the page count, not on `grant_page`'s whole-page test.
Weakening that test to `off >= r.size` leaves the soundness harness green, and rounding the count
up is what breaks it. The test is defense in depth for a caller passing an index the builder never
would.

### What the proofs cover

Stated over the built tables, "the domain maps exactly the grant" is a build-and-translate round
trip over a symbolic IOVA. That is the bounded model checking over real memory that
[verification.md](verification.md) declined for `Mapper` itself. So the property is decomposed.
The page set the builder feeds the mapper is loopless arithmetic, and Kani proves for every region
and index that it is:

- sound: every enumerated page lies wholly inside the grant, so no ungranted byte is reachable;
- complete: every whole page of the grant is enumerated, so confinement does not starve a driver;
- injective: no page is enumerated twice, so a build cannot fail `AlreadyMapped` against itself;
- total: the arithmetic never panics or wraps.

The proof is format-independent, so it covers the SMMUv3 and RISC-V IOMMU domains at once, which
§19 (architectural parity is a tenet) asks for. The remaining link is that `Mapper::map` writes
exactly one leaf. The proved walk arithmetic and the build-and-translate tests on every format
underwrite it, and dma.md states that residual.

The domain tests choose their own physical addresses in the low half, with host frames behind
`phys_to_ptr`. A host allocator's addresses need not lie in the low half, and on aarch64 Linux they
do not. `FramePool` owns every frame and frees them on drop, so Miri's leak check stays meaningful
([undefined-behavior.md](undefined-behavior.md)).

## The Nth-allocation sweep

Every `OutOfPageFrames` in `Mapper` fires midway through a mapping, after something was taken:
tables for the levels above, or whole leaves before the failing one. `tests/nth_allocation.rs`
reaches each one. It builds milestone 747 (a page-table allocator that fails on its Nth call).

The allocator fails on exactly one call. One failure is stricter than exhaustion, because a mapper
that swallowed the error would succeed on the next call and leave a hole, which exhaustion hides.
After each failure three things hold. No frame leaks, since the mapper owns none and teardown frees
only what the tables reach. What is left mapped is a prefix of the successful run's leaves, which
is what the kernel's `unmap_run_prefix` unwinds. And the error is `OutOfPageFrames`, which the
kernel turns into `OutOfMemory`. Empty intermediate tables stay standing after a failure; they are
reachable, so not a leak.

## T-Head memory types

Before Svpbmt was ratified, T-Head put a memory type in PTE bits 63:59, which standard Sv39
reserves. With `MAEE` set the core reads them on every leaf:

| bit | name | meaning |
|---|---|---|
| 63 | SO | strongly ordered |
| 62 | C | cacheable |
| 61 | B | bufferable |
| 60 | SH | shareable |
| 59 | SEC | trustable (not used here) |

A leaf with all five clear is undefined for normal memory, so plain `Sv39` on that core runs on
behavior nobody promised. `Sv39Mae` is a type rather than a switch inside `Sv39`. The kernel
chooses it per machine at compile time, the way it chooses the early console. A type keeps the
crate free of global state, and the kernel checks at boot that the hart agrees with it.
