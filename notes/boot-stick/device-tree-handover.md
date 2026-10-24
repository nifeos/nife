# How the device tree reaches the kernel on aarch64 and riscv64, measured

*Name: provisional, for the handover the appendix measures. calef names files.*

An appendix to [`notes/boot-stick.md`](../boot-stick.md). It holds the four steps the loader takes
to hand the kernel a device tree, and the transcripts under EDK2. It also holds the ACPI case: how
EDK2 on aarch64 hid the tree, and how the loader now writes one from the ACPI tables.

## The four steps

1. The loader finds the tree in the UEFI configuration table under `gFdtTableGuid`
   (`b1b621d5-f19c-41a5-830b-d9152c69aae0`, read from EDK2's `MdePkg.dec`).
2. It copies the tree below what the kernel's boot page tables reach (2 GiB on aarch64, 3 GiB on
   riscv64), adding `/chosen/linux,initrd-start` and `linux,initrd-end` for the archive it placed.
   That is `uefi_loader::device_tree_patch`, whose tests decode its output with the crate the kernel
   decodes with, so the two cannot disagree without a host test failing.
3. On riscv64 it also asks which hart it is, through `RISCV_EFI_BOOT_PROTOCOL` (GUID read from
   EDK2's `UefiCpuPkg.dec`), falling back to `/chosen/boot-hartid`, and prints which answered.
4. It exits boot services and enters the kernel with the tree's address in a register. On aarch64
   that is `x0`, after cleaning the data cache to coherency and turning the MMU off. On riscv64 it
   is `a1`, after turning paging off.

What the kernels printed under EDK2, 2026-09-19:

```text
aarch64:  firmware handoff: 0x000000004cb41000  (device tree)
          initrd          : 10415616 bytes at 0x000000004ae3a000, a nifefs archive of 88 program(s)
          smp: 4 core(s) online
riscv64:  uefi_loader: boot hart from RISCV_EFI_BOOT_PROTOCOL: 0
          firmware handoff: 0x000000008dbad000  (device tree)
          initrd          : 7476736 bytes at 0x000000008dbaf000, a nifefs archive of 88 program(s)
          smp: 4 core(s) online                     (with NIFE_SMP=4)
```

## EDK2 on aarch64 hid the device tree when it presented ACPI

Measured 2026-09-19. Under `-machine virt`, where ACPI is on by default, the loader printed `the
firmware offers no device tree (on QEMU virt, boot with acpi=off; ...)` and stopped. Under
`acpi=off` it booted. That was recorded here as a kernel limit the loader reported rather than one
it added. It was the reason `helpers/qemu-stick.sh` passed `acpi=off`.

It was fixed on 2026-09-23. The loader now reads the tables and writes the device tree the kernel
would have been handed (`uefi_loader/src/device_tree_from_acpi.rs`). So `NIFE_ACPI=on
helpers/qemu-stick.sh aarch64 target/stick` reaches the same shell prompt `acpi=off` does. The
firmware's own tree still wins when a machine offers both, because it is the richer description.
What the two boots differ by, measured the same day on the same stick:

```text
                  acpi=off (the firmware's tree)         acpi=on (written from ACPI)
  pcie          : ecam 0x4010000000..0x4020000000        none (this machine describes no host bridge)
  memory        : 256 MiB total, 234 MiB free            256 MiB total, 228 MiB free
```

Everything else in the machine description is byte-identical: the same cpu, console, GIC, timer,
MMU and scheduler lines. The PCI bus is missing because an ACPI machine states its BAR windows and
its interrupt routing in AML (`_CRS` and `_PRT`), which needs an interpreter. The six megabytes are
the ACPI tables and firmware runtime regions. They are excluded from the RAM this writes rather than
handed to the frame allocator. `device_tree_from_acpi`'s own BUGS section is the full list.

Why this matters beyond QEMU: SBBR requires an aarch64 server to present ACPI and permits it to
present no device tree. So every aarch64 cloud machine (AWS Graviton, Azure Cobalt, Google Axion,
Oracle Ampere A1) is the `acpi=on` case. `notes/rented-metal.md` priced them and found none could
run nife; this is the half of that which was ours to fix.
