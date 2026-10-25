//! **What the driver writes to the controller's control registers**: the bits it sets, and the
//! `RCTL`, `TCTL` and `TIPG` values it programs, and the MAC address read back.

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

/// `RCTL` as the kernel programs it: enabled, broadcast accepted (DHCP's offer is broadcast),
/// 2048-byte buffers (size field zero), the CRC stripped. **Not** promiscuous: unicast is filtered
/// to receive-address entry 0, which the process cannot write.
pub const fn rctl() -> u32 {
    const EN: u32 = 0x0000_0002;
    const BAM: u32 = 0x0000_8000;
    const SECRC: u32 = 0x0400_0000;
    EN | BAM | SECRC
}

/// `TCTL` as the kernel programs it: enabled, short frames padded, collision threshold 15 and
/// distance 63 (Linux's full-duplex values, `defines.h`).
pub const fn tctl() -> u32 {
    const EN: u32 = 0x0000_0002;
    const PSP: u32 = 0x0000_0008;
    EN | PSP | 15 << 4 | 63 << 12
}

/// `TIPG` for copper: IPGT 8, IPGR1 8, IPGR2 6 (Linux's `DEFAULT_82543_TIPG_*`).
pub const fn tipg() -> u32 {
    8 | 8 << 10 | 6 << 20
}

/// The MAC address in receive-address entry 0, or `None` when the entry is not marked valid, which
/// on a real part means the NVM load did not happen and on QEMU does not occur.
pub fn mac_from_receive_address(ral: u32, rah: u32) -> Option<[u8; 6]> {
    if rah & RAH_AV == 0 {
        return None;
    }
    let l = ral.to_le_bytes();
    let h = rah.to_le_bytes();
    let mac = [l[0], l[1], l[2], l[3], h[0], h[1]];
    // All zeros, or a group address, is not an address a station can own.
    (mac != [0; 6] && mac[0] & 1 == 0).then_some(mac)
}
