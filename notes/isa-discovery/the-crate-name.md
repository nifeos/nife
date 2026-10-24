# Why the crate is `machine_discovery`

*Name: provisional. After the parent's "Naming". calef names files.*

An appendix of [ISA discovery](../isa-discovery.md). The parent keeps the name; this file keeps the
reasons and the prior art behind it.

## The name, and the prior art behind it

calef named the crate `machine_discovery` on 2026-08-23, in a naming review of every crate the
kernel directly depends on. The crate is a boot-time "what machine is this?" record built from
device-tree claims, targeted measurements, and ID-register reads, not just instruction-set-extension
discovery. Verified prior art uses "machine" for this same device-tree-plus-probe scope: Linux ARM's
`setup_machine_fdt()`/`struct machine_desc`/`DT_MACHINE_START` and FreeBSD's `hw.machine`.

## What it is not

- Not `isa`, although `DECISIONS.md` §39 (a component is named for what it is) protects a standard
  term of art a reader already knows from outside this project, like `elf`, `dtb` and `pci`. `isa`
  collides with the well-known ISA bus, a 1980s PC expansion standard, and it describes only part
  of the crate's scope.
- Not `hardware_discovery`: too generic, and it collides with `pci`'s and `dtb`'s own territory.
- Not `identcpu`, FreeBSD's actual name: narrower, register-only, with no device-tree layer.
