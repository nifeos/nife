---
status: NOT-STARTED
promoted_from: the-loaders-two-install-agreements-are-code
raised: 2026-10-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 974. The loader's two install agreements are code

Raised by the 2026-10-27 documentation sweep
([its report](../audit-reports/2026-10-27-the-fat-crate-read-from-the-outside.md), finding 7).
Name: provisional, for this file and for every function and constant it sketches.

Reuse: two crates both sides already depend on. `machine_discovery` holds the x86_64 handoff that
`uefi_loader` encodes and the kernel decodes, and `boot_slot` holds the slot states that
`uefi_loader`'s chooser and `system_installer` both read.

## The failure

The install path carries two agreements that only prose states.

### The PVH module order

`uefi_loader` hands the kernel the archive as module 0 and the file it
booted from as module 1. `write_start_info` (`uefi_loader/src/arch/x86_64/mod.rs`) builds the
order as an array, `[module, boot_file]`. The kernel reads it with literal indices,
`nth_module(info, 0)` in `initial_ramdisk` and `nth_module(info, 1)` in `boot_file`
(`kernel/src/arch/x86_64/machine.rs`). Each side's comment says "the order is the contract" and
names the other. A swap on one side hands the boot file to the archive reader, which fails the
measured boot, or hands the archive to an installer, which writes a disk that boots nothing.

### Which partition is boot slot `n`

The chooser writes `boot-slot=<n>` onto the kernel's command line, and `system_installer`'s
`ROLE_CONFIRM` sets the successful bit on slot `n`. Both refuse a table whose used entries are not
contiguous from index 0. Both count `NIFE_BOOT` partitions in table order: the chooser in
`read_table` (`uefi_loader/src/chooser.rs`), capped at `MAX_SLOTS` (4), and `confirm` in its own
loop, capped at `MAX_PARTITIONS` (16). `notes/installing/system-installer.md` calls that counting
"the whole agreement between the two". A change to either loop (skipping an unreadable slot, say)
confirms an image that never ran, which is the outcome that note ranks worse than not confirming.

## The fix

```rust
// machine_discovery::x86_64
/// The PVH module list's order: the archive, then the file the loader booted from.
pub const ARCHIVE_MODULE: usize = 0;
pub const BOOT_FILE_MODULE: usize = 1;
pub const MODULES: usize = 2;
```

`write_start_info` fills `[Option<(u64, u64)>; MODULES]` by those indices, and the kernel calls
`nth_module(info, ARCHIVE_MODULE)` and `nth_module(info, BOOT_FILE_MODULE)`.

```rust
// boot_slot
/// How many boot slots a disk may hold. The chooser counts no further, so neither does anything
/// that names a slot by its number.
pub const MAX_SLOTS: usize = 4;

/// Why a table names no slot `n`.
pub enum NoSlot {
    /// The used entries are not contiguous from index 0, or there are more than the caller can
    /// rewrite (`MAX_PARTITIONS`). A rewrite would renumber them, so both sides refuse.
    NotContiguous,
    /// The table holds `found` boot slots and `n` is not below it.
    Missing { found: usize },
}

/// Boot slot `n` on a disk: the `n`th `NIFE_BOOT` partition in table order, counting at most
/// [`MAX_SLOTS`], with its index in the entry array. The one definition the chooser and the
/// installer both count by.
pub fn nth_slot<'t>(
    partitions: impl Iterator<Item = (usize, &'t Entry)>,
    max_partitions: usize,
    n: usize,
) -> Result<(usize, &'t Entry), NoSlot>;
```

The chooser builds its `Disk` from the same walk, and `confirm` calls
`nth_slot(table.partitions(), MAX_PARTITIONS, slot)` and sends `R_NO_SLOT` with `found` for
either refusal. Today `confirm` keeps counting past four slots where the chooser stops; with one
function both stop at `MAX_SLOTS`. A host test in `boot_slot` pins the order on a table that mixes
data, slot and EFI partitions, and refuses a table with a gap at index 1 and one with five slots.

## Exit test

No literal module index remains in `machine.rs` or `write_start_info`, `confirm` and the chooser
both reach a slot through `boot_slot::nth_slot`, and `cargo xtask install-boot` and `cargo xtask
confirm-boot` pass.

## Index row

The install path carries two agreements that only prose states. `uefi_loader` hands the kernel
the archive as module 0 and the boot file as module 1, while the kernel reads literal indices. Both
sides must also agree on which partition is boot slot `n`. The fix encodes each in a crate both
sides already depend on, `machine_discovery` for the PVH order and `boot_slot` for the slot
states.
