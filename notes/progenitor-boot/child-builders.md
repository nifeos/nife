# The ELF child builders: an appendix to notes/progenitor-boot.md

*Name: provisional. For the six builders in `fixtures/`. calef names files.*

`user::spawn_elf_child_builder` enters one of the six test builders (`report_child_builder`,
`device_child_builder`, `interrupt_child_builder`, `console_server_builder`,
`least_authority_demo_builder` and `coremark_builder`, all in `fixtures/`). A builder reads its child
out of the archive by name, builds it out of an ELF it parsed, and reports. It is not the boot
path, and every builder gets the same bare world:

| slot | what | rights |
|---|---|---|
| 0 | the building budget | the root's, `GRANT` included |
| 1 | the kernel's `report` endpoint | `WRITE \| GRANT`, to report and to endow a child |
| 2 | the console UART's registers | `WRITE \| GRANT` |
| 3 | the delegated test interrupt (19d.2b) | `READ \| GRANT` |
| 4 | the UART receive interrupt (19d.2c) | `READ \| GRANT` |

`x0` is 0, since no builder dispatches on it. The kernel enters `entry` itself, so `entry` must be
in the trust root, which is why the six are in `sealed_pair::BOOT_PROGRAMS` beside the progenitor.

The building budget is carved before the thread exists, so the caller has a name for it and can
reclaim it through the returned `Holding`: 12,288 pages lent rather than spent
([frames](../frames.md) has the suite's frame ledger).

On `x86_64` slot 2 is a device capability over physical page zero, because `UART_PHYS` is zero
there. It is a foot gun, marked as one where it is granted. The slot is positional, so declining to
grant it would renumber the interrupts after it.
