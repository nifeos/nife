# Units

*An appendix to [`design/naming.md`](../naming.md). It holds how a name carries a unit.*

## A name spells its unit

A name that holds a quantity ends with its unit, spelled out: `_HERTZ`, `_MILLISECONDS`,
`_MICROSECONDS`, `_NANOSECONDS`, `_SECONDS`, `_MINUTES`, `_BYTES`, `megabits_per_second`. No SI
symbol is a valid unit in a name: not `_HZ`, `_MS`, `_NS`, `_US`, `_S` or `mbps`.

The reason is case. Rust writes a constant in uppercase, and uppercase erases the case that SI
symbols depend on: `MHZ` could mean megahertz or millihertz, and `_MS` could be megasiemens. The
micro sign has no ASCII symbol at all, so `us` already reads two ways. A spelled-out unit reads one
way in every casing.

- A length or a limit carries its unit: `MAXIMUM_NAME_BYTES`, never `MAXIMUM_NAME_LEN`.
- A limit opens with `MAXIMUM_` or `MINIMUM_` and puts the unit last: `MAXIMUM_FRAME_BYTES`,
  `MAXIMUM_TIME_ZONE_BYTES`, `MINIMUM_MEMORY_KIB`.
- A count of things names the things: `MAXIMUM_TRANSLATION_UNITS`, `MAXIMUM_COLUMNS`.
- `bytes` is a word. A binary multiple keeps the IEC prefix as a word part (`kib`, `mib`, `gib`),
  since it has one meaning in every casing.
- A Rust standard-library method keeps std's spelling (`as_nanos`, `from_secs`), and so does a
  smoltcp time method (`total_millis`). A name this tree defines does not copy it.

## Where the existing names stand

About 95 existing names use a symbol (`TRANSFER_HZ`, `CARD_INTERFACE_UNIT_HZ`, `MATRIX_HZ`,
`clock_contract::NS_PER_SECOND`, `MAXIMUM_STEP_FORWARD_NS`). A separate sweep renames them. Until
it lands, a new or renamed name follows this page, and a ratified symbol name keeps its spelling
until its own name review.

## BUGS

- `helpers/unit_symbols.py` counts only `nanos`, `millis`, `micros`, `us`, `usec`, `sec`, `secs`,
  `mins` and a bare `s` after `per` or a number. It does not count `ns`, `ms`, `hz`, `khz` or `mhz`,
  so the gate passes a new name with one of those symbols. Its `respell` table still suggests `ns`
  and `ms` for `nanos` and `millis`. The ceiling stays as it is until a lane moves the check to
  this page's rule and banks the new count.
- The helper leaves `nsec`, `msec`, `kb`, `mb`, `gb`, `tb`, `bps` and `ppm` uncounted.
