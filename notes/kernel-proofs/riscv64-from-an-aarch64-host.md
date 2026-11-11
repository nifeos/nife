# Proving riscv64's IOMMU driver from an aarch64 host

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/kernel-proofs.md](../kernel-proofs.md), which links here from its stub-list
item 8. It is milestone 432 (the RISC-V IOMMU driver has no counterpart to the SMMU's proofs), built
as option 1 of
[the proposal that measured it](../../design/roadmap/0650-riscv64-code-the-prover-can-already-compile.md).

## A correction first

`kernel-proofs.md` said riscv64 was unreachable, and that `iommu.rs`'s own property "can be written
and cannot be run". That was true of the `cfg` dispatch in `arch/mod.rs`. It was not true of the
files the dispatch hides. A riscv64 file with no `asm!` and no riscv64-only sibling compiles
unchanged on an aarch64 host. The `lane/price-kani-kernel-reach` lane measured this on 2026-09-24.

Native riscv64 proof is still out of reach. Stock Kani panics on a riscv64 host, and nothing here
runs one.

## How the file reaches the prover

`kernel/src/arch/mod.rs` holds `mod riscv64 { mod iommu; }` under
`cfg(all(kani, target_arch = "aarch64"))`. No `#[path]` is needed, because an inline module in a
`mod.rs` resolves `mod iommu;` to `arch/riscv64/iommu.rs` on its own. The harness paths are the
native ones, `arch::riscv64::iommu::proofs::...`, so `script/falsifications` derives their patch
names with no special case.

It is aarch64 only on purpose. Exactly one verify host should run it, and whether x86_64 resolves the
same names was not measured.

## The caveat: crate::arch is the host's

Inside that module, `crate::arch` means aarch64. So `iommu.rs`'s call to
`crate::arch::mmu::phys_to_virt` silently resolves to aarch64's function. A harness that reached it
would prove aarch64's code under riscv64's name. That is the stub that reads as coverage, which the
parent note exists to prevent.

Only code that never calls through `crate::arch` is proved here. Today both harnesses call four pure
functions and nothing else: `device_context`, `is_in_directory`, `context_offset` and
`iodir_inval_ddt`.

`script/lint` check 5b holds the line. It records each `crate::arch` path a file in the module names,
with why no harness reaches it, and fails on a new one or a new file. It is a grep, not a call graph,
so it cannot prove reachability. It forces the question at the moment it becomes live.

Extending the module to every asm-free riscv64 file was refused, in the proposal's option 2.
`mmu.rs` and `timer.rs` call `crate::arch` throughout, so their proofs would be fictions.

## The two properties

The SMMU's first property does not transfer. This driver writes whole 64-bit words, so there is no
split for an address to lose half of. Milestone 432's block said finding the right property was most
of the work.

`the_iommu_is_handed_exactly_the_domain_the_kernel_built`:

- `iosatp`'s PPN, read back at bits [43:0], is the table root entire, for every page-frame root below
  2^56.
- The reserved bits above it are zero.
- `ta.PSCID` is the domain's tag and nothing else.
- The valid bit is set, fault reporting is on, and the MSI words are zero.
- For every root, with no assumption at all, MODE is Sv39. MODE 0 is Bare, which turns translation
  off.

`no_device_can_reach_another_devices_context`:

- It covers both context formats: the 64-byte one QEMU selects and the 32-byte one that has never run.
- Every requester id `is_in_directory` admits has its whole context inside the directory's frame.
- Each context is 8-byte aligned, and no two ids share a byte.
- The `IODIR.INVAL_DDT` that follows the write names exactly that device, with DV set.

## Falsified before believed

Both patches are in `kernel/falsifications/` and replay on an aarch64 host. Each was confirmed red on
2026-09-25.

One is the SMMU's `STRTAB_LOG2` shape again. A bound written for the 32-byte format admits ids 64 to
127, past the frame, and every test stays green.

The other gives `iosatp` the `(pa >> 12) << 10` encoding that three neighbors in the same file use.
It turns MODE red for a high enough root. The boot confinement test would probably catch it too; that
was reasoned, not run.

## Cost

On patagonia the new harnesses verify in 2.6 and 4.2 seconds. The whole `cargo kani -p kernel` run
stays near 400 MB peak resident.

## BUGS

- Everything that touches the hardware is unreached: `init`, `cmd_push`, `take_fault`, and the write
  order in `attach` (valid bit last).
- The register offsets and bit constants cannot be checked against the RISC-V IOMMU specification by
  anything here. The boot-time confinement test in `kernel/src/virtio.rs` still stands against that.
- About 91% of `arch/riscv64/` is still reached by nothing, and so are the riscv64-only `cfg` sites
  outside `arch/`. `iommu.rs` is 564 of its 6,083 Rust lines (`wc -l`, 2026-09-25). This said 94%,
  from a total that counted every file twice.
