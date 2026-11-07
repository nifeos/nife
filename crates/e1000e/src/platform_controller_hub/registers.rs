//! Name: ratified 2026-10-26 (#2761). Ruled in the tree-wide ruling that turns every `regs` module
//! into `registers`, which names e1000e's. Refused `regs` (an abbreviation). The constants inside
//! are spelled out and end `_REGISTER`, each keeping its source spelling as a `#[doc(alias)]`
//! (ruled 2026-11-03 (UTC), #3149).

/// **Strap** (`E1000_STRAP`, FreeBSD's `e1000_ich8lan.h`). Read only; FreeBSD writes it to order a
/// config-space read before reset. Bits 5:1 also give the NVM's size
/// ([`super::non_volatile_memory::Geometry`]).
#[doc(alias = "strap")]
pub(crate) const STRAP: u64 = 0x0_000c;
/// **MDI Control** (`E1000_MDIC`, FreeBSD's `e1000_regs.h`). The MDIO transaction register
/// ([`super::physical_layer_device`]).
#[doc(alias = "management data interface control")]
pub(crate) const MDIC: u64 = 0x0_0020;
/// **Future Extended NVM** (`E1000_FEXTNVM`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended non-volatile memory")]
pub(crate) const FEXTNVM: u64 = 0x0_0028;
/// **Future Extended NVM 3** (`E1000_FEXTNVM3`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended non-volatile memory 3")]
pub(crate) const FEXTNVM3: u64 = 0x0_003c;
/// **Future Extended NVM 7** (`E1000_FEXTNVM7`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended non-volatile memory 7")]
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
#[doc(alias = "physical layer device control register in control and status registers")]
#[doc(alias = "E1000_PHY_CTRL")]
pub(crate) const PHYSICAL_LAYER_DEVICE_CONTROL_REGISTER: u64 = 0x0_0f10;
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
/// **Receive Descriptor Control** (OpenBSD's `E1000_RXDCTL(0)`, `sys/dev/pci/if_em_hw.h`). Queue 0.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RXDCTL`; that and Intel's `E1000_RXDCTL`
/// stay as aliases.
#[doc(alias = "receive descriptor control")]
#[doc(alias = "E1000_RXDCTL")]
#[doc(alias = "RXDCTL")]
pub const RECEIVE_DESCRIPTOR_CONTROL_REGISTER: u64 = 0x0_2828;
/// **Analog Front End Band Gap Transmit Reference Data** (`E1000_KABGTXD`, FreeBSD's
/// `e1000_regs.h`).
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
#[doc(alias = "host to management engine")]
pub(crate) const H2ME: u64 = 0x0_5b50;
/// **FW Semaphore** (`E1000_FWSM`, FreeBSD's `e1000_regs.h`). The firmware semaphore: the
/// Management Engine's status.
#[doc(alias = "firmware semaphore")]
pub(crate) const FWSM: u64 = 0x0_5b54;
/// **Future Extended NVM 11** (OpenBSD's `E1000_FEXTNVM11`, `sys/dev/pci/if_em_hw.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `FEXTNVM11`; that and Intel's
/// `E1000_FEXTNVM11` stay as aliases.
#[doc(alias = "future extended non-volatile memory 11")]
#[doc(alias = "E1000_FEXTNVM11")]
#[doc(alias = "FEXTNVM11")]
pub const FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_REGISTER: u64 = 0x0_5bbc;
