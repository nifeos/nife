//! Name: ratified 2026-10-26 (UTC), in the tree-wide ruling that turns every `regs` module into
//! `registers`, which names e1000e's. The constants inside keep their sources' names. Refused
//! `regs` (an abbreviation).

/// **Strap** (`E1000_STRAP`, FreeBSD's `e1000_ich8lan.h`). Read only; FreeBSD writes it to order a
/// config-space read before reset. Bits 5:1 also give the NVM's size
/// ([`super::non_volatile_memory::Geometry`]).
#[doc(alias = "strap")]
pub(crate) const STRAP: u64 = 0x0_000c;
/// **MDI Control** (`E1000_MDIC`, FreeBSD's `e1000_regs.h`). The MDIO transaction register
/// ([`super::physical_layer_device`]).
#[doc(alias = "mdi control")]
pub(crate) const MDIC: u64 = 0x0_0020;
/// **Future Extended NVM** (`E1000_FEXTNVM`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended nvm")]
pub(crate) const FEXTNVM: u64 = 0x0_0028;
/// **Future Extended NVM 3** (`E1000_FEXTNVM3`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended nvm 3")]
pub(crate) const FEXTNVM3: u64 = 0x0_003c;
/// **Future Extended NVM 7** (`E1000_FEXTNVM7`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended nvm 7")]
pub(crate) const FEXTNVM7: u64 = 0x0_00e4;
/// **LED Control** (`E1000_LEDCTL`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "led control")]
pub(crate) const LEDCTL: u64 = 0x0_0e00;
/// **Extended Configuration Control** (`E1000_EXTCNF_CTRL`, FreeBSD's `e1000_regs.h`). The
/// semaphore, the PHY-configuration gate, and the pointer to the NVM's extended configuration
/// region.
#[doc(alias = "extended configuration control")]
pub(crate) const EXTCNF_CTRL: u64 = 0x0_0f00;
/// **Extended Configuration Size** (`E1000_EXTCNF_SIZE`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "extended configuration size")]
pub(crate) const EXTCNF_SIZE: u64 = 0x0_0f08;
/// **PHY Control Register in CSR** (`E1000_PHY_CTRL`, FreeBSD's `e1000_regs.h`). The PHY control
/// the MAC holds: LPLU and gigabit disable.
#[doc(alias = "phy control register in csr")]
pub(crate) const PHY_CTRL: u64 = 0x0_0f10;
/// **IOSF Power Control** (`E1000_IOSFPC`, FreeBSD's `e1000_regs.h`). The I/O side-band fabric's
/// power control, which SPT's transmit errata write. FreeBSD's comment reads "TX corrupted data".
#[doc(alias = "iosf power control")]
pub(crate) const IOSFPC: u64 = 0x0_0f28;
/// **Packet Buffer Allocation** (`E1000_PBA`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "packet buffer allocation")]
pub(crate) const PBA: u64 = 0x0_1000;
/// **Packet Buffer ECC Status** (`E1000_PBECCSTS`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "packet buffer ecc status")]
pub(crate) const PBECCSTS: u64 = 0x0_100c;
/// **Receive Descriptor Control** (`E1000_RXDCTL(0)`, FreeBSD's `e1000_regs.h`). Queue 0.
#[doc(alias = "receive descriptor control")]
pub const RXDCTL: u64 = 0x0_2828;
/// **AFE Band Gap Transmit Reference Data** (`E1000_KABGTXD`, FreeBSD's `e1000_regs.h`). AFE is the
/// analog front end.
#[doc(alias = "analog front end band gap transmit reference data")]
pub(crate) const KABGTXD: u64 = 0x0_3004;
/// **Transmit Descriptor Control** (`E1000_TXDCTL(0)`, FreeBSD's `e1000_regs.h`). Queue 0.
#[doc(alias = "transmit descriptor control")]
pub(crate) const TXDCTL0: u64 = 0x0_3828;
/// **Transmit Arbitration Count** (`E1000_TARC(0)`, FreeBSD's `e1000_regs.h`). Queue 0.
#[doc(alias = "transmit arbitration count")]
pub(crate) const TARC0: u64 = 0x0_3840;
/// **Transmit Descriptor Control** (`E1000_TXDCTL(1)`, FreeBSD's `e1000_regs.h`). Queue 1.
#[doc(alias = "transmit descriptor control")]
pub(crate) const TXDCTL1: u64 = 0x0_3928;
/// **Transmit Arbitration Count** (`E1000_TARC(1)`, FreeBSD's `e1000_regs.h`). Queue 1.
#[doc(alias = "transmit arbitration count")]
pub(crate) const TARC1: u64 = 0x0_3940;
/// **Host to ME** (`E1000_H2ME`, FreeBSD's `e1000_ich8lan.h`). Messages from the driver to the
/// Management Engine.
#[doc(alias = "host to me")]
pub(crate) const H2ME: u64 = 0x0_5b50;
/// **FW Semaphore** (`E1000_FWSM`, FreeBSD's `e1000_regs.h`). The firmware semaphore: the
/// Management Engine's status.
#[doc(alias = "fw semaphore")]
pub(crate) const FWSM: u64 = 0x0_5b54;
/// **Future Extended NVM 11** (`E1000_FEXTNVM11`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended nvm 11")]
pub const FEXTNVM11: u64 = 0x0_5bbc;
