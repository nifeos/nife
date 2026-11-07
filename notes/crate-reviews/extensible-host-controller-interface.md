# Review package: `extensible_host_controller_interface`

*Name: provisional, minted by lane lane/spell-out-xhci-e1000e. The names section of the
`extensible_host_controller_interface` package of [crate-reviews.md](../crate-reviews.md), in its
own file because the list of public names a later rule reaches is longer than the section.*

## Names

The crate has 446 judged names across 12 files. Its `Name:` blocks all read ratified, and the
specification's register mnemonics are the public names a later rule reaches.

### Naming rules

The template's own checks (`vendor/crate-workflows/helpers/`, run with `--manifest` over every
`.rs` file under `src/`), then a reading of every name for clipped words the word list does not
hold.

| rule | result |
|---|---|
| spelled-out words (`spelled_out.py`) | 0 findings. `CONFIG_ERROR_CHANGE` is now `CONFIGURATION_ERROR_CHANGE`. |
| units spelled out (`unit_symbols.py`) | 0 findings. `_MILLISECONDS` and `_BYTES` throughout. |
| question shape (`question_shape.py`) | 0 findings. |
| British spelling | 0 findings. |
| aliases of an old nife spelling | none. The crate has no `doc(alias)`. |
| `usb`, `bios`, `id` | accepted words. |

A general gate for undeclared abbreviations: the word list refuses only listed words. Here it
caught one of the private names renamed below, `CONFIG_ERROR_CHANGE`. The others (`IOC`, `ISP`,
`IDT`, `TR`, `PORTSC`, `esit`, `trt`, `ccs`, `erdp`, `hi`, `lo`) were not on the list.

### Renamed by this lane

Private names only (`pub(crate)`, module-private, locals and test names). None has a `Name:` block.

| now | was |
|---|---|
| `INTERRUPT_ON_COMPLETION`, `INTERRUPT_ON_SHORT_PACKET`, `IMMEDIATE_DATA` | `IOC`, `ISP`, `IDT` |
| `SET_TRANSFER_RING_DEQUEUE_POINTER` | `SET_TR_DEQUEUE` |
| `CONFIGURATION_ERROR_CHANGE` | `CONFIG_ERROR_CHANGE` |
| `PORT_STATUS_AND_CONTROL_BASE` | `PORTSC_BASE` |
| locals `maximum_service_interval_payload`, `clamped_interval`, `words`, `transfer_type`, `consumer_cycle_state`, `event_ring_dequeue_pointer`, `high`, `low` | `esit`, `b`, `d`, `trt`, `ccs`, `erdp`, `hi`, `lo` |
| tests `the_port_status_and_control_bits_are_where_the_specification_puts_them`, `a_setup_transfer_request_block_carries_the_packet_and_the_transfer_type` | `the_portsc_bits_are_where_...`, `a_setup_trb_carries_...` |

All seven falsification patches apply and fail their harness.

### Ratified names a later rule reaches

They are not renamed here. The specification's mnemonic is a term of art a reader searches for.
Each recommended name is spelled out and keeps the mnemonic as a `doc(alias)`. The expansion opens
the doc comment, as the rule for a kept foreign name asks. The crate has neither today.

| name | recommended name |
|---|---|
| `registers::CAPLENGTH` | `CAPABILITY_REGISTERS_LENGTH` |
| `registers::HCSPARAMS1`, `HCSPARAMS2` | `STRUCTURAL_PARAMETERS_1`, `STRUCTURAL_PARAMETERS_2` |
| `registers::HCCPARAMS1` | `CAPABILITY_PARAMETERS_1` |
| `registers::DBOFF`, `RTSOFF` | `DOORBELL_OFFSET`, `RUNTIME_REGISTER_SPACE_OFFSET` |
| `registers::USBCMD`, `USBSTS` | `USB_COMMAND`, `USB_STATUS` |
| `registers::PAGESIZE` | `PAGE_SIZE_BITMAP` |
| `registers::CRCR` | `COMMAND_RING_CONTROL` |
| `registers::DCBAAP` | `DEVICE_CONTEXT_BASE_ADDRESS_ARRAY_POINTER` |
| `registers::IMAN`, `IMOD` | `INTERRUPTER_MANAGEMENT`, `INTERRUPTER_MODERATION` |
| `registers::ERSTSZ`, `ERSTBA`, `ERDP` | `EVENT_RING_SEGMENT_TABLE_SIZE`, `EVENT_RING_SEGMENT_TABLE_BASE_ADDRESS`, `EVENT_RING_DEQUEUE_POINTER` |
| `registers::IMAN_PENDING`, `IMAN_ENABLE`, `ERDP_BUSY` | `INTERRUPTER_MANAGEMENT_PENDING`, `INTERRUPTER_MANAGEMENT_ENABLE`, `EVENT_RING_DEQUEUE_POINTER_BUSY` |
| `registers::Capabilities::portsc` | `port_status_and_control` |

### Parameter names of public functions

Spelled out. They are provisional: a parameter name is no part of a Rust signature's
compatibility, so the maintainer rules them, pending the architect, and no caller changes.

| parameter | of | now |
|---|---|---|
| `caplength`, `hcsparams1`, `hcsparams2`, `hccparams1`, `dboff`, `rtsoff` | `registers::Capabilities::decode` | `capability_registers_length`, `structural_parameters_1`, `structural_parameters_2`, `capability_parameters_1`, `doorbell_offset`, `runtime_register_space_offset`, as the constants above would be spelled |
| `n` | `Capabilities::portsc` | `port_number` |
| `reg` | `Capabilities::interrupter` | `register_offset` |
| `dci` | `context::device_offset`, `TransferRequestBlock::reset_endpoint`, `set_dequeue` | `device_context_index` |
| `average_trb` | `context::endpoint` | `average_transfer_request_block_bytes` |
| `s` | `context::is_superspeed` | `speed` |
| `t` | `event::Event::decode` | `transfer_request_block` |
| `b_interval` | `context::interrupt_interval` | `descriptor_interval`; the doc comment says it is the endpoint descriptor's `bInterval` |
| `portsc` | `port::speed_of`, `port::write_value` | `port_status_and_control` |

`TransferRequestBlock::data` and `normal` keep `len`, the standard library's spelling for a length
parameter (`slice::from_raw_parts`).

### Borderline private names, left as they are

- `INTERRUPTER_0`: a digit-suffixed constant that reads as a register-set index, not a clip.
- The single-letter test locals are out of scope.
- Possessives in test names (`qemus_capability_registers_decode`, `the_preserved_set_is_linuxs`,
  `xenons_ports_are_all_counted_once_they_settle`): proper nouns with `s`, not abbreviations.
