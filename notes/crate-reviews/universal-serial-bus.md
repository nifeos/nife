# Review package: `universal_serial_bus`

*Name: provisional, minted by lane lane/1111-outside-review-fixes. The `usb` package of the index
[crate-reviews.md](../crate-reviews.md), journey 5 step 3, under the name the crate will publish as.*

## The package

Name and version. `usb` `0.1.0` in this tree, published as `universal_serial_bus` because the name
`usb` is taken on crates.io (milestone 857, crates released on their own). The package and the
directory take the long name at the move, which is item 2 of
[milestone 1111 (what an outside maintainer would fix before the first release)](../../design/roadmap/1111-usb-outside-review.md).
Nine `Name:` blocks: the crate's own and eight public items, all provisional.

API. The host side of a USB boot keyboard, as pure computation: what a device says about itself,
what a host asks it, and what a boot report means. It touches nothing.

| module | what it holds |
|---|---|
| `descriptor` | the four descriptor types, the four descriptor sizes, the HID class triple, `DeviceDescriptor::parse`, `ConfigurationHeader::parse`, `Speed`, `control_maximum_packet_bytes`, `find_boot_keyboard` and its `BootKeyboard`, `Refusal` |
| `request` | `SetupPacket` and its four constructors (`get_descriptor`, `set_configuration`, `set_boot_protocol`, `set_idle_forever`), `is_device_to_host`, `to_u64`, and the eight request constants |
| `boot_keyboard` | `BootReport::parse`, `BootReport::RELEASED`, `REPORT_BYTES`, `ERROR_ROLL_OVER`, `MODIFIER_CODES`, `evdev_code`, `changes` |

No public signature names another crate's type, and no nife type appears in one.

What a stranger may find surprising:

- `changes` speaks Linux evdev codes, not HID usages. Milestone 1111's Decision 2 decides,
  provisionally, that it emits HID usages and the caller maps them through `evdev_code`; the crate
  does not build that yet.
- `DeviceDescriptor::parse` answers `Option`, where `ConfigurationHeader::parse` answers
  `Result<_, Refusal>`. Decision 3 decides, provisionally, that it answers `Result` too; the crate
  does not build that yet. `BootReport::parse` keeps `Option` by the same decision.
- No enum is `#[non_exhaustive]`, so a variant added to `Refusal` or `Speed` after the first
  release breaks a caller's `match`.

Proofs. Five Kani harnesses, each with a falsification patch:

| harness | bound | what it proves |
|---|---|---|
| `the_header_parsers_are_total` | every byte string of up to 20 bytes | the device descriptor and configuration header parsers never panic |
| `the_configuration_walk_is_total_and_accepts_only_an_interrupt_in` | every configuration of up to 25 bytes | the walk never panics, and what it accepts is an interrupt IN endpoint numbered above zero |
| `two_interfaces_are_walked_as_two` | 41 bytes in one fixed shape, cut at every length | across two interfaces the walk answers only a boot keyboard's endpoint, and a refusal names a boot interface the buffer holds |
| `two_reports_produce_a_bounded_number_of_events` | every pair of eight-byte reports | at most twenty events |
| `two_reports_produce_their_events_in_order` | every pair of eight-byte reports | modifiers first in bit order, then releases, then presses, each the code of a usage held in exactly one report |

Outside the bounds: a configuration longer than 25 bytes in any shape but the fixed one, which is
what a real composite keyboard sends (the unit tests walk several, and item 10's fuzz target is the
check that reaches them), and the converse of the report proof, that every key which changed
produces an event. The two-interface harness takes 14 s and the event-order harness 238 s, beside
177 s for the 25-byte walk.

Dependencies. None. The one consumer in nife is `components` (`usb_keyboard_driver.rs`), by path.

Naming rules. The template's checks, run with `--manifest crates/usb/Cargo.toml` over every
tracked `.rs` file of the crate.

| rule | result |
|---|---|
| spelled-out words (`spelled_out.py`) | 0 of 239 names refused |
| register and size names (`register_names.py`) | 0 refused. `total_length` and `SetupPacket::length` keep the specification's word in the kept form, the doc comment opening with the name and the specification's field and the field as `doc(alias)`; `control_packet_size_code` the same, for `bMaxPacketSize0` |
| units spelled out (`unit_symbols.py`) | 0 refused |
| question shape (`question_shape.py`) | 0 refused |
| British spelling (`british_spelling.py`) | 0 refused |
| contract names (`contract_names.py`) | 0 refused |
| undeclared abbreviations (`abbreviations.py`, report only) | `evdev` and `keycaps`, both kept: `evdev` is the name of Linux's input event interface, and `keycaps` is a word of this domain |
| prose acronyms (`prose_acronyms.py`) | 13 findings, the move lane's: `USB`, `IN`, `HID`, `ABI`, `LEDs` in `lib.rs`; `GUI`, `ISO`, `AT`, `US`, `PC`, `AS` in `boot_keyboard.rs`; `OUT` in `descriptor.rs`; `TRB` in `request.rs`. Each is expanded at its first use or added to the template's word list |

