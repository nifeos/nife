//! **What the kernel hands the driver** at spawn.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The role word's low half when `arg0` is this server's handoff. Disjoint from `net_stack`'s
/// existing roles: 0 is the virtio server, small integers are socket clients, `0xe1e0` is
/// `e1000e`'s.
pub const ROLE_TAG: u64 = 0xd3e0;

/// **What the kernel hands `net_stack` at spawn**, in two of its three argument words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// The station address the kernel programmed. The process cannot read or change it: the
    /// MAC's page is not mapped.
    pub mac: [u8; 6],
    /// The DMA region's physical base. Descriptors speak physical addresses (there is no IOMMU
    /// on the JH7110), so the process cannot compute them from its virtual window.
    pub data_plane_phys: u64,
}

impl Handoff {
    /// `[arg0, arg1]`: the tag and the MAC address in the role word, the base in the second.
    pub fn pack(&self) -> [u64; 2] {
        let mut mac = [0u8; 8];
        mac[..6].copy_from_slice(&self.mac);
        [
            ROLE_TAG | u64::from_le_bytes(mac) << 16,
            self.data_plane_phys,
        ]
    }

    /// The inverse of [`Handoff::pack`], refusing a role word without the tag, a base that is
    /// zero or not page aligned, and a MAC address no station can own.
    ///
    /// # Examples
    ///
    /// ```
    /// use designware_ethernet::Handoff;
    ///
    /// let h = Handoff { mac: [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c], data_plane_phys: 0x4012_3000 };
    /// let [role, phys] = h.pack();
    /// assert_eq!(Handoff::unpack(role, phys), Some(h));
    /// assert_eq!(Handoff::unpack(0, phys), None);
    /// assert_eq!(Handoff::unpack(role, phys + 8), None);
    /// ```
    pub fn unpack(arg0: u64, arg1: u64) -> Option<Handoff> {
        if arg0 & 0xffff != ROLE_TAG || arg1 == 0 || !arg1.is_multiple_of(PAGE) {
            return None;
        }
        let b = (arg0 >> 16).to_le_bytes();
        let mac = [b[0], b[1], b[2], b[3], b[4], b[5]];
        if mac == [0; 6] || mac[0] & 1 != 0 {
            return None;
        }
        Some(Handoff {
            mac,
            data_plane_phys: arg1,
        })
    }

    /// Is `arg0` this server's role word at all? `net_stack` dispatches on this.
    pub fn is_role(arg0: u64) -> bool {
        arg0 & 0xffff == ROLE_TAG
    }
}
