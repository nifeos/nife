# Drivers, boot and install (part 2 of 2)

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  uefi_loader/src/device_tree_from_acpi.rs (RAM cap) | memory runs past `MAX_RAM` are dropped and nothing says so | Report when the ACPI tree truncates RAM runs | S  |
|  xtask/src/confirm.rs | the two disk clients cannot collide only by ordering; a pass shows it held, not that it is enforced | Enforce the confirm-versus-boot disk client ordering | M  |
|  xtask/src/uefi.rs | cannot install to the machine's own ESP with a boot entry | Install nife to a machine's own ESP with a boot entry | M  |
