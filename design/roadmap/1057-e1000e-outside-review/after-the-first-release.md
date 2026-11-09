# P2: after the first release

An appendix to [the milestone](../1057-e1000e-outside-review.md).

## P2-1. A VLAN-tagged full frame cannot pass

`MAXIMUM_FRAME_BYTES` (`src/descriptor.rs`) is 1514. The controller without `RCTL.LPE` accepts 1522
bytes with CRC, 1518 after stripping, so a tagged 1518-byte frame reads `Drop` and `transmit`
refuses it. Change: either raise the constant to 1518, which fits the 2048-byte buffer and stays
compatible for a caller that sizes buffers from it, or state the limit in the README's BUGS.
Two-way. S.

## P2-2. A caller buffer smaller than a frame loses the frame silently

`DataPlane::receive` judges with `out.len()` as the room, so a 576-byte `out` drops every larger
frame. The `dropped` counter takes it, and nothing tells it from a bad frame. Change: a distinct
verdict (`TooLongForCaller`) or a distinct counter, and a sentence in the doc of `receive`.
Two-way. S.

## P2-3. `DataPlane::new` accepts a base address that overflows the region

`start` adds `layout::receive_buffer(index)` to the base without a check. A base near `u64::MAX`
panics in a debug build and wraps in a release build. Change: `new` asserts that
`base + DIRECT_MEMORY_ACCESS_REGION_BYTES` does not overflow. Two-way. S.

## P2-4. One tail write per received frame

`receive` reposts the slot and writes `RDT` after every frame; Linux and the ixy drivers batch
tail writes. Change: measure under QEMU and on hardware first. Then, if it matters, repost lazily
and write the tail once per burst. Two-way. M.

## P2-5. No interrupt vocabulary

`src/registers.rs` carries the `ICR` and `IMC` offsets and nothing of `IMS`, `ITR` or the cause
bits, so a user who wants to sleep on the device must look elsewhere. Change: the offsets and the
receive and transmit cause bits, each spelled out with its source alias, and a sentence in the
README on how a polled driver becomes an interrupt-driven one. Two-way. S.

## P2-6. Constants fix the ring size at 16

`RECEIVE_ENTRIES` and `TRANSMIT_ENTRIES` (`src/descriptor.rs`) shape the layout and the proofs; a
user wanting 256 descriptors cannot have them. Change: none for 0.1, as the milestone's decisions
list records. A const-generic or runtime size is a later break. One-way when it happens. L.

## P2-7. The test doubles are private

`SimulatedDevice` and `SimulatedHardware` are `cfg(test)`, so a user implementing the traits writes
a fake from scratch. Change: a `simulator` feature, or the example of P1-5 as the reusable fake.
Two-way. M.

## P2-8. No fuzz target, although the crate qualifies

The crate has no `unsafe`, so the weekly fuzz run could carry a target. It would drive `DataPlane`
against an arbitrary simulated device and assert the ring invariants of P1-6. `fuzz.yml` opts out
today, and no `fuzz/` directory exists. Change: one target and `public-crash-inputs: true`.
Two-way. S.

## P2-9. Port the link-up half, after the measurement of P1-7

`e1000_check_for_copper_link_ich8lan` and what it calls, with its own simulator cases. Two-way. L,
and it needs hardware.

## P2-10. The flush assumes the ring length of the crate for the device's

`empty_descriptor_rings` (`src/platform_controller_hub/flush.rs`) reads `TDLEN` to decide whether
to flush, then reduces the tail modulo `crate::TRANSMIT_ENTRIES` instead of `TDLEN / 16`. Change:
derive the entry count from the `TDLEN` it already read. Two-way. S.

## P2-11. Two harnesses prove less than their names say

The paged-access harness asserts that the page is not 800, but not that the function refuses pages
1 to 767. The frame harness asserts the three upper bounds, but not that an error bit or a missing
end-of-packet yields `Drop`. Change: add the assertions, each with a line in the falsification
patch. Two-way. S.

## P2-12. Single-bit public constants written as hex, not shifts

`RECEIVE_STATUS_DESCRIPTOR_DONE`, `RECEIVE_STATUS_END_OF_PACKET`, `TRANSMIT_STATUS_DESCRIPTOR_DONE`,
`FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_DISABLE_MULR_FIX`, `DESCRIPTOR_RING_FLUSH_REQUIRED` and the
two `MDIC` bits use hex literals; the renamed private bits already use shifts. The names stay; only
the literal form changes. Two-way. S.

## P2-13. Private names that the rules would spell differently

`reset_blocked` and `accessible` answer yes or no without `is_`. `PAGE` is a byte count without its
unit. `READ_COMMAND_TIMEOUT` and `POLLS` are limits without `MAXIMUM_`.
`I218_ULTRA_LOW_POWER_CONFIGURATION1` is a register offset without `_REGISTER`. All are private, so
the maintainers rule them provisionally. Two-way. S.

## P2-14. Wording a stranger cannot place

The crate doc speaks of "the process" and two halves as if every reader had the same system.
`flush.rs` says "the architect chose not to guess". The doc of the device table cites the PCI
survey of one bench machine, and the GitHub description is template text ("a standalone nifeos
crate"). Change: "a driver may split into a kernel half and a stack half; this crate serves
either", "the maintainers chose not to guess", "one Sunrise Point desktop", and a description that
names the device family. Two-way. S.

## P2-15. Doc gaps a user hits first

`HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER` is an offset from `FLASH_REGISTERS_OFFSET_BYTES`, not
from BAR0, and its doc does not say so. An 82574L user also needs `Hardware`, for
`read_identifier_at_device_number`, yet it lives under `platform_controller_hub`. Change: the
sentence, and a re-export of `Hardware` at the crate root (additive). Two-way. S.

## P2-16. Housekeeping

`CODEOWNERS` names a team that may not exist. The template pin lags the `main` of the template;
Dependabot will offer the bump, and the maintainers take it. `.github/accepted-words` carries a
commented example from the domain of another crate. Two-way. S.
