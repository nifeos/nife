# The names in `extensible_host_controller_interface`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 820 (proven xHCI logic, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-25 (UTC). `python3 helpers/name_release.py --draft extensible_host_controller_interface` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

158 names: 143 no block, 14 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 7 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 25 rows say something other than `clear` or `term`. This file renames nothing.

- `clear`: a stranger reads it right.
- `term`: the specification or databook owns it, and it keeps that spelling, as design/naming.md asks of a name across somebody else's interface.
- `nife`: it names nife's own process plan (address-space windows, the handoff a server receives). A stranger has no such plan, and the crate may shed the name before it leaves.
- `scope`: it belongs to something other than the crate's subject.
- `generic`: a word that could name almost anything, design/naming.md's second failure mode.
- `unclear`: what a stranger would trip on, said in the cell.

| name | kind | path | status | outside | a stranger reads it |
|---|---|---|---|---|---|
| `extensible_host_controller_interface` | crate | `src/lib.rs` | ratified 2026-10-07. | yes | clear |
| `completion` | module | `src/completion.rs` | unrecorded. | yes | clear |
| `context` | module | `src/context.rs` | unrecorded. | yes | clear |
| `dma` | module | `src/dma.rs` | unrecorded. | yes | clear: but `DMA` is a phrase people say (the acronym test) |
| `event` | module | `src/event.rs` | provisional. | no | clear |
| `handoff` | module | `src/handoff.rs` | provisional. | no | nife: nife's own process plan |
| `port` | module | `src/port.rs` | unrecorded. | yes | clear |
| `regs` | module | `src/regs.rs` | unrecorded. | yes | clear |
| `report` | module | `src/report.rs` | unrecorded. | yes | scope: a keyboard |
| `ring` | module | `src/ring.rs` | provisional. | no | clear |
| `tests` | module | `src/tests.rs` | unrecorded. | no | clear; private |
| `trb` | module | `src/trb.rs` | provisional. | no | clear |
| `trb_type` | module | `src/trb_type.rs` | unrecorded. | yes | term |
| `verification` | module | `src/verification.rs` | unrecorded. | no | clear; private |
| `window` | module | `src/window.rs` | provisional. | no | nife: nife's own process plan |
| `NO_SLOTS` | item | `src/completion.rs` | unrecorded. | yes | clear |
| `SHORT_PACKET` | item | `src/completion.rs` | unrecorded. | yes | clear |
| `STALL` | item | `src/completion.rs` | unrecorded. | yes | clear |
| `SUCCESS` | item | `src/completion.rs` | unrecorded. | yes | clear |
| `TRANSACTION_ERROR` | item | `src/completion.rs` | unrecorded. | yes | clear |
| `CONTROL` | item | `src/context.rs` | unrecorded. | yes | clear |
| `INTERRUPT_IN` | item | `src/context.rs` | unrecorded. | yes | clear |
| `control_endpoint` | item | `src/context.rs` | unrecorded. | yes | clear |
| `device_offset` | item | `src/context.rs` | unrecorded. | yes | clear |
| `endpoint` | item | `src/context.rs` | unrecorded. | yes | clear |
| `endpoint_index` | item | `src/context.rs` | unrecorded. | yes | clear |
| `input_control` | item | `src/context.rs` | unrecorded. | yes | clear |
| `input_offset` | item | `src/context.rs` | unrecorded. | yes | clear |
| `interrupt_interval` | item | `src/context.rs` | unrecorded. | yes | clear |
| `is_superspeed` | item | `src/context.rs` | unrecorded. | yes | clear |
| `slot` | item | `src/context.rs` | unrecorded. | yes | clear |
| `slot_address` | item | `src/context.rs` | unrecorded. | yes | clear |
| `COMMAND_RING` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `CONTROL_BUFFER` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `CONTROL_RING` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `DEVICE_CONTEXT` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `DEVICE_CONTEXT_ARRAY` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `EVENT_RING` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `EVENT_SEGMENT_TABLE` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `INPUT_CONTEXT` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `INTERRUPT_RING` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `MAX_SCRATCHPADS` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `REPORTS_QUEUED` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `REPORT_BUFFERS` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `REPORT_SLOT` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `RING_ENTRIES` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `SCRATCHPAD_ARRAY` | item | `src/dma.rs` | unrecorded. | yes | clear |
| `WORK_PAGES` | item | `src/dma.rs` | unrecorded. | yes | nife: nife's own process plan |
| `region_pages` | item | `src/dma.rs` | unrecorded. | yes | nife: nife's own process plan |
| `Event` | item | `src/event.rs` | unrecorded. | yes | clear |
| `decode` | item | `src/event.rs` | unrecorded. | yes | clear |
| `Handoff` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `pack` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `unpack` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `MAX_REGISTER_PAGES` | item | `src/lib.rs` | unrecorded. | yes | nife: nife's own process plan |
| `PAGE` | item | `src/lib.rs` | unrecorded. | yes | generic |
| `CHANGES` | item | `src/port.rs` | unrecorded. | yes | clear |
| `CONNECTED` | item | `src/port.rs` | unrecorded. | yes | clear |
| `CONNECT_CHANGE` | item | `src/port.rs` | unrecorded. | yes | clear |
| `Connections` | item | `src/port.rs` | provisional. | yes | clear |
| `DEBOUNCE_STABLE_MS` | item | `src/port.rs` | provisional. | yes | clear |
| `DEBOUNCE_STEP_MS` | item | `src/port.rs` | provisional. | yes | clear |
| `DEBOUNCE_TIMEOUT_MS` | item | `src/port.rs` | provisional. | yes | clear |
| `ENABLED` | item | `src/port.rs` | unrecorded. | yes | clear |
| `FULL` | item | `src/port.rs` | unrecorded. | yes | unclear alone: full speed, in the port module |
| `POWER` | item | `src/port.rs` | unrecorded. | yes | clear |
| `RESET` | item | `src/port.rs` | unrecorded. | yes | clear |
| `RESET_CHANGE` | item | `src/port.rs` | unrecorded. | yes | clear |
| `Settle` | item | `src/port.rs` | provisional. | yes | unclear alone: what settles (a port's debounce) is not in the word |
| `default_control_packet` | item | `src/port.rs` | unrecorded. | yes | clear |
| `new` | item | `src/port.rs` | provisional. | yes | clear |
| `observe` | item | `src/port.rs` | provisional. | yes | generic |
| `speed_name` | item | `src/port.rs` | unrecorded. | yes | clear |
| `speed_of` | item | `src/port.rs` | unrecorded. | yes | clear |
| `write_value` | item | `src/port.rs` | unrecorded. | yes | clear |
| `CAPLENGTH` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CMD_INTERRUPTS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CMD_RESET` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CMD_RUN` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CONFIG` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CRCR` | item | `src/regs.rs` | unrecorded. | yes | term |
| `Capabilities` | item | `src/regs.rs` | unrecorded. | yes | clear |
| `DBOFF` | item | `src/regs.rs` | unrecorded. | yes | term |
| `DCBAAP` | item | `src/regs.rs` | unrecorded. | yes | term |
| `ERDP` | item | `src/regs.rs` | unrecorded. | yes | term |
| `ERDP_BUSY` | item | `src/regs.rs` | unrecorded. | yes | term |
| `ERSTBA` | item | `src/regs.rs` | unrecorded. | yes | term |
| `ERSTSZ` | item | `src/regs.rs` | unrecorded. | yes | term |
| `EXTENDED_LEGACY_SUPPORT` | item | `src/regs.rs` | unrecorded. | yes | term |
| `HCCPARAMS1` | item | `src/regs.rs` | unrecorded. | yes | term |
| `HCSPARAMS1` | item | `src/regs.rs` | unrecorded. | yes | term |
| `HCSPARAMS2` | item | `src/regs.rs` | unrecorded. | yes | term |
| `IMAN` | item | `src/regs.rs` | unrecorded. | yes | term |
| `IMAN_ENABLE` | item | `src/regs.rs` | unrecorded. | yes | term |
| `IMAN_PENDING` | item | `src/regs.rs` | unrecorded. | yes | term |
| `IMOD` | item | `src/regs.rs` | unrecorded. | yes | term |
| `LEGACY_BIOS_OWNED` | item | `src/regs.rs` | unrecorded. | yes | term |
| `LEGACY_OS_OWNED` | item | `src/regs.rs` | unrecorded. | yes | term |
| `LEGACY_SMI_ENABLES` | item | `src/regs.rs` | unrecorded. | yes | term |
| `LEGACY_SMI_EVENTS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `PAGESIZE` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RTSOFF` | item | `src/regs.rs` | unrecorded. | yes | term |
| `STS_CONTROLLER_ERROR` | item | `src/regs.rs` | unrecorded. | yes | term |
| `STS_EVENT_INTERRUPT` | item | `src/regs.rs` | unrecorded. | yes | term |
| `STS_HALTED` | item | `src/regs.rs` | unrecorded. | yes | term |
| `STS_HOST_SYSTEM_ERROR` | item | `src/regs.rs` | unrecorded. | yes | term |
| `STS_NOT_READY` | item | `src/regs.rs` | unrecorded. | yes | term |
| `STS_PORT_CHANGE` | item | `src/regs.rs` | unrecorded. | yes | term |
| `USBCMD` | item | `src/regs.rs` | unrecorded. | yes | term |
| `USBSTS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `decode` | item | `src/regs.rs` | unrecorded. | yes | clear |
| `doorbell` | item | `src/regs.rs` | unrecorded. | yes | clear |
| `find_extended` | item | `src/regs.rs` | unrecorded. | yes | clear |
| `interrupter` | item | `src/regs.rs` | unrecorded. | yes | clear |
| `portsc` | item | `src/regs.rs` | unrecorded. | yes | clear |
| `FAILED` | item | `src/report.rs` | unrecorded. | yes | generic |
| `KEYBOARD` | item | `src/report.rs` | unrecorded. | yes | scope: a keyboard |
| `NO_KEYBOARD` | item | `src/report.rs` | unrecorded. | yes | scope: a keyboard |
| `Step` | item | `src/report.rs` | unrecorded. | yes | generic |
| `describe` | item | `src/report.rs` | unrecorded. | yes | generic |
| `from_code` | item | `src/report.rs` | unrecorded. | yes | clear |
| `keyboard` | item | `src/report.rs` | unrecorded. | yes | scope: a keyboard |
| `keyboard_of` | item | `src/report.rs` | unrecorded. | yes | scope: a keyboard |
| `ports` | item | `src/report.rs` | provisional. | yes | clear |
| `ports_of` | item | `src/report.rs` | provisional. | yes | clear |
| `refusal` | item | `src/report.rs` | unrecorded. | yes | generic |
| `refusal_of` | item | `src/report.rs` | unrecorded. | yes | generic |
| `EventRing` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `Placement` | item | `src/ring.rs` | unrecorded. | yes | generic |
| `ProducerRing` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `address_of` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `advance` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `dequeue_pointer` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `enqueue` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `index` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `is_ready` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `new` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `segment_table_entry` | item | `src/ring.rs` | unrecorded. | yes | clear |
| `Trb` | item | `src/trb.rs` | unrecorded. | yes | term |
| `address_device` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `configure_endpoint` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `cycle` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `data` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `disable_slot` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `enable_slot` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `evaluate_context` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `kind` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `link` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `normal` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `reset_endpoint` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `set_dequeue` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `setup` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `status` | item | `src/trb.rs` | unrecorded. | yes | clear |
| `DATA` | item | `src/trb_type.rs` | unrecorded. | yes | clear |
| `NORMAL` | item | `src/trb_type.rs` | unrecorded. | yes | clear |
| `STATUS` | item | `src/trb_type.rs` | unrecorded. | yes | clear |
| `WindowRefusal` | item | `src/window.rs` | unrecorded. | yes | nife: nife's own process plan |
| `register_window` | item | `src/window.rs` | unrecorded. | yes | nife: nife's own process plan |

Inline modules a stranger also meets, which `script/names` does not count as names of their own: `speed`.
