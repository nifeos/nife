# Review package: `counter_frequency_contract`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `counter_frequency_contract`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the five packages
together outgrow the page.*

## In one paragraph

The layout of a 16-byte read-only page that tells a process how many counter ticks make a second,
on the architectures with no register that says so: the magic `TIMEBAS1`, then the rate as one
little-endian `u64`. The crate also holds the page's fixed virtual address per architecture and the
band of rates worth believing (100 kHz to 100 GHz), which the kernel's three boot paths share. It is
`no_std` with no dependency. Unlike the other crates in this note, it has `unsafe`, because the
reader names a raw address, and two Kani harnesses cover the rest. Every name has a ratified block, and each unit is spelled out.

## Identity

| item | value |
|---|---|
| crate | `counter_frequency_contract`, ratified on #3059 |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root |
| `publish` | `false`; milestone 1017 (the contract crates leave in this order) keeps moved contract crates unpublished until their own release milestone |
| size | one `src/lib.rs` |
| identity values | frozen: `MAGIC` (`TIMEBAS1`), `VERSION` 1, `INTERFACE_ID` 9, `PAGE_CONTENT_BYTES` 16 (the magic at offset 0, the little-endian rate at offset 8) and `PAGE_VIRTUAL_ADDRESS` `0x0000_7000_0000_0000` on `x86_64` and `0x0000_0038_0000_0000` on `riscv64`. A layout break ships as a new magic and version together. The crate header and README state them, and a host test pins them as literals. Frozen 2026-11-11 (UTC) on #3516 |
| consumers in nife | `kernel`, `user_mode_runtime`, `process_builder`, `components` and `system_tests` (development only), by path |

## Public API

| kind | items |
|---|---|
| layout | `MAGIC`, `VERSION`, `INTERFACE_ID`, `PAGE_CONTENT_BYTES` (16), `PAGE_VIRTUAL_ADDRESS` (`0x7000_0000_0000`, or `0x38_0000_0000` on `riscv64`) |
| rate band | `MINIMUM_PLAUSIBLE_HERTZ`, `MAXIMUM_PLAUSIBLE_HERTZ`, `is_plausible(hertz)` |
| writer | `build_page(hertz) -> [u8; 16]` |
| reader | `CounterFrequencyPage`, `unsafe fn new(virtual_address: u64)`, `hertz() -> Option<u64>` |

What a stranger may find surprising:

- `CounterFrequencyPage::new` takes an address as a `u64` and is `unsafe`; the caller promises 16
  mapped, unchanging bytes for the value's whole life. The type is `Copy`, `Send` and `Sync` by
  `unsafe impl`, on the argument that the page has one writer before any reader exists.
- `hertz()` answers `Some` for any page with the magic, so a page built with a rate of 0 reads as
  `Some(0)`. The crate documents a zeroed frame as "unknown", and the reader applies
  `is_plausible` itself; `user_mode_runtime::cntfrq_checked` does.
- `PAGE_VIRTUAL_ADDRESS` exists on aarch64 too, with the `x86_64` value, although aarch64 maps no
  page and reads `CNTFRQ_EL0` instead.

## Proofs and falsification patches

Two Kani harnesses, each with a falsification patch in `falsifications/`: `hertz` answers `None`
exactly when the first eight bytes differ from the magic and otherwise the little-endian number
that follows, and `build_page(x).hertz()` is `Some(x)` for every `u64`. Six host tests cover the
round trip, a zeroed frame, a wrong magic, the band's edges, a rate outside the band reading back
as written, and the layout offsets. The `unsafe` surface is one `from_raw_parts` behind `new`'s
contract, and no proof covers it, since Kani cannot prove a caller's promise about a mapping.

## Names

#3059 ratified all 13 names on 2026-11-01. Names spell each unit out (`_HERTZ`) and use no SI symbol,
because uppercase erases the case that tells `MHz` from `mHz`.

| name | kind |
|---|---|
| `MINIMUM_PLAUSIBLE_HERTZ` | constant |
| `MAXIMUM_PLAUSIBLE_HERTZ` | constant |
| `CounterFrequencyPage::hertz` | method |
| `hertz` | parameter of `is_plausible` and `build_page` |

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
`CNTFRQ_EL0` and `CPUID.80000007H:EDX[8]` are register names cited as facts.

## What the move still needs

| item | owner |
|---|---|
| The crate leaves after `address_space_map` in milestone 1017's order. The table there lists a development dependency on it; the manifest no longer has one, so the order may no longer bind. | maintainer |

## Proposed items

1. Whether `hertz()` should answer `None` for a rate outside the band. The signature stays and the
   documentation says the reader applies `is_plausible`, since a later release can tighten the
   behavior without breaking a caller. The maintainer can decide it.

## Open questions

1. Whether `PAGE_VIRTUAL_ADDRESS` should exist on aarch64. Recommendation: leave it, because the
   host build takes the same arm and tests need a value. Removing it later breaks an aarch64 caller,
   and none exists. The maintainer can decide it.

## Result of `script/crate-standalone`

Manifest, files, citations, names, the proofs and the build on 1.88 for the host and the three
bare-metal targets pass.

## Recommendation

Ready for review.
