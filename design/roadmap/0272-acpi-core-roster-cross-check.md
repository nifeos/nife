---
status: NOT-STARTED
raised: 2026-09-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 272. Give x86_64's core roster the independent re-read the other two architectures already have

Not a design fork. ACPI's MADT is already parsed to build the roster
(`kernel/src/arch/x86_64/machine.rs`); this adds a second, independent walk of the same table to
check the first against, the same shape the other two architectures already have against their
device tree.

## What is skipping

`kernel/src/smp.rs:749` skips its core-roster cross-check on x86_64:

> nothing has read this machine's core roster from a device tree (either `described_count()` is 0,
> or this is x86_64, whose roster is ACPI-based and has no independent device-tree re-read to check
> it against yet)

On aarch64 and riscv64, the roster the scheduler built at boot is checked against a second, later
parse of the device tree (`device_tree_blob::DeviceTreeBlob::from_ptr` on `crate::DTB`), so a bug in the first parse has
something independent to disagree with. `crate::DTB` on x86 holds PVH's `hvm_start_info` pointer,
not an FDT blob, so parsing it as a device tree would not skip, it would panic; the test correctly
avoids that rather than avoiding the check's purpose.

No test has ever independently verified x86_64's core roster is correct. The skip is honest
about that rather than hiding it, which is exactly why it surfaced in the inventory instead of
staying invisible.

## What this needs

1. A second walk of the MADT, independent of `arch::machine`'s own parse (re-reading from the RSDP
   rather than reusing the first parse's result, the same independence the device-tree re-read has).
2. Compare its local-APIC-ID list against `super::described_count()`'s roster.
3. Remove the `cfg!(target_arch = "x86_64")` branch of the skip once the check exists; the
   `described_count() == 0` branch stays, since that half is legitimately architecture-neutral.

## BUGS

- **Not yet scoped how "independent" the second MADT walk needs to be to be worth anything.** Reusing
  the same ACPI table-walking code with a different entry point checks less than the device-tree
  re-read does, since a bug in that shared code would pass both. Worth naming before building rather
  than after.

## Follow-on

- **None.** Self-contained; does not touch milestone 268's boot ladder or DECISIONS §149.

## Index row

calef, 2026-09-10. No test has ever independently verified x86_64's core roster.
