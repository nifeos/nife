//! **What the driver writes to the controller's control registers**: the bits it sets, and the
//! Receive Control, Transmit Control and Transmit Inter-Packet Gap (`RCTL`, `TCTL` and `TIPG`) values it programs, and the MAC address read back.
//!
//! Name: ratified 2026-11-03 (UTC), #3145. Renamed from `control`, which did not say that the
//! module holds the controller's control registers.

/// Device Control (`CTRL`): set link up. OpenBSD's `E1000_CTRL_SLU` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `CTRL_SLU`: the abbreviation is spelled out, and the source spellings are the aliases.
#[doc(alias = "CTRL_SLU")]
#[doc(alias = "E1000_CTRL_SLU")]
pub const DEVICE_CONTROL_SET_LINK_UP: u32 = 0x0000_0040;

/// Device Control (`CTRL`): auto-speed detection enable. OpenBSD's `E1000_CTRL_ASDE` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `CTRL_ASDE`: the abbreviation is spelled out, and the source spellings are the aliases.
#[doc(alias = "CTRL_ASDE")]
#[doc(alias = "E1000_CTRL_ASDE")]
pub const DEVICE_CONTROL_AUTO_SPEED_DETECTION_ENABLE: u32 = 0x0000_0020;

/// Device Control (`CTRL`): global reset, self-clearing. OpenBSD's `E1000_CTRL_RST` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `CTRL_RST`: the abbreviation is spelled out, and the source spellings are the aliases.
#[doc(alias = "CTRL_RST")]
#[doc(alias = "E1000_CTRL_RST")]
pub const DEVICE_CONTROL_GLOBAL_RESET: u32 = 0x0400_0000;

/// Extended Device Control (`CTRL_EXT`): tells management firmware a driver has the device. OpenBSD's `E1000_CTRL_EXT_DRV_LOAD` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `CTRL_EXT_DRV_LOAD`: the abbreviation is spelled out, and the source spellings are the aliases.
#[doc(alias = "CTRL_EXT_DRV_LOAD")]
#[doc(alias = "E1000_CTRL_EXT_DRV_LOAD")]
pub const EXTENDED_DEVICE_CONTROL_DRIVER_LOAD: u32 = 0x1000_0000;

/// Device Status (`STATUS`): link up. OpenBSD's `E1000_STATUS_LU` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `STATUS_LU`: the abbreviation is spelled out, and the source spellings are the aliases.
#[doc(alias = "STATUS_LU")]
#[doc(alias = "E1000_STATUS_LU")]
pub const DEVICE_STATUS_LINK_UP: u32 = 0x0000_0002;

/// Receive Filter Control (`RFCTL`): extended receive descriptors. Cleared, so the device writes the legacy format. OpenBSD's `E1000_RFCTL_EXTEN` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `RFCTL_EXTEN`: the abbreviation is spelled out, and the source spellings are the aliases.
#[doc(alias = "RFCTL_EXTEN")]
#[doc(alias = "E1000_RFCTL_EXTEN")]
pub const RECEIVE_FILTER_CONTROL_EXTENDED_DESCRIPTORS_ENABLE: u32 = 0x0000_8000;

/// Receive Address High (`RAH`): the address in this entry is valid. OpenBSD's `E1000_RAH_AV` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `RAH_AV`: the abbreviation is spelled out, and the source spellings are the aliases.
#[doc(alias = "RAH_AV")]
#[doc(alias = "E1000_RAH_AV")]
pub const RECEIVE_ADDRESS_HIGH_ADDRESS_VALID: u32 = 0x8000_0000;

/// `RCTL`: receive enable (Linux's `regs.h`).
const RCTL_EN: u32 = 0x0000_0002;

/// `RCTL`: accept broadcast, which DHCP's offer needs (`E1000_RCTL_BAM`).
const RCTL_BAM: u32 = 0x0000_8000;

/// `RCTL`: strip the CRC (`E1000_RCTL_SECRC`).
const RCTL_SECRC: u32 = 0x0400_0000;

// The three share no bit, so | and ^ compose the word identically; the compiler checks that here,
// and .cargo/mutants.toml excludes receive_control_register_value's OR-to-XOR swaps on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(RCTL_EN ^ RCTL_BAM ^ RCTL_SECRC == RCTL_EN | RCTL_BAM | RCTL_SECRC);

