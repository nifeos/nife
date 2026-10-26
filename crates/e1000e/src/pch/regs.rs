/// Strap register, read only; FreeBSD writes it to order a config-space read before reset.
/// Bits 5:1 also give the NVM's size ([`super::non_volatile_memory::Geometry`]).
pub(crate) const STRAP: u64 = 0x0_000c;
/// MDI control: the MDIO transaction register ([`super::phy`]).
pub(crate) const MDIC: u64 = 0x0_0020;
/// Future extended NVM.
pub(crate) const FEXTNVM: u64 = 0x0_0028;
/// Future extended NVM 3.
pub(crate) const FEXTNVM3: u64 = 0x0_003c;
/// Future extended NVM 7.
pub(crate) const FEXTNVM7: u64 = 0x0_00e4;
/// LED control.
pub(crate) const LEDCTL: u64 = 0x0_0e00;
/// Extended configuration control: the semaphore, the PHY-configuration gate, and the pointer
/// to the NVM's extended configuration region.
pub(crate) const EXTCNF_CTRL: u64 = 0x0_0f00;
/// Extended configuration size.
pub(crate) const EXTCNF_SIZE: u64 = 0x0_0f08;
/// PHY control as the MAC sees it (`E1000_PHY_CTRL`): LPLU and gigabit disable.
pub(crate) const PHY_CTRL: u64 = 0x0_0f10;
/// I/O side-band fabric power control; SPT's transmit errata write it.
pub(crate) const IOSFPC: u64 = 0x0_0f28;
/// Packet buffer allocation.
pub(crate) const PBA: u64 = 0x0_1000;
/// Packet buffer ECC status.
pub(crate) const PBECCSTS: u64 = 0x0_100c;
/// Receive descriptor control, queue 0.
pub const RXDCTL: u64 = 0x0_2828;
/// Analog front end band gap transmit reference data.
pub(crate) const KABGTXD: u64 = 0x0_3004;
/// Transmit descriptor control, queue 0.
pub(crate) const TXDCTL0: u64 = 0x0_3828;
/// Transmit arbitration control, queue 0.
pub(crate) const TARC0: u64 = 0x0_3840;
/// Transmit descriptor control, queue 1.
pub(crate) const TXDCTL1: u64 = 0x0_3928;
/// Transmit arbitration control, queue 1.
pub(crate) const TARC1: u64 = 0x0_3940;
/// Host to Management Engine.
pub(crate) const H2ME: u64 = 0x0_5b50;
/// Firmware semaphore (Management Engine status).
pub(crate) const FWSM: u64 = 0x0_5b54;
/// Future extended NVM 11.
pub const FEXTNVM11: u64 = 0x0_5bbc;
