# Review package: `extensible_host_controller_interface`

*Name: provisional, minted by lane lane/spell-out-xhci-e1000e. The names section of the
`extensible_host_controller_interface` package of [crate-reviews.md](../crate-reviews.md), in its
own file because the list of public names a later rule reaches is longer than the section.*

## Names

The crate has 446 judged names across 12 files. Its `Name:` blocks all read ratified.

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
| aliases of an old nife spelling | none. Every `doc(alias)` is the specification's spelling. |
| `usb`, `bios`, `id` | accepted words. |

A general gate for undeclared abbreviations: the word list refuses only listed words. Here it
caught one of the private names below, `CONFIG_ERROR_CHANGE`. The others (`IOC`, `ISP`,
`IDT`, `TR`, `PORTSC`, `esit`, `trt`, `ccs`, `erdp`, `hi`, `lo`) were not on the list.

### Private names spelled out

Private names only (`pub(crate)`, module-private, locals and test names). None has a `Name:` block.

| name | refused |
|---|---|
| `INTERRUPT_ON_COMPLETION`, `INTERRUPT_ON_SHORT_PACKET`, `IMMEDIATE_DATA` | `IOC`, `ISP`, `IDT` |
| `SET_TRANSFER_RING_DEQUEUE_POINTER` | `SET_TR_DEQUEUE` |
| `CONFIGURATION_ERROR_CHANGE` | `CONFIG_ERROR_CHANGE` |
| `FIRST_PORT_STATUS_AND_CONTROL_REGISTER`, `INTERRUPTER_0_REGISTER` | `PORTSC_BASE`, `INTERRUPTER_0` |
| locals `maximum_service_interval_payload`, `clamped_interval`, `words`, `transfer_type`, `consumer_cycle_state`, `event_ring_dequeue_pointer`, `high`, `low` | `esit`, `b`, `d`, `trt`, `ccs`, `erdp`, `hi`, `lo` |
| tests `the_port_status_and_control_bits_are_where_the_specification_puts_them`, `a_setup_transfer_request_block_carries_the_packet_and_the_transfer_type` | `the_portsc_bits_are_where_...`, `a_setup_trb_carries_...` |

All seven falsification patches apply and fail their harness.

### The specification's register mnemonics

Spelled out, each with the specification's mnemonic as its `doc(alias)` and its expansion opening
the doc comment. A register offset ends `_REGISTER`, and an address held in a register says it is
physical.

| mnemonic | name |
|---|---|
| `CAPLENGTH` | `CAPABILITY_REGISTERS_LENGTH_BYTES_REGISTER` |
| `HCSPARAMS1`, `HCSPARAMS2` | `STRUCTURAL_PARAMETERS_1_REGISTER`, `STRUCTURAL_PARAMETERS_2_REGISTER` |
| `HCCPARAMS1` | `CAPABILITY_PARAMETERS_1_REGISTER` |
| `DBOFF`, `RTSOFF` | `DOORBELL_OFFSET_REGISTER`, `RUNTIME_REGISTER_SPACE_OFFSET_REGISTER` |
| `USBCMD`, `USBSTS` | `USB_COMMAND_REGISTER`, `USB_STATUS_REGISTER` |
| `PAGESIZE` | `PAGE_SIZE_BITMAP_REGISTER` |
| `CRCR` | `COMMAND_RING_CONTROL_REGISTER` |
| `DCBAAP` | `DEVICE_CONTEXT_BASE_PHYSICAL_ADDRESS_ARRAY_POINTER_REGISTER` |
| `CONFIG` | `CONFIGURATION_REGISTER` |
| `IMAN`, `IMOD` | `INTERRUPTER_MANAGEMENT_REGISTER`, `INTERRUPTER_MODERATION_REGISTER` |
| `ERSTSZ` | `EVENT_RING_SEGMENT_TABLE_SIZE_ENTRIES_REGISTER` |
| `ERSTBA` | `EVENT_RING_SEGMENT_TABLE_BASE_PHYSICAL_ADDRESS_REGISTER` |
| `ERDP` | `EVENT_RING_DEQUEUE_POINTER_REGISTER` |
| `IMAN.IP`, `IMAN.IE`, `ERDP.EHB` | `INTERRUPTER_MANAGEMENT_PENDING`, `INTERRUPTER_MANAGEMENT_ENABLE`, `EVENT_RING_DEQUEUE_POINTER_BUSY` |
| `PORTSC` | `Capabilities::port_status_and_control` |

