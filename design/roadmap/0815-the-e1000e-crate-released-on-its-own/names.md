# Names in `e1000e`, kept beside its milestone

<!-- writing-standards: exception. Marked 2026-10-25 (UTC) by lane crates-stand-alone. Reason: each block below is a provenance record moved verbatim from the crate's source, refusals and wording included, and a record keeps its words. -->

Several blocks cite milestone 494 (a driver for the network card a PC actually has), where the
crate began.

An appendix of [milestone 815 (`e1000e`: proven Intel e1000e ring logic, released on its
own)](../0815-the-e1000e-crate-released-on-its-own.md). The crate ships on its own and may cite
nothing of this tree, so the `Name:` block of its crate, its modules and its marked items lives
here. `script/names` and the module-block ratchet read each paragraph below as if it sat at the
name (helpers/name_census.py, `_sidecars`).

`crates/e1000e/src/lib.rs` Name: ratified 2026-10-07 (calef, #1806), permanent on first publication: the family name QEMU's `-device e1000e` and Linux's driver directory use, kept across their interface (design/naming.md, the acronym test). Milestone 815 (proven Intel e1000e ring logic, released on its own) publishes it. The argument its minting lane gave: PROVISIONAL (milestone 494's lane, 2026-10-04). `e1000e` is the family name QEMU's `-device e1000e` and Linux's driver directory both use, so it arrives across somebody else's interface (design/naming.md, the acronym test's second paragraph). An architect may prefer a name for what it is rather than whose driver it resembles.

`crates/e1000e/src/control.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/e1000e/src/data_plane.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/e1000e/src/descriptor.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/e1000e/src/device.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/e1000e/src/handoff.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

`crates/e1000e/src/pch/hardware.rs` Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
