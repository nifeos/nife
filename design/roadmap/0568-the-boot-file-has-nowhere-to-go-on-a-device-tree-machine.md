---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: the-boot-file-has-nowhere-to-go-on-a-device-tree-machine
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 568. The boot file has nowhere to go on a device-tree machine

The number is **provisional**: the integrator mints it at merge. Promoted from the proposal `the-boot-file-has-nowhere-to-go-on-a-device-tree-machine` on 2026-09-22, filed 2026-09-21. Raised by the rung 2a lane of milestone 198 (a package manager, and
the trivial install that makes a second customer possible), which built the install on `x86_64`. It
threaded the gap through the other two architectures as `None` rather than leaving it implied.

The loader half needs no decision. `/chosen` is where a boot loader already tells
this kernel where the archive is, and adding a second pair of properties is the same act twice.

## What is missing

Rung 2a rests on one finding: **the running system does not have its own file.** `uefi_loader`
places the kernel's segments, hands the archive over, and the PE file that contained both is gone.
An installer has to write that file to the new disk. It cannot be carried inside the archive,
because the file contains the archive. The answer is that the loader reads its own file back off the
boot volume while the firmware is still up, and hands it over.

On `x86_64` it hands it over as PVH module 1, beside the archive at module 0, and
`arch::x86_64::machine::boot_file` reads it. On aarch64 and riscv64 the kernel is told about its
machine through a device tree, and `/chosen` carries one initrd (`linux,initrd-start` and
`linux,initrd-end`) with no second slot. So `arch::hand_over` on those two takes the boot file as a
parameter named `_boot_file` that is always `None`, with the reason written at the signature.
`memory::boot_file_region` is dead there.

That is why `kernel/src/user/install_service.rs` is `#[cfg(target_arch = "x86_64")]`, and why rung
2a is an `x86_64` claim rather than the three-architecture one AGENTS.md rule 5 normally asks for.

## What it would take, priced

The loader half is small and the shape already exists.
`uefi_loader/src/device_tree_patch.rs` copies the firmware's tree and writes the two `/chosen`
initrd properties into it. `output_len` already sizes the copy for exactly those additions. Two more
properties (`nife,boot-file-start` and `nife,boot-file-end`, provisional names) are the same code
path with a second pair of values and a larger `output_len`. The reading half is
`kernel/src/memory.rs`'s `init`, which already pulls the initrd bounds out of `/chosen` and stores
them. `record_boot_file` is already written and already portable. Call it a morning, plus the host
tests `device_tree_patch` already has for the initrd pair.

**What it does not buy on its own is an install.** Three other things stand between this and a
riscv64 or aarch64 machine installing itself. They are worth naming here so that nobody promotes
this expecting the rung:

1. A FAT name the writer can produce. `BOOTRISCV64.EFI` is not 8.3, and
   `crates/file_allocation_table` writes it with long-name entries (milestone 560 (a long file name, or riscv64 cannot be installed), built).
2. A disk the firmware will boot from. Rung 2a's second boot rests on a UEFI firmware finding
   `\EFI\BOOT\<file>` on an NVMe disk with no boot variable. radon boots from a card through vendor
   U-Boot, not from an ESP. So "install onto the disk" is a different sentence there, and milestone
   515 (a stick that puts itself on the machine's disk) does not describe it.
3. A runner. There is no aarch64 or riscv64 gate that attaches an empty NVMe controller and an
   ESP at once.

So the honest reading is that this proposal closes a portability gap in the handoff and nothing
more. That is still worth doing on its own terms. An architecture-shaped hole in a boot contract is
the kind of thing that gets forgotten and then surprises somebody. This one is currently held open
by a parameter named `_boot_file`.

## BUGS

- A property name is a thing two programs agree on. `nife,boot-file-start` is provisional and
  the spelling is an architect's, the same way every other name is. A lane should ship one and say
  so rather than wait.
- It is not clear a device-tree machine should carry the file at all. On a board that boots from
  a card there may be no "file the firmware started" in the sense this mechanism means. The
  answer there may be that the installer reads the card rather than that the loader hands the file
  over. That is a fork this proposal does not settle and a promoter should look at first.
- `output_len` is the one place this can go wrong quietly. A tree copied into a buffer sized for
  two properties and then given four overruns, and the failure lands before any console exists on
  some firmware. It has host tests; they would need two more.

## Index row

An installer has to write the running system's own boot file to the new disk. That file cannot be carried inside the archive because it contains the archive. On x86_64 the loader reads its own file back off the boot volume and hands it over as PVH module 1. On aarch64 and riscv64 the kernel learns about its machine from a device tree, and `/chosen` carries one initrd with no second slot. So `hand_over` takes a `_boot_file` that is always `None`. That is why `install_service` is x86_64 only, and closing it is a second pair of `/chosen` properties.
