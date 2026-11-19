//! The words of one port's glue, relative to
//! [`PortNumber::system_top_group_controller_base`].
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.

/// The AXI slave read-channel word; bits 20:17 select a physical function.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_AR_OFFSET")]
pub const READ_CHANNEL: u32 = 0x78;
/// [`READ_CHANNEL`]'s field the bring-up clears, bits 22:8.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_AXI4_SLVL_AR_MASK")]
pub const READ_CHANNEL_MASK: u32 = 0x7fff << 8;
/// [`READ_CHANNEL`]: the physical function select's shift.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const READ_CHANNEL_FUNCTION_SHIFT: u32 = 17;
/// The AXI slave write-channel word; bits 12:9 select a physical function.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_AW_OFFSET")]
pub const WRITE_CHANNEL: u32 = 0x7c;
/// [`WRITE_CHANNEL`]'s field the bring-up clears, bits 14:0.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_AXI4_SLVL_AW_MASK")]
pub const WRITE_CHANNEL_MASK: u32 = 0x7fff;
/// [`WRITE_CHANNEL`]: the physical function select's shift.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const WRITE_CHANNEL_FUNCTION_SHIFT: u32 = 9;
/// [`WRITE_CHANNEL`]: the controller asserts CLKREQ#.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_CLKREQ")]
pub const CLOCK_REQUEST: u32 = 1 << 22;
/// [`WRITE_CHANNEL`]: the reference clock source field, bits 19:18.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_CKREF_SRC_MASK")]
pub const REFERENCE_CLOCK_SOURCE_MASK: u32 = 0x3 << 18;
/// The reference clock source the bring-up selects, already shifted.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const REFERENCE_CLOCK_SOURCE: u32 = 2 << 18;
/// The root port word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_RP_NEP_OFFSET")]
pub const ROOT_PORT: u32 = 0xe8;
/// [`ROOT_PORT`]: the port is a root port and not an endpoint.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_K_RP_NEP")]
pub const ROOT_PORT_NOT_ENDPOINT: u32 = 1 << 8;
/// The link status word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "STG_SYSCON_LNKSTA_OFFSET")]
pub const LINK_STATUS: u32 = 0x170;
/// [`LINK_STATUS`]: the data link layer is up.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const DATA_LINK_ACTIVE: u32 = 1 << 5;
/// The physical functions a port has; the bring-up disables every one but function 0.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_FUNC_NUM")]
pub const FUNCTIONS: u32 = 4;
