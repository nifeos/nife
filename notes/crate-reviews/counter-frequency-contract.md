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
reader names a raw address, and it has no Kani harness. Every name has a ratified block, but three carry the
unit symbol `HZ`, which the later rule on units refuses.

## Identity

| item | value |
|---|---|
| crate | `counter_frequency_contract`, ratified on #3059 |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root |
| `publish` | `false`; milestone 1017 (the contract crates leave in this order) keeps moved contract crates unpublished until their own release milestone |
| size | one `src/lib.rs` of 292 lines: about 215 of library and 80 of tests |
| identity values | `MAGIC` (`TIMEBAS1`), `VERSION` 1, `INTERFACE_ID` 9 |
| consumers in nife | `kernel`, `user_mode_runtime`, `process_builder`, `components` and `system_tests` (development only), by path |

## Public API

| kind | items |
|---|---|
| layout | `MAGIC`, `VERSION`, `INTERFACE_ID`, `PAGE_CONTENT_BYTES` (16), `PAGE_VIRTUAL_ADDRESS` (`0x7000_0000_0000`, or `0x38_0000_0000` on `riscv64`) |
| rate band | `MINIMUM_PLAUSIBLE_HZ`, `MAXIMUM_PLAUSIBLE_HZ`, `is_plausible(hz)` |
| writer | `build_page(hz) -> [u8; 16]` |
| reader | `CounterFrequencyPage`, `unsafe fn new(virtual_address: u64)`, `hz() -> Option<u64>` |

What a stranger may find surprising:

- `CounterFrequencyPage::new` takes an address as a `u64` and is `unsafe`; the caller promises 16
  mapped, unchanging bytes for the value's whole life. The type is `Copy`, `Send` and `Sync` by
  `unsafe impl`, on the argument that the page has one writer before any reader exists.
- `hz()` answers `Some` for any page with the magic, so a page built with a rate of 0 reads as
  `Some(0)`. The crate documents a zeroed frame as "unknown", and the reader must still call
  `is_plausible`.
- `PAGE_VIRTUAL_ADDRESS` exists on aarch64 too, with the `x86_64` value, although aarch64 maps no
  page and reads `CNTFRQ_EL0` instead.

## Proofs and falsification patches

None. The crate has no Kani harness and no `falsifications/` directory, and the README says so.
Five host tests cover the round trip, a zeroed frame, a wrong magic, the band's edges and the
layout offsets. The `unsafe` surface is one `from_raw_parts` behind `new`'s contract, and no proof
covers it, since Kani cannot prove a caller's promise about a mapping.

## Names

#3059 ratified all 13 names on 2026-11-01. Three of them, and two parameters, use the unit
symbol `HZ`. The rule of 2026-11-03 spells every unit out (`_HERTZ`) and accepts no SI symbol,
because uppercase erases the case that tells `MHz` from `mHz`. The rule leaves a ratified name
alone until its own review, and a crate's release review is that review.

| name | kind | recommendation |
|---|---|---|
| `MINIMUM_PLAUSIBLE_HZ` | constant | rename to `MINIMUM_PLAUSIBLE_HERTZ` |
| `MAXIMUM_PLAUSIBLE_HZ` | constant | rename to `MAXIMUM_PLAUSIBLE_HERTZ` |
| `CounterFrequencyPage::hz` | method | rename to `hertz` |
| `hz` | parameter of `is_plausible` and `build_page` | rename to `hertz`; a parameter name breaks no caller |

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
`CNTFRQ_EL0` and `CPUID.80000007H:EDX[8]` are register names cited as facts.

## What the move still needs

| item | owner |
|---|---|
| The README says the crate "was written for the nife kernel and its user-mode runtime"; reword it. | move lane |
| The `Send` and `Sync` safety comment cites `environment_contract::ConfigPage`, and two tests cite `environment_contract` and `clock_contract`, all crates in nife. State each argument in the crate's own terms. | move lane |
| The doc comments name radon (a VisionFive 2) and "every rate this tree has met"; say "a VisionFive 2" and "rates real machines have". | move lane |
| Ratify the identity value `INTERFACE_ID = 9`, which milestone 1017 asks for before a contract crate leaves. | architect |
| The crate leaves after `address_space_map` in milestone 1017's order. The table there lists a development dependency on it; the manifest no longer has one, so the order may no longer bind. | maintainer |

## Proposed items

1. The documentation says the page carries "the calibrated TSC rate" (`build_page`, `hz`), and
   `new`'s safety section says the kernel maps it "into every `x86_64` process". `PAGE_VIRTUAL_ADDRESS`
   says every `x86_64` and every `riscv64` process, and on `riscv64` the rate is the `time` CSR's.
   Proposed: say "the counter's rate", and name both architectures in `new`.
2. `new`'s safety section has a sentence that starts in lowercase ("the kernel maps exactly such a
   page"), left by an earlier edit.
3. Whether `hz()` should answer `None` for a rate outside the band. Proposed: keep the signature
   and document that the reader checks `is_plausible`, since a later release can tighten the
   behavior without breaking a caller. The maintainer can decide it.

## Open questions

1. The unit spelling of the three ratified names in the table above, before the crate leaves.
   Recommendation: rename all three. Each name is a published identifier of the kind the units rule
   exists for, and the crate has five consumers in one tree today, so the rename costs one lane.
   **architect** (renaming a ratified name). The same question stands for
   `designware_mobile_storage` and `jh7110_clock_and_reset`, and one ruling can cover all three.
2. Whether `PAGE_VIRTUAL_ADDRESS` should exist on aarch64. Recommendation: leave it, because the
   host build takes the same arm and tests need a value. Removing it later breaks an aarch64 caller,
   and none exists. The maintainer can decide it.

## Result of `script/crate-standalone`

One failure: `proofs`, because the crate has no Kani harness. Manifest, files, citations, names and
the build on 1.88 for the host and the three bare-metal targets pass. Whether a contract crate this
small needs a harness is open; the reader's one decision is the magic compare, and a harness that
`hz` answers `None` unless the magic matches would cost little. The maintainer can decide it.

## Recommendation

Blocked by open question 1, the unit spelling, and by the move lane's three edits. After those,
ready for review.
