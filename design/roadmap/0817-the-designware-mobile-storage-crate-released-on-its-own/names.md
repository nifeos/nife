# Names in `designware_mobile_storage`, kept beside its milestone

<!-- writing-standards: exception. Marked 2026-10-25 (UTC) by lane crates-stand-alone. Reason: each block below is a provenance record moved verbatim from the crate's source, refusals and wording included, and a record keeps its words. -->

An appendix of [milestone 817 (`designware_mobile_storage`: a proven DesignWare SD and eMMC host,
released on its own)](../0817-the-designware-mobile-storage-crate-released-on-its-own.md). The
crate ships on its own and may cite nothing of this tree, so the `Name:` block of its crate, its
modules and its marked items lives here. `script/names` and the module-block ratchet read each
paragraph below as if it sat at the name (helpers/name_census.py, `_sidecars`).

`crates/designware_mobile_storage/src/lib.rs` Name: ratified 2026-10-07 (calef, #1806): the databook's name for the part, spelled out as §154 (the acronym test is whether the phrase is spoken) asks; permanent on first publication. The argument the minting lane gave: PROVISIONAL (milestone 53's storage lane, 2026-10-06 UTC). The Synopsys databook's own name for the part, spelled out as `designware_ethernet` (the same lane's network half) spells its own. An architect may prefer `designware_mmc` (Linux's `dw_mmc`) or a name for what it does rather than whose IP it is.

`crates/designware_mobile_storage/src/bench.rs::IdmacReport` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/bench.rs::idmac_read` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/bench.rs::Watch` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/designware_mobile_storage/src/bench.rs::new` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/designware_mobile_storage/src/bench.rs::clear` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/designware_mobile_storage/src/bench.rs::MatrixRow` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/designware_mobile_storage/src/bench.rs::MATRIX_HZ` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/designware_mobile_storage/src/bench.rs::MATRIX_BLOCKS` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/designware_mobile_storage/src/bench.rs::read_matrix` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/designware_mobile_storage/src/host.rs::with_idmac` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/idmac.rs` Name: provisional (milestone 53 (the board's own peripherals)'s IDMAC lane, 2026-10-22 UTC). The databook's own name for the engine, as `regs` names its registers.

`crates/designware_mobile_storage/src/idmac.rs::DirectMemoryAccessRegion` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/idmac.rs::NoDirectMemoryAccess` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/idmac.rs::Refusal` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/idmac.rs::Idmac` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/idmac.rs::new` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/regs.rs::IDSTS` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/regs.rs::DSCADDR` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.

`crates/designware_mobile_storage/src/regs.rs::BUFADDR` Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.
