//! Name: ratified 2026-10-26 (UTC), in the tree-wide ruling that turns every `regs` module into
//! `registers`, which names e1000e's. The constants inside keep their sources' names. Refused
//! `regs` (an abbreviation).

/// **Device Control** (`E1000_CTRL`, Linux's `regs.h`).
#[doc(alias = "device control")]
pub const CTRL: u64 = 0x0_0000;
/// **Device Status** (`E1000_STATUS`, Linux's `regs.h`). Read only.
#[doc(alias = "device status")]
pub const STATUS: u64 = 0x0_0008;
/// **Extended Device Control** (`E1000_CTRL_EXT`, Linux's `regs.h`).
#[doc(alias = "extended device control")]
pub const CTRL_EXT: u64 = 0x0_0018;
/// **Interrupt Cause Read** (`E1000_ICR`, Linux's `regs.h`). Reading it clears it.
#[doc(alias = "interrupt cause read")]
pub const ICR: u64 = 0x0_00c0;
/// **Interrupt Mask Clear** (`E1000_IMC`, Linux's `regs.h`). Write only.
#[doc(alias = "interrupt mask clear")]
pub const IMC: u64 = 0x0_00d8;
/// **Receive Control** (`E1000_RCTL`, Linux's `regs.h`).
#[doc(alias = "receive control")]
pub const RCTL: u64 = 0x0_0100;
/// **Transmit Control** (`E1000_TCTL`, Linux's `regs.h`).
#[doc(alias = "transmit control")]
pub const TCTL: u64 = 0x0_0400;
/// **Transmit Inter-Packet Gap** (`E1000_TIPG`, Linux's `regs.h`).
#[doc(alias = "transmit inter-packet gap")]
pub const TIPG: u64 = 0x0_0410;
/// **Receive Descriptor Base Address Low** (`E1000_RDBAL(0)`, Linux's `regs.h`). The low 32 bits,
/// queue 0.
#[doc(alias = "receive descriptor base address low")]
pub const RDBAL: u64 = 0x0_2800;
/// **Receive Descriptor Base Address High** (`E1000_RDBAH(0)`, Linux's `regs.h`). The high 32 bits.
#[doc(alias = "receive descriptor base address high")]
pub const RDBAH: u64 = 0x0_2804;
/// **Receive Descriptor Length** (`E1000_RDLEN(0)`, Linux's `regs.h`). The ring length in bytes, a
/// multiple of 128.
#[doc(alias = "receive descriptor length")]
pub const RDLEN: u64 = 0x0_2808;
/// **Receive Descriptor Head** (`E1000_RDH(0)`, Linux's `regs.h`). The device's cursor.
#[doc(alias = "receive descriptor head")]
pub const RDH: u64 = 0x0_2810;
/// **Receive Descriptor Tail** (`E1000_RDT(0)`, Linux's `regs.h`). The driver's cursor. **The same
/// 4 KiB page as [`RDBAL`].**
#[doc(alias = "receive descriptor tail")]
pub const RDT: u64 = 0x0_2818;
/// **Transmit Descriptor Base Address Low** (`E1000_TDBAL(0)`, Linux's `regs.h`). The low 32 bits,
/// queue 0.
#[doc(alias = "transmit descriptor base address low")]
pub const TDBAL: u64 = 0x0_3800;
/// **Transmit Descriptor Base Address High** (`E1000_TDBAH(0)`, Linux's `regs.h`). The high 32
/// bits.
#[doc(alias = "transmit descriptor base address high")]
pub const TDBAH: u64 = 0x0_3804;
/// **Transmit Descriptor Length** (`E1000_TDLEN(0)`, Linux's `regs.h`). The ring length in bytes.
#[doc(alias = "transmit descriptor length")]
pub const TDLEN: u64 = 0x0_3808;
/// **Transmit Descriptor Head** (`E1000_TDH(0)`, Linux's `regs.h`).
#[doc(alias = "transmit descriptor head")]
pub const TDH: u64 = 0x0_3810;
/// **Transmit Descriptor Tail** (`E1000_TDT(0)`, Linux's `regs.h`). **The same 4 KiB page as
/// [`TDBAL`].**
#[doc(alias = "transmit descriptor tail")]
pub const TDT: u64 = 0x0_3818;
/// **Receive Filter Control** (`E1000_RFCTL`, Linux's `regs.h`). `EXTEN` selects extended
/// descriptors.
#[doc(alias = "receive filter control")]
pub const RFCTL: u64 = 0x0_5008;
/// **Multicast Table Array** (`E1000_MTA`, Linux's `regs.h`). 128 32-bit entries, the DWORDs of
/// Intel's documentation.
#[doc(alias = "multicast table array")]
pub const MTA: u64 = 0x0_5200;
/// How many 32-bit entries [`MTA`] has.
///
/// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
/// `MTA_DWORDS` (a DWORD is 32 bits in Intel's word sizes and not in Arm's or RISC-V's, so the
/// count names what the table holds).
#[doc(alias = "dwords")]
pub const MTA_ENTRIES: u64 = 128;
/// **Receive Address Low** (`E1000_RAL(0)`, Linux's `regs.h`). Entry 0: the MAC address's first
/// four octets.
#[doc(alias = "receive address low")]
pub const RAL0: u64 = 0x0_5400;
/// **Receive Address High** (`E1000_RAH(0)`, Linux's `regs.h`). Entry 0: the last two octets, and
/// [`super::RAH_AV`].
#[doc(alias = "receive address high")]
pub const RAH0: u64 = 0x0_5404;
