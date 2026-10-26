//! Name: ratified 2026-10-26 (UTC), in the tree-wide ruling that turns every `regs` module into
//! `registers`, which names e1000e's. The constants inside keep their sources' names. Refused
//! `regs` (an abbreviation).

/// Device control.
pub const CTRL: u64 = 0x0_0000;
/// Device status (read only).
pub const STATUS: u64 = 0x0_0008;
/// Extended device control.
pub const CTRL_EXT: u64 = 0x0_0018;
/// Interrupt cause read (read to clear).
pub const ICR: u64 = 0x0_00c0;
/// Interrupt mask clear (write only).
pub const IMC: u64 = 0x0_00d8;
/// Receive control.
pub const RCTL: u64 = 0x0_0100;
/// Transmit control.
pub const TCTL: u64 = 0x0_0400;
/// Transmit inter-packet gap.
pub const TIPG: u64 = 0x0_0410;
/// Receive descriptor base, low 32 bits, queue 0.
pub const RDBAL: u64 = 0x0_2800;
/// Receive descriptor base, high 32 bits.
pub const RDBAH: u64 = 0x0_2804;
/// Receive ring length in bytes (a multiple of 128).
pub const RDLEN: u64 = 0x0_2808;
/// Receive head (the device's cursor).
pub const RDH: u64 = 0x0_2810;
/// Receive tail (the driver's cursor). **The same 4 KiB page as [`RDBAL`].**
pub const RDT: u64 = 0x0_2818;
/// Transmit descriptor base, low 32 bits, queue 0.
pub const TDBAL: u64 = 0x0_3800;
/// Transmit descriptor base, high 32 bits.
pub const TDBAH: u64 = 0x0_3804;
/// Transmit ring length in bytes.
pub const TDLEN: u64 = 0x0_3808;
/// Transmit head.
pub const TDH: u64 = 0x0_3810;
/// Transmit tail. **The same 4 KiB page as [`TDBAL`].**
pub const TDT: u64 = 0x0_3818;
/// Receive filter control (`EXTEN` selects extended descriptors).
pub const RFCTL: u64 = 0x0_5008;
/// Multicast table array: 128 32-bit entries, the DWORDs of Intel's documentation.
pub const MTA: u64 = 0x0_5200;
/// How many 32-bit entries [`MTA`] has.
///
/// Name: ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused
/// `MTA_DWORDS` (a DWORD is 32 bits in Intel's word sizes and not in Arm's or RISC-V's, so the
/// count names what the table holds).
#[doc(alias = "dwords")]
pub const MTA_ENTRIES: u64 = 128;
/// Receive address low, entry 0 (the MAC address's first four octets).
pub const RAL0: u64 = 0x0_5400;
/// Receive address high, entry 0 (the last two octets, and [`super::RAH_AV`]).
pub const RAH0: u64 = 0x0_5404;