Provenance. The usage table takes its usages and names from the USB-IF HID Usage Tables 1.5 and its
numbers from the `KEY_*` names of Linux's `input-event-codes.h`, an ABI header, as milestone 1019
(the driver crates carry facts from their sources, not their text) item 2 rules. Linux's
`hid_keyboard[]` is a test cross-check only. The table now runs from 0x04 to 0x8b, and the rerun
cross-check agrees on 140 of its 144 rows; the four differences are this crate's choices, which
`the_table_matches_its_linux_cross_check` lists.

Result of `script/crate-standalone`. Eight failures, each one of the move's own steps: the crate
name and 48 names in the crate are not ratified (`python3 helpers/name_release.py --draft usb`
prints the table); the manifest inherits `license` and `lints` and has no `rust-version` (item 1);
and the README and both license files are missing (item 4). The build passes for the host and the
three bare-metal targets on stable, all five harnesses prove, and each of the five falsification
patches turns its harness red.

Outstanding before the move. These are milestone 1111's items for the move lane:

- Item 1: the manifest (`description`, `repository`, `readme`, `keywords`, `categories`,
  `rust-version`, the license and lint tables written out).
- Item 2: the package and directory renamed to `universal_serial_bus`, `#[doc(alias = "usb")]` at
  the crate root, the `Name:` blocks stripped.
- Item 4: the scaffold files, the README with the three bounds above and the cross-check's two facts
  (item 13), and a stranger test.
- Item 10: two fuzz targets.
- The 13 prose acronym findings above.
- The work of Decision 2 (`changes` emits HID usages) and of Decision 3's other half
  (`DeviceDescriptor::parse` answers `Result`), both decided provisionally by the maintainer and
  not yet built.

Recommendation. Ready for review of the names below. The move waits for items 1, 2, 4 and 10 and for
the work of Decisions 2 and 3.

## Names

Every public name of the crate and its state. All are provisional; the architect ratifies.

### Renamed by Decision 1's recommendation, provisional until it is ruled

| was | now |
|---|---|
| `find_boot_keyboard(config)` | `find_boot_keyboard(configuration)` |
| `BootKeyboard::max_packet` | `BootKeyboard::maximum_packet_bytes` |
| `Refusal::PacketTooSmall { max_packet }` | `Refusal::PacketTooSmall { maximum_packet_bytes }` |
| `DeviceDescriptor::max_packet0` | `DeviceDescriptor::control_packet_size_code`, alias `bMaxPacketSize0` |
| `control_max_packet(b_max_packet0, superspeed)` | `control_maximum_packet_bytes(control_packet_size_code, speed: Speed)` |
| `DEVICE_LEN`, `CONFIGURATION_LEN`, `INTERFACE_LEN`, `ENDPOINT_LEN` | `DEVICE_DESCRIPTOR_BYTES`, `CONFIGURATION_HEADER_BYTES`, `INTERFACE_DESCRIPTOR_BYTES`, `ENDPOINT_DESCRIPTOR_BYTES` |
| `REPORT_LEN` | `REPORT_BYTES` |
| `DeviceDescriptor::usb` | `DeviceDescriptor::usb_release_binary_coded_decimal`, alias `bcdUSB` |
| `parse(b)` on the three parsers | `parse(bytes)` |

### New, provisional

| name | what it is |
|---|---|
| `descriptor::Speed` (`Low`, `Full`, `High`, `Super`) | the bus speed `control_maximum_packet_bytes` judges a packet size by (item 11, the speed half of Decision 3). USB vocabulary, not a host controller's port speed id |

### Kept as the specification spells them, provisional

| name | specification |
|---|---|
| `ConfigurationHeader::total_length` | `wTotalLength`, USB 2.0 Table 9-10 |
| `SetupPacket::length` | `wLength`, USB 2.0 Table 9-2 |
| `ERROR_ROLL_OVER` | `ErrorRollOver`, Usage Tables section 10 |

### Unchanged, provisional

| module | names |
|---|---|
| `descriptor` | `DEVICE`, `CONFIGURATION`, `INTERFACE`, `ENDPOINT`, `CLASS_HID`, `SUBCLASS_BOOT`, `PROTOCOL_KEYBOARD`, `Refusal` and its variants `Truncated`, `NotThatDescriptor`, `Malformed { at }`, `NoBootKeyboard`, `NoInterruptIn { interface }`; `DeviceDescriptor` and its fields `class`, `vendor`, `product`, `configurations`; `ConfigurationHeader` and its fields `interfaces`, `value`; `BootKeyboard` and its fields `configuration`, `interface`, `endpoint`, `interval` |
| `request` | `DEVICE_TO_HOST`, `CLASS`, `TO_INTERFACE`, `GET_DESCRIPTOR`, `SET_CONFIGURATION`, `SET_IDLE`, `SET_PROTOCOL`, `BOOT_PROTOCOL`, `SetupPacket` and its fields `request_type`, `request`, `value`, `index`; `get_descriptor`, `set_configuration`, `set_boot_protocol`, `set_idle_forever`, `is_device_to_host`, `to_u64` |
| `boot_keyboard` | `BootReport` and its fields `modifiers`, `keys`; `BootReport::RELEASED`, `MODIFIER_CODES`, `evdev_code`, `changes` |

`MODIFIER_CODES` leaves the public surface when the work of Decision 2 lands.
