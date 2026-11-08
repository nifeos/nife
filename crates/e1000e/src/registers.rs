//! Each offset is bytes from the start of BAR0, as OpenBSD's `sys/dev/pci/if_em_hw.h` defines it
//! (FreeBSD's `e1000_regs.h` and Linux's `regs.h` carry the same `E1000_*` macros).
//!
//! Name: ratified 2026-10-26 (#2761). Ruled in the tree-wide ruling that turns every `regs` module
//! into `registers`, which names e1000e's. Refused `regs` (an abbreviation). The constants inside
//! are spelled out and end `_REGISTER`, each keeping its source spelling as a `#[doc(alias)]`
//! (ruled 2026-11-03 (UTC), #3149).

/// **Device Control** (OpenBSD's `E1000_CTRL`, `sys/dev/pci/if_em_hw.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `CTRL`; that and Intel's `E1000_CTRL`
/// stay as aliases.
#[doc(alias = "device control")]
#[doc(alias = "E1000_CTRL")]
#[doc(alias = "CTRL")]
pub const DEVICE_CONTROL_REGISTER: u64 = 0x0_0000;
/// **Device Status** (OpenBSD's `E1000_STATUS`, `sys/dev/pci/if_em_hw.h`). Read only.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `STATUS`; that and Intel's `E1000_STATUS`
/// stay as aliases.
#[doc(alias = "device status")]
#[doc(alias = "E1000_STATUS")]
#[doc(alias = "STATUS")]
pub const DEVICE_STATUS_REGISTER: u64 = 0x0_0008;
/// **Extended Device Control** (OpenBSD's `E1000_CTRL_EXT`, `sys/dev/pci/if_em_hw.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `CTRL_EXT`; that and Intel's
/// `E1000_CTRL_EXT` stay as aliases.
#[doc(alias = "extended device control")]
#[doc(alias = "E1000_CTRL_EXT")]
#[doc(alias = "CTRL_EXT")]
pub const EXTENDED_DEVICE_CONTROL_REGISTER: u64 = 0x0_0018;
/// **Interrupt Cause Read** (OpenBSD's `E1000_ICR`, `sys/dev/pci/if_em_hw.h`). Reading it clears it.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `ICR`; that and Intel's `E1000_ICR` stay
/// as aliases.
#[doc(alias = "interrupt cause read")]
#[doc(alias = "E1000_ICR")]
#[doc(alias = "ICR")]
pub const INTERRUPT_CAUSE_READ_REGISTER: u64 = 0x0_00c0;
/// **Interrupt Mask Clear** (OpenBSD's `E1000_IMC`, `sys/dev/pci/if_em_hw.h`). Write only.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `IMC`; that and Intel's `E1000_IMC` stay
/// as aliases.
#[doc(alias = "interrupt mask clear")]
#[doc(alias = "E1000_IMC")]
#[doc(alias = "IMC")]
pub const INTERRUPT_MASK_CLEAR_REGISTER: u64 = 0x0_00d8;
/// **Receive Control** (OpenBSD's `E1000_RCTL`, `sys/dev/pci/if_em_hw.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RCTL`; that and Intel's `E1000_RCTL`
/// stay as aliases.
#[doc(alias = "receive control")]
#[doc(alias = "E1000_RCTL")]
#[doc(alias = "RCTL")]
pub const RECEIVE_CONTROL_REGISTER: u64 = 0x0_0100;
/// **Transmit Control** (OpenBSD's `E1000_TCTL`, `sys/dev/pci/if_em_hw.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `TCTL`; that and Intel's `E1000_TCTL`
/// stay as aliases.
#[doc(alias = "transmit control")]
#[doc(alias = "E1000_TCTL")]
#[doc(alias = "TCTL")]
pub const TRANSMIT_CONTROL_REGISTER: u64 = 0x0_0400;
/// **Transmit Inter-Packet Gap** (OpenBSD's `E1000_TIPG`, `sys/dev/pci/if_em_hw.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `TIPG`; that and Intel's `E1000_TIPG`
/// stay as aliases.
#[doc(alias = "transmit inter-packet gap")]
#[doc(alias = "E1000_TIPG")]
#[doc(alias = "TIPG")]
pub const TRANSMIT_INTER_PACKET_GAP_REGISTER: u64 = 0x0_0410;
/// **Receive Descriptor Base Address Low** (OpenBSD's `E1000_RDBAL(0)`, `sys/dev/pci/if_em_hw.h`). The low 32 bits,
/// queue 0.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RDBAL`; that and Intel's `E1000_RDBAL`
/// stay as aliases.
#[doc(alias = "receive descriptor base address low")]
#[doc(alias = "E1000_RDBAL")]
#[doc(alias = "RDBAL")]
pub const RECEIVE_DESCRIPTOR_BASE_ADDRESS_LOW_REGISTER: u64 = 0x0_2800;
/// **Receive Descriptor Base Address High** (OpenBSD's `E1000_RDBAH(0)`, `sys/dev/pci/if_em_hw.h`). The high 32 bits.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RDBAH`; that and Intel's `E1000_RDBAH`
/// stay as aliases.
#[doc(alias = "receive descriptor base address high")]
#[doc(alias = "E1000_RDBAH")]
#[doc(alias = "RDBAH")]
pub const RECEIVE_DESCRIPTOR_BASE_ADDRESS_HIGH_REGISTER: u64 = 0x0_2804;
/// **Receive Descriptor Length** (OpenBSD's `E1000_RDLEN(0)`, `sys/dev/pci/if_em_hw.h`). The ring length in bytes, a
/// multiple of 128.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RDLEN`; that and Intel's `E1000_RDLEN`
/// stay as aliases.
#[doc(alias = "receive descriptor length")]
#[doc(alias = "E1000_RDLEN")]
#[doc(alias = "RDLEN")]
pub const RECEIVE_DESCRIPTOR_LENGTH_REGISTER: u64 = 0x0_2808;
/// **Receive Descriptor Head** (OpenBSD's `E1000_RDH(0)`, `sys/dev/pci/if_em_hw.h`). The device's cursor.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RDH`; that and Intel's `E1000_RDH` stay
/// as aliases.
#[doc(alias = "receive descriptor head")]
#[doc(alias = "E1000_RDH")]
#[doc(alias = "RDH")]
pub const RECEIVE_DESCRIPTOR_HEAD_REGISTER: u64 = 0x0_2810;
/// **Receive Descriptor Tail** (OpenBSD's `E1000_RDT(0)`, `sys/dev/pci/if_em_hw.h`). The driver's cursor. **The same
/// 4 KiB page as [`RECEIVE_DESCRIPTOR_BASE_ADDRESS_LOW_REGISTER`].**
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RDT`; that and Intel's `E1000_RDT` stay
/// as aliases.
#[doc(alias = "receive descriptor tail")]
#[doc(alias = "E1000_RDT")]
#[doc(alias = "RDT")]
pub const RECEIVE_DESCRIPTOR_TAIL_REGISTER: u64 = 0x0_2818;
/// **Transmit Descriptor Base Address Low** (OpenBSD's `E1000_TDBAL(0)`, `sys/dev/pci/if_em_hw.h`). The low 32 bits,
/// queue 0.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `TDBAL`; that and Intel's `E1000_TDBAL`
/// stay as aliases.
#[doc(alias = "transmit descriptor base address low")]
#[doc(alias = "E1000_TDBAL")]
#[doc(alias = "TDBAL")]
pub const TRANSMIT_DESCRIPTOR_BASE_ADDRESS_LOW_REGISTER: u64 = 0x0_3800;
/// **Transmit Descriptor Base Address High** (OpenBSD's `E1000_TDBAH(0)`, `sys/dev/pci/if_em_hw.h`). The high 32
/// bits.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `TDBAH`; that and Intel's `E1000_TDBAH`
/// stay as aliases.
#[doc(alias = "transmit descriptor base address high")]
#[doc(alias = "E1000_TDBAH")]
#[doc(alias = "TDBAH")]
pub const TRANSMIT_DESCRIPTOR_BASE_ADDRESS_HIGH_REGISTER: u64 = 0x0_3804;
/// **Transmit Descriptor Length** (OpenBSD's `E1000_TDLEN(0)`, `sys/dev/pci/if_em_hw.h`). The ring length in bytes.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `TDLEN`; that and Intel's `E1000_TDLEN`
/// stay as aliases.
#[doc(alias = "transmit descriptor length")]
#[doc(alias = "E1000_TDLEN")]
#[doc(alias = "TDLEN")]
pub const TRANSMIT_DESCRIPTOR_LENGTH_REGISTER: u64 = 0x0_3808;
/// **Transmit Descriptor Head** (OpenBSD's `E1000_TDH(0)`, `sys/dev/pci/if_em_hw.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `TDH`; that and Intel's `E1000_TDH` stay
/// as aliases.
#[doc(alias = "transmit descriptor head")]
#[doc(alias = "E1000_TDH")]
#[doc(alias = "TDH")]
pub const TRANSMIT_DESCRIPTOR_HEAD_REGISTER: u64 = 0x0_3810;
/// **Transmit Descriptor Tail** (OpenBSD's `E1000_TDT(0)`, `sys/dev/pci/if_em_hw.h`). **The same 4 KiB page as
/// [`TRANSMIT_DESCRIPTOR_BASE_ADDRESS_LOW_REGISTER`].**
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `TDT`; that and Intel's `E1000_TDT` stay
/// as aliases.
#[doc(alias = "transmit descriptor tail")]
#[doc(alias = "E1000_TDT")]
#[doc(alias = "TDT")]
pub const TRANSMIT_DESCRIPTOR_TAIL_REGISTER: u64 = 0x0_3818;
/// **Receive Filter Control** (OpenBSD's `E1000_RFCTL`, `sys/dev/pci/if_em_hw.h`). `EXTEN` selects extended
/// descriptors.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RFCTL`; that and Intel's `E1000_RFCTL`
/// stay as aliases.
#[doc(alias = "receive filter control")]
#[doc(alias = "E1000_RFCTL")]
#[doc(alias = "RFCTL")]
pub const RECEIVE_FILTER_CONTROL_REGISTER: u64 = 0x0_5008;
/// **Multicast Table Array** (OpenBSD's `E1000_MTA`, `sys/dev/pci/if_em_hw.h`). 128 32-bit entries, the DWORDs of
/// Intel's documentation.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `MTA`; that and Intel's `E1000_MTA` stay
/// as aliases.
#[doc(alias = "multicast table array")]
#[doc(alias = "E1000_MTA")]
#[doc(alias = "MTA")]
pub const MULTICAST_TABLE_ARRAY_REGISTER: u64 = 0x0_5200;
/// How many 32-bit entries [`MULTICAST_TABLE_ARRAY_REGISTER`] has (OpenBSD's
/// `E1000_NUM_MTA_REGISTERS`, `sys/dev/pci/if_em_hw.h`).
///
/// Name: ratified 2026-11-07 (UTC), #3322. Renamed from `MTA_ENTRIES`, which stays as an alias with
/// OpenBSD's `E1000_NUM_MTA_REGISTERS`. Refused `MTA_DWORDS` (a DWORD is 32 bits in Intel's word
/// sizes and not in Arm's or RISC-V's, so the count names what the table holds).
#[doc(alias = "E1000_NUM_MTA_REGISTERS")]
#[doc(alias = "MTA_ENTRIES")]
#[doc(alias = "dwords")]
pub const MULTICAST_TABLE_ARRAY_ENTRIES: u64 = 128;
/// **Receive Address Low** (OpenBSD's `E1000_RAL(0)`, `sys/dev/pci/if_em_hw.h`). Entry 0: the MAC address's first
/// four octets.
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RAL0`; that and Intel's `E1000_RAL` stay
/// as aliases.
#[doc(alias = "receive address low")]
#[doc(alias = "E1000_RAL")]
#[doc(alias = "RAL0")]
pub const RECEIVE_ADDRESS_0_LOW_REGISTER: u64 = 0x0_5400;
/// **Receive Address High** (OpenBSD's `E1000_RAH(0)`, `sys/dev/pci/if_em_hw.h`). Entry 0: the last two octets, and
/// [`super::RECEIVE_ADDRESS_HIGH_ADDRESS_VALID`].
///
/// Name: ratified 2026-11-03 (UTC), #3149. Renamed from `RAH0`; that and Intel's `E1000_RAH` stay
/// as aliases.
#[doc(alias = "receive address high")]
#[doc(alias = "E1000_RAH")]
#[doc(alias = "RAH0")]
pub const RECEIVE_ADDRESS_0_HIGH_REGISTER: u64 = 0x0_5404;
