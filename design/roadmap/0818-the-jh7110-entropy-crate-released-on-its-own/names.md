# Names in `jh7110_entropy`, kept beside its milestone

<!-- writing-standards: exception. Marked 2026-10-25 (UTC) by lane crates-stand-alone. Reason: each block below is a provenance record moved verbatim from the crate's source, refusals and wording included, and a record keeps its words. -->

An appendix of [milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its
own)](../0818-the-jh7110-entropy-crate-released-on-its-own.md). The crate ships on its own and may
cite nothing of this tree, so the `Name:` block of its crate and its marked items lives here.
`script/names` and the module-block ratchet read each paragraph below as if it sat at the name
(helpers/name_census.py, `_sidecars`).

`crates/jh7110_entropy/src/lib.rs` Name: ratified 2026-09-13 (calef); kept as the crates.io name (calef, 2026-10-07 UTC, #1806). The argument its minting lane gave (milestone 159's lane, 2026-08-24): chip-qualified and unambiguous rather than generic, since `trng` alone would be a generic word that could name almost anything, the same reasoning that keeps `non_volatile_memory_express` and `pci` named for the specification rather than for "storage" or "bus".

`crates/jh7110_entropy/src/lib.rs::COMPATIBLE` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, made public when the device-tree walk left the crate. The binding's own word for the property it holds.

`crates/jh7110_entropy/src/lib.rs::COMPATIBLES` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone: both spellings, in the order to try them.

`crates/jh7110_entropy/src/lib.rs::interrupt` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, for the property it decodes.

`crates/jh7110_entropy/src/lib.rs::status_okay` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the `Discovered` field it fills.
