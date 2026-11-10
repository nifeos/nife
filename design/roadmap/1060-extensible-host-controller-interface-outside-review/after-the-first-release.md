# P2: after the first release

An appendix to [the milestone](../1060-extensible-host-controller-interface-outside-review.md).
Every item here is two-way and the maintainers' provisional call, except P2-12, whose field waits on
the architect; the main page carries its recommendation. A name an item mints is provisional.

## P2-1. Two clamps that cannot fire, and a doc comment that claims them

For full and low speed, `interrupt_interval` in `src/context.rs` computes the period in
microframes as `descriptor_interval * 8`, between 8 and 2,040, so the exponent always lies between
3 and 10, and both clamps on it are dead. The doc comment says the field is "clamped to the
specification's 3 (1 ms) to 10 (128 ms)", which reads as cases the code handles. Five mutants on
those comparisons and on the high-speed clamp survive for that reason. Change: delete both clamps,
and say in the doc comment that 3 to 10 is the range the arithmetic produces. The harness's
`(3..=10)` assertion stays as the proof. Exempt the surviving `>` to `>=` on the high-speed clamp
as equivalent, since both give 16. XS.

## P2-2. Five proofs are cheap and missing

The README says "Not yet proven: event decoding". Four more properties lack a proof:

- each TRB builder places the slot in bits 31:24 and the endpoint in bits 20:16 of its last dword,
  and the pointer in the first two (a host test checks fixed values);
- `Capabilities::decode` keeps each field to its width (the fuzz target `capability_registers`
  checks it, the model checker does not);
- `PortDebounce::sample` answers by the timeout for any sample sequence;
- a producer and a modeled consumer agree across a wrap (the host test runs three laps of a
  four-entry ring with fixed bursts).

 Change: five harnesses, each with a falsification patch that the proofs workflow replays
red, and the README's "Not yet proven" line goes. S.

## P2-3. The crate's front page on docs.rs shows no code

The crate documentation in `src/lib.rs` is prose and limits; the example lives in the README, which
the crate compiles as a doctest and docs.rs does not render. Change: the README example in the
crate documentation, and one sentence per module on when a driver reaches for it. XS.

## P2-4. The event ring dequeue pointer comes back without the busy bit a write needs

`EventRing::dequeue_pointer` says it is the physical address to write to `ERDP`, and a write that
does not also set the Event Handler Busy bit leaves the interrupter's busy state pending. The
crate's driver writes `dequeue_pointer() | EVENT_RING_DEQUEUE_POINTER_BUSY`; the documentation does
not say to. Change: `EventRing::dequeue_pointer_register_value()` including the bit, or one sentence
on `dequeue_pointer` saying to set it. XS.

## P2-5. One interrupter

`Capabilities::interrupter` reaches interrupter 0 only, through `INTERRUPTER_0_REGISTER`; the
stride between interrupter register sets is 32 bytes, and nothing reads the decoded
`maximum_interrupters`. Change: `interrupter_register(interrupter: u16, register_offset: u64)` beside the
existing method. Additive. XS.

## P2-6. The Supported Protocol capability is absent

`src/extended_capabilities.rs` names one id, `LEGACY_SUPPORT_EXTENDED_CAPABILITY_ID`. The Supported
Protocol capability (id 2) says which port numbers belong to USB 2 and which to USB 3, and carries
the speed id table a controller may define in place of the defaults `port::speed` assumes. Change:
`SUPPORTED_PROTOCOL_EXTENDED_CAPABILITY_ID` and a decoder for the capability's revision, name string
and port range, and a README `BUGS` line now saying the crate assumes the default speed ids.
Additive. S.

## P2-7. The driver's other waits have no named constants

`src/port.rs` carries the debounce figures (`DEBOUNCE_STABLE_MILLISECONDS` and its two siblings)
with their source, and nothing else a driver waits on: the command completion bound, the port reset
bound, the controller-not-ready wait after reset, the halt wait. Change: named constants in
milliseconds beside them, each with its source (the specification's timing table or Linux's
defaults). Additive. XS.

## P2-8. The direct memory access layout is one driver's, and the module does not say so

`src/direct_memory_access.rs` fixes eleven pages for one slot, one keyboard, four report slots and
one scratchpad page, and says both ends read these constants. Nothing else in the crate reads them,
so a stranger writing a different driver meets a layout they need not use. Change: one paragraph at
the module's head saying it is the layout of a single-slot boot-keyboard driver, that nothing else
in the crate depends on it, and that a different driver lays out its own. XS.

## P2-9. The bit constants carry no `doc(alias)` for their mnemonics

The register offsets carry the specification's mnemonic as an alias (`USBCMD`, `IMAN`), and so do
the interrupter bits. The command, status, port and legacy bits do not: `COMMAND_RUN` (`RS`),
`COMMAND_RESET` (`HCRST`), `STATUS_HALTED` (`HCH`), `CONNECTED` (`CCS`), `ENABLED` (`PED`) and the
rest in `src/registers.rs` and `src/port.rs`. Change: `#[doc(alias = "PORTSC.CCS")]` and the rest,
in the form the interrupter bits use. Additive. XS.

## P2-10. Small text fixes for a stranger

- `CODEOWNERS` says the maintainers' team may not exist; it does. Drop the caveat.
- `CONTRIBUTING.md` says the crate stays "free of vocabulary from any other tree", which means
  nothing to an outside reader. Say the `standalone` job refuses references to other repositories.
- `cargo deny check` warns that the `Unicode-3.0` and `Zlib` allowances in `deny.toml` match
  nothing. Leave the scaffold's list, and let this line record the warning.
- The repository's GitHub description is the scaffold's sentence and does not say xHCI, and its
  topics lack `xhci` and `usb`. Set both through the template's settings path.
- The name of the host test `xenons_ports_are_all_counted_once_they_settle` in `src/port.rs` names a
  bench machine a stranger cannot identify, with a dated probe in its doc comment. Name it for what
  the ports did.

XS.

## P2-11. `rust-version` is higher than the code needs

The library and its tests build and pass at 1.85 except for one call to `is_multiple_of` in the
host test `a_port_that_never_holds_still_ends_at_the_timeout`, stabilized in 1.87. Edition 2024
needs 1.85, and lowering later is compatible while raising is a change users feel. Change: write
the test's check as a remainder, state 1.85, and let the weekly `msrv` job hold it. XS.

## P2-12. The Event Data bit is ignored

When a transfer event has its Event Data bit (bit 2 of its last dword) on, it carries the event data value
in its pointer field, not a TRB address. The crate builds no Event Data TRBs, so its own controller
never sets the bit, and `Event::decode` in `src/event.rs` would report the value as an address in
`Event::Transfer`. Whether the variant grows an `event_data` field is one-way and waits on the
architect; the documentation sentence is two-way and lands now. XS.
