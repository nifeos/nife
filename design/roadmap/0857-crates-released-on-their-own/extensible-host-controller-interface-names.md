# The names in `extensible_host_controller_interface`

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md),
for the crate of milestone 820 (proven xHCI logic, released on its own). This file is the record of
every name in the crate, which `helpers/name_release.py` reads in place of `Name:` blocks: the
crate's shipped files carry none.

Ratified: 2026-10-25 (#2720).

The ruling of that date ratified every name below; register names keep the specification's
spelling. Each row with a refusal names the spelling the ruling turned down and why. The same ruling
moved nife's process plan out of the crate. `Handoff`, `register_window`, `WindowRefusal`,
`MAX_REGISTER_PAGES`, `WORK_PAGES`, `region_pages` and the driver's bring-up report sit in the
module `extensible_host_controller_interface` of the nife-internal crate `driver_process_contract`,
which does not publish. Their names follow nife's ordinary naming, with no row here. A new name in this crate takes a row here, `provisional` until a ruling, and `script/lint`
fails a module or `pub` item without one.

The column `outside` says whether a caller outside the crate reaches the name. Some names have no
row because `script/names` does not count them: enum variants, struct fields and trait members. The
same ruling covers them. Ruled 2026-10-26 (UTC), on #2760: the TRB field of `Event::Transfer`,
`Event::Command` and `TransferRequestBlockPlacement` is `transfer_request_block`, the module's own
spelling, and the `USBCMD`, `USBSTS` and `USBLEGCTLSTS` bits spell out what this tree abbreviated.

| name | kind | path | status | outside |
|---|---|---|---|---|
| `extensible_host_controller_interface` | crate | `src/lib.rs` | ratified 2026-10-07 (#1806). xHCI expanded, as the acronym test asks; permanent once published. Too long for a program name, so the program is `usb_keyboard_driver`. | yes |
| `completion` | module | `src/completion.rs` | ratified 2026-10-25 (#2720). | yes |
| `context` | module | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `direct_memory_access` | module | `src/direct_memory_access.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `dma` (the acronym test spells it out, as §154 records). | yes |
| `event` | module | `src/event.rs` | ratified 2026-10-25 (#2720). | no |
| `port` | module | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `registers` | module | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `regs` (an abbreviation; the constants inside keep the specification's names). | yes |
| `ring` | module | `src/ring.rs` | ratified 2026-10-25 (#2720). | no |
| `tests` | module | `src/tests.rs` | ratified 2026-10-25 (#2720). | no |
| `transfer_request_block` | module | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). Refused `trb` (spelled out, as the acronym test asks). | no |
| `transfer_request_block_type` | module | `src/transfer_request_block_type.rs` | ratified 2026-10-25 (#2720). Refused `trb_type` (spelled out, as the acronym test asks). | yes |
| `verification` | module | `src/verification.rs` | ratified 2026-10-25 (#2720). | no |
| `NO_SLOTS` | item | `src/completion.rs` | ratified 2026-10-25 (#2720). | no |
| `SHORT_PACKET` | item | `src/completion.rs` | ratified 2026-10-25 (#2720). | yes |
| `STALL` | item | `src/completion.rs` | ratified 2026-10-25 (#2720). | no |
| `SUCCESS` | item | `src/completion.rs` | ratified 2026-10-25 (#2720). | yes |
| `TRANSACTION_ERROR` | item | `src/completion.rs` | ratified 2026-10-25 (#2720). | no |
| `CONTROL` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | no |
| `INTERRUPT_IN` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `control_endpoint` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `device_offset` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | no |
| `endpoint` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `endpoint_index` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `input_control` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `input_offset` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `interrupt_interval` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `is_superspeed` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `slot` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | yes |
| `slot_address` | item | `src/context.rs` | ratified 2026-10-25 (#2720). | no |
| `COMMAND_RING` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `CONTROL_BUFFER` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `CONTROL_RING` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `DEVICE_CONTEXT` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `DEVICE_CONTEXT_ARRAY` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `EVENT_RING` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `EVENT_SEGMENT_TABLE` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `INPUT_CONTEXT` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `INTERRUPT_RING` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `MAX_SCRATCHPADS` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `REPORTS_QUEUED` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `REPORT_BUFFERS` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `REPORT_SLOT` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `RING_ENTRIES` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `SCRATCHPAD_ARRAY` | item | `src/direct_memory_access.rs` | ratified 2026-10-25 (#2720). | yes |
| `Event` | item | `src/event.rs` | ratified 2026-10-25 (#2720). | yes |
| `decode` | item | `src/event.rs` | ratified 2026-10-25 (#2720). | yes |
| `PAGE_BYTES` | item | `src/lib.rs` | ratified 2026-10-25 (#2720). Refused `PAGE` (it reads as a page and holds a size). | yes |
| `CHANGES` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `CONNECTED` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `CONNECT_CHANGE` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `Connections` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `DEBOUNCE_STABLE_MS` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `DEBOUNCE_STEP_MS` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `DEBOUNCE_TIMEOUT_MS` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | no |
| `ENABLED` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `FULL` | item | `src/port.rs` | ratified 2026-10-25 (#2720). Kept: `speed::FULL` reads as the USB full-speed id it is. | no |
| `POWER` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `PortDebounce` | item | `src/port.rs` | ratified 2026-10-25 (#2720). Refused `Settle` (the debounce is what it does; `Settle` named the outcome). | yes |
| `RESET` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `RESET_CHANGE` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `default_control_packet` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `new` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `sample` | item | `src/port.rs` | ratified 2026-10-25 (#2720). Refused `observe` (each call is one sample of the ports' connections). | yes |
| `speed_name` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `speed_of` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `write_value` | item | `src/port.rs` | ratified 2026-10-25 (#2720). | yes |
| `CAPLENGTH` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `COMMAND_INTERRUPTS` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `CMD_INTERRUPTS` (spelled out; the register keeps `USBCMD`). | yes |
| `COMMAND_RESET` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `CMD_RESET` (spelled out; the register keeps `USBCMD`). | yes |
| `COMMAND_RUN` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `CMD_RUN` (spelled out; the register keeps `USBCMD`). | yes |
| `CONFIG` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `CRCR` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `Capabilities` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `DBOFF` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `DCBAAP` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `ERDP` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `ERDP_BUSY` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `ERSTBA` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `ERSTSZ` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `EXTENDED_LEGACY_SUPPORT` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `HCCPARAMS1` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `HCSPARAMS1` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `HCSPARAMS2` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `IMAN` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `IMAN_ENABLE` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `IMAN_PENDING` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `IMOD` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | no |
| `LEGACY_BIOS_OWNED` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `LEGACY_OS_OWNED` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | no |
| `LEGACY_SYSTEM_MANAGEMENT_INTERRUPT_ENABLES` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `LEGACY_SMI_ENABLES` (`smi` spelled out, as the acronym test asks). | yes |
| `LEGACY_SYSTEM_MANAGEMENT_INTERRUPT_EVENTS` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `LEGACY_SMI_EVENTS` (`smi` spelled out, as the acronym test asks). | yes |
| `PAGESIZE` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `RTSOFF` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `STATUS_CONTROLLER_ERROR` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `STS_CONTROLLER_ERROR` (spelled out; the register keeps `USBSTS`). | no |
| `STATUS_EVENT_INTERRUPT` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `STS_EVENT_INTERRUPT` (spelled out; the register keeps `USBSTS`). | yes |
| `STATUS_HALTED` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `STS_HALTED` (spelled out; the register keeps `USBSTS`). | yes |
| `STATUS_HOST_SYSTEM_ERROR` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `STS_HOST_SYSTEM_ERROR` (spelled out; the register keeps `USBSTS`). | no |
| `STATUS_NOT_READY` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `STS_NOT_READY` (spelled out; the register keeps `USBSTS`). | yes |
| `STATUS_PORT_CHANGE` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), on #2760. Refused `STS_PORT_CHANGE` (spelled out; the register keeps `USBSTS`). | yes |
| `USBCMD` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `USBSTS` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `decode` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `doorbell` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `find_extended` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `interrupter` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `portsc` | item | `src/registers.rs` | ratified 2026-10-25 (#2720). | yes |
| `EventRing` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `ProducerRing` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `TransferRequestBlockPlacement` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). Refused `Placement` (it did not say what is placed). | yes |
| `address_of` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `advance` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `dequeue_pointer` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `enqueue` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `index` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `is_ready` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `new` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `segment_table_entry` | item | `src/ring.rs` | ratified 2026-10-25 (#2720). | yes |
| `TransferRequestBlock` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). Refused `Trb` (spelled out, as the acronym test asks). | yes |
| `address_device` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `configure_endpoint` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `cycle` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `data` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `disable_slot` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `enable_slot` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `evaluate_context` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `kind` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `link` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `normal` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `reset_endpoint` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `set_dequeue` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `setup` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `status` | item | `src/transfer_request_block.rs` | ratified 2026-10-25 (#2720). | yes |
| `DATA` | item | `src/transfer_request_block_type.rs` | ratified 2026-10-25 (#2720). | no |
| `NORMAL` | item | `src/transfer_request_block_type.rs` | ratified 2026-10-25 (#2720). | no |
| `STATUS` | item | `src/transfer_request_block_type.rs` | ratified 2026-10-25 (#2720). | no |