/// **Receive Control** (`E1000_RCTL`, Linux's `regs.h`) as the kernel programs it: enabled,
/// broadcast accepted (DHCP's offer is broadcast), 2048-byte buffers (size field zero), the CRC
/// stripped. **Not** promiscuous: unicast is filtered to receive-address entry 0, which the process
/// cannot write.
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `rctl`: the register mnemonic is spelled out, and it stays as an alias beside the register's title.
#[doc(alias = "receive control")]
#[doc(alias = "rctl")]
#[doc(alias = "E1000_RCTL")]
pub const fn receive_control_register_value() -> u32 {
    RCTL_EN | RCTL_BAM | RCTL_SECRC
}

/// `TCTL`: transmit enable (Linux's `regs.h`).
const TCTL_EN: u32 = 0x0000_0002;

/// `TCTL`: pad short frames (`E1000_TCTL_PSP`).
const TCTL_PSP: u32 = 0x0000_0008;

// The four operands share no bit, so | and ^ compose the word identically; the compiler checks
// that here, and .cargo/mutants.toml excludes transmit_control_register_value's OR-to-XOR swaps on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    TCTL_EN ^ TCTL_PSP ^ (15 << 4) ^ (63 << 12) == TCTL_EN | TCTL_PSP | (15 << 4) | (63 << 12)
);

/// **Transmit Control** (`E1000_TCTL`, Linux's `regs.h`) as the kernel programs it: enabled, short
/// frames padded, collision threshold 15 and distance 63 (Linux's `E1000_COLLISION_THRESHOLD` and
/// `E1000_COLLISION_DISTANCE`, `defines.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `tctl`: the register mnemonic is spelled out, and it stays as an alias beside the register's title.
#[doc(alias = "transmit control")]
#[doc(alias = "tctl")]
#[doc(alias = "E1000_TCTL")]
pub const fn transmit_control_register_value() -> u32 {
    TCTL_EN | TCTL_PSP | 15 << 4 | 63 << 12
}

// The three field values share no bit, so | and ^ compose the word identically; the compiler
// checks that here, and .cargo/mutants.toml excludes transmit_inter_packet_gap_register_value's OR-to-XOR swaps on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(8 ^ (8 << 10) ^ (6 << 20) == 8 | (8 << 10) | (6 << 20));

/// **Transmit Inter-Packet Gap** (`E1000_TIPG`, Linux's `regs.h`) for copper: IPGT 8, IPGR1 8,
/// IPGR2 6 (Linux's `DEFAULT_82543_TIPG_*`).
///
/// Name: ratified 2026-11-03 (UTC), #3150. Renamed from `tipg`: the register mnemonic is spelled out, and it stays as an alias beside the register's title.
#[doc(alias = "transmit inter-packet gap")]
#[doc(alias = "tipg")]
#[doc(alias = "E1000_TIPG")]
pub const fn transmit_inter_packet_gap_register_value() -> u32 {
    8 | 8 << 10 | 6 << 20
}

/// The station address (the MAC address) in receive-address entry 0, or `None` when the entry is
/// not marked valid, which on a real part means the NVM load did not happen and on QEMU does not
/// occur.
///
/// Name: ratified 2026-10-26 (#2761). Ruled in Decision 49 (2026-10-26 UTC): `mac` holding the
/// six-byte hardware address is spelled `station_address`, tree-wide. Refused
/// `mac_from_receive_address` (`mac` names the controller, not the address).
#[doc(alias = "mac")]
#[doc(alias = "mac_address")]
pub fn station_address_from_receive_address(ral: u32, rah: u32) -> Option<[u8; 6]> {
    if rah & RECEIVE_ADDRESS_HIGH_ADDRESS_VALID == 0 {
        return None;
    }
    let l = ral.to_le_bytes();
    let h = rah.to_le_bytes();
    let station_address = [l[0], l[1], l[2], l[3], h[0], h[1]];
    // All zeros, or a group address, is not an address a station can own.
    (station_address != [0; 6] && station_address[0] & 1 == 0).then_some(station_address)
}
