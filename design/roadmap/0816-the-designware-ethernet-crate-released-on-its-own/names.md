# Names in `designware_ethernet`, kept beside its milestone

<!-- writing-standards: exception. Marked 2026-10-25 (UTC) by lane crates-stand-alone. Reason: each block below is a provenance record moved verbatim from the crate's source, refusals and wording included, and a record keeps its words. -->

An appendix of [milestone 816 (`designware_ethernet`: proven DesignWare Ethernet QoS logic, released
on its own)](../0816-the-designware-ethernet-crate-released-on-its-own.md). The crate ships on its
own and may cite nothing of this tree, so the `Name:` block of its crate, its modules and its marked
items lives here. `script/names` and the module-block ratchet read each paragraph below as if it sat
at the name (helpers/name_census.py, `_sidecars`).

`crates/designware_ethernet/src/lib.rs` Name: ratified 2026-10-07 (calef, #1806): Synopsys's name, spelled out per §154 (the acronym test). The argument its minting lane gave: PROVISIONAL (milestone 53 (the board's own peripherals: network and storage on real silicon)'s lane, 2026-10-06 UTC); the manifest pointed to this paragraph for the argument and the alternative an architect may prefer.

`crates/designware_ethernet/src/data_plane.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/designware_ethernet/src/descriptor.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/designware_ethernet/src/handoff.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/designware_ethernet/src/hardware.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/designware_ethernet/src/coherence.rs::is_transmit_complete` Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-question-shapes. Refused `transmit_completed` (a participle alone reads as a getter).
