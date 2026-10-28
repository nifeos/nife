//! **What the driver writes to the controller's control registers**: the bits it sets, and the
//! `RCTL`, `TCTL` and `TIPG` values it programs, and the MAC address read back.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// `CTRL`: set link up.
pub const CTRL_SLU: u32 = 0x0000_0040;

/// `CTRL`: auto-speed detection.
pub const CTRL_ASDE: u32 = 0x0000_0020;

/// `CTRL`: global reset, self-clearing.
pub const CTRL_RST: u32 = 0x0400_0000;

/// `CTRL_EXT`: tells management firmware a driver has the device.
pub const CTRL_EXT_DRV_LOAD: u32 = 0x1000_0000;

/// `STATUS`: link up.
pub const STATUS_LU: u32 = 0x0000_0002;

/// `RFCTL`: extended receive descriptors. Cleared, so the device writes the legacy format.
pub const RFCTL_EXTEN: u32 = 0x0000_8000;

/// `RAH`: the address in this entry is valid.
pub const RAH_AV: u32 = 0x8000_0000;

/// `RCTL`: receive enable (Linux's `regs.h`).
const RCTL_EN: u32 = 0x0000_0002;

/// `RCTL`: accept broadcast, which DHCP's offer needs (`E1000_RCTL_BAM`).
const RCTL_BAM: u32 = 0x0000_8000;

/// `RCTL`: strip the CRC (`E1000_RCTL_SECRC`).
const RCTL_SECRC: u32 = 0x0400_0000;

// The three share no bit, so | and ^ compose the word identically; the compiler checks that here,
// and .cargo/mutants.toml excludes rctl's OR-to-XOR swaps on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(RCTL_EN ^ RCTL_BAM ^ RCTL_SECRC == RCTL_EN | RCTL_BAM | RCTL_SECRC);

/// **Receive Control** (`E1000_RCTL`, Linux's `regs.h`) as the kernel programs it: enabled,
/// broadcast accepted (DHCP's offer is broadcast), 2048-byte buffers (size field zero), the CRC
/// stripped. **Not** promiscuous: unicast is filtered to receive-address entry 0, which the process
/// cannot write.
#[doc(alias = "receive control")]
pub const fn rctl() -> u32 {
    RCTL_EN | RCTL_BAM | RCTL_SECRC
}

/// `TCTL`: transmit enable (Linux's `regs.h`).
const TCTL_EN: u32 = 0x0000_0002;

/// `TCTL`: pad short frames (`E1000_TCTL_PSP`).
const TCTL_PSP: u32 = 0x0000_0008;

// The four operands share no bit, so | and ^ compose the word identically; the compiler checks
// that here, and .cargo/mutants.toml excludes tctl's OR-to-XOR swaps on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    TCTL_EN ^ TCTL_PSP ^ (15 << 4) ^ (63 << 12) == TCTL_EN | TCTL_PSP | (15 << 4) | (63 << 12)
);

/// **Transmit Control** (`E1000_TCTL`, Linux's `regs.h`) as the kernel programs it: enabled, short
/// frames padded, collision threshold 15 and distance 63 (Linux's full-duplex values, `defines.h`).
#[doc(alias = "transmit control")]
pub const fn tctl() -> u32 {
    TCTL_EN | TCTL_PSP | 15 << 4 | 63 << 12
}

// The three field values share no bit, so | and ^ compose the word identically; the compiler
// checks that here, and .cargo/mutants.toml excludes tipg's OR-to-XOR swaps on this assertion.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(8 ^ (8 << 10) ^ (6 << 20) == 8 | (8 << 10) | (6 << 20));

/// **Transmit Inter-Packet Gap** (`E1000_TIPG`, Linux's `regs.h`) for copper: IPGT 8, IPGR1 8,
/// IPGR2 6 (Linux's `DEFAULT_82543_TIPG_*`).
#[doc(alias = "transmit inter-packet gap")]
pub const fn tipg() -> u32 {
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
    if rah & RAH_AV == 0 {
        return None;
    }
    let l = ral.to_le_bytes();
    let h = rah.to_le_bytes();
    let station_address = [l[0], l[1], l[2], l[3], h[0], h[1]];
    // All zeros, or a group address, is not an address a station can own.
    (station_address != [0; 6] && station_address[0] & 1 == 0).then_some(station_address)
}