Three names carry what the size and address rules add: a length or a size carries its unit
(`CAPLENGTH`'s `_BYTES`, `ERSTSZ`'s `_ENTRIES`), and the addresses the device context base address
array holds are physical. The private offsets follow the same rule: `FIRST_PORT_STATUS_AND_CONTROL_REGISTER` and
`INTERRUPTER_0_REGISTER`.

The template's `register_names.py` refuses four names in the crate. All four have a ratified
name, so this note leaves them and asks the architect:

| name | rule it breaks | a name that passes the gate |
|---|---|---|
| `registers::PAGE_SIZE_BITMAP_REGISTER` | `size` carries a unit from `[size_units]`; `bitmap` is not one | `SUPPORTED_PAGE_SIZE_ORDER_REGISTER`: each set bit n is a supported page of order n + 12 |
| `registers::EXTENDED_LEGACY_SUPPORT` | an integer in a register module is an offset, but this is an extended capability id | `EXTENDED_LEGACY_SUPPORT_VALUE` |
| `context::slot_address` | an address is virtual or physical; this is the USB device number in the slot context | a name without `address`, such as `assigned_device_number` |
| `ring::ProducerRing::address_of` | the same | `physical_address_of` |

`address_device` is the xHCI command of that name. The gate reads a leading `address` as a verb
only before `and`, `or`, `the` and similar words, not before `device`, so the function states its
source in the kept form instead: its doc comment opens with `(address_device, xHCI 1.2
specification section 4.6.5)` and carries `#[doc(alias = "Address Device command")]`.

### Provisional names from the gate findings

| old name | new name | reason |
|---|---|---|
| `transfer_request_block_type::ADDRESS_DEVICE` (`pub(crate)`) | `DEVICE_ADDRESSING_COMMAND`, alias `Address Device` | an integer constant is never kept; `address` was neither virtual nor physical |
| `context::endpoint_index` parameter `address` | `endpoint_number_and_direction` | the endpoint address of a descriptor is a number and a direction, not a memory address |
| `TransferRequestBlock::pointer` parameter `address` (private) | `physical_address` | the controller reads the pointer from memory |
| test `the_assigned_address_and_the_superspeed_test` | `the_assigned_device_number_and_the_superspeed_test` | the same rule |
| test `the_next_pointer_is_in_dwords_and_the_limit_is_inclusive` | `the_next_pointer_is_in_double_words_and_the_limit_is_inclusive` | `dwords` is not a word |

The prose acronyms the gate named (`USB`, `DCI`, `IOMMU`, `HC`, `BIOS`, `SMI`, `TR`, and the bare
`IN`, `OUT`, `OFF` and `PORTSC`) now expand at their first use or sit in code.

### Parameter names of public functions

Spelled out. They are provisional: a parameter name is no part of a Rust signature's
compatibility, so the maintainer rules them, pending the architect, and no caller changes.

| parameter | of | now |
|---|---|---|
| `caplength`, `hcsparams1`, `hcsparams2`, `hccparams1`, `dboff`, `rtsoff` | `registers::Capabilities::decode` | `capability_registers_length`, `structural_parameters_1`, `structural_parameters_2`, `capability_parameters_1`, `doorbell_offset`, `runtime_register_space_offset` |
| `n` | `Capabilities::port_status_and_control` | `port_number` |
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

- The single-letter test locals are out of scope.
- Possessives in test names (`qemus_capability_registers_decode`, `the_preserved_set_is_linuxs`,
  `xenons_ports_are_all_counted_once_they_settle`): proper nouns with `s`, not abbreviations.
