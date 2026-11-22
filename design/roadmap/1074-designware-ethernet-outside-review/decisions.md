# Decisions for the architect

An appendix to [the milestone](../1074-designware-ethernet-outside-review.md).

Only one-way doors. Each was a public name or path that the first tag fixes. All four are ruled
and built.

## Decision 1: Where the link type lives

Ruled 2026-11-14 (UTC). A crate-level `link` module holds `Link` and `Speed`. The
`motorcomm_physical_layer_device` module does not re-export either, so every caller imports
`designware_ethernet::link::{Link, Speed}`. A restrictive start leaves room to add a re-export
later; removing one would break callers.

Example: `use designware_ethernet::link::{Link, Speed}; controller::start(&mut hardware, Link {
speed: Speed::Thousand, full_duplex: true }, false);`

## Decision 2: Whether the compile-time gates are API

Ruled 2026-11-14 (UTC). The sixteen `…_DO_NOT_OVERLAP: ()` constants are `pub(crate)`, each with
`#[allow(dead_code)]`, so they are not API. A `pub(crate)` constant is still a name the mutation
exclusions in `.cargo/mutants.toml` can cite, and those comments keep naming them unchanged.

Example: `pub(crate) const COMMAND_WORD_FIELDS_DO_NOT_OVERLAP: () = assert!(…);`

## Decision 3: The five public names the register gate refused

Ruled 2026-11-14 (UTC). The five names carry the spellings below, each `Name:` line says ratified
2026-11-14, and the three length masks keep their specification aliases (`RDES3_LENGTH`,
`TDES2_BUF1_LEN`, `TDES3_FRAME_LEN`). No alias records an earlier nife spelling.

| name | what it is |
|---|---|
| `TRANSMIT_DESCRIPTOR_WORD_2_BUFFER_1_LENGTH_BYTES_MASK` | buffer 1's length field, in bytes |
| `TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_BYTES_MASK` | the whole frame's length field, in bytes |
| `RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_BYTES_MASK` | the received frame's length field, in bytes |
| `DIRECT_MEMORY_ACCESS_REGISTER_PAGE_OFFSET_BYTES` | the page of the window holding channel 0's registers |
| `GMAC0_INTERFACE_SELECT_REGISTER` | the always-on system controller register holding `gmac0`'s interface select, alias `AON_SYSCFG_12` |

Example: `let length = (word_3 & RECEIVE_DESCRIPTOR_WORD_3_PACKET_LENGTH_BYTES_MASK) as usize;`

## Decision 4: The five names that say what they hold

Ruled 2026-11-16 (UTC), option A: `station_mac_address`, `physical_layer_device_bus_address`,
`direct_memory_access_physical_address_bits`, `AUTONEGOTIATION_LINK_PARTNER_100BASE_TWISTED_PAIR`
and `AUTONEGOTIATION_LINK_PARTNER_100BASE_TWISTED_PAIR_FULL_DUPLEX` stand as built. MAC stays an
accepted abbreviation (`design/naming.md`), so each `station_mac_address` field's doc opens with
"media access control".

Example: `let address = discovered.station_mac_address.unwrap_or(from_registers);`

## Not brought

The two Motorcomm alias sets carry OpenBSD's and FreeBSD's spellings and no Linux one (#3319), as
`design/naming.md` requires; they need no ruling. Items 6 and 21 stay two-way until the tag, and the
crate takes the narrow option as the maintainer's provisional call. Item 11 is a template rule.
