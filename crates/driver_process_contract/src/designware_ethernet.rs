//! The JH7110 Ethernet server's windows, which `net_stack`'s transport reads and the kernel maps,
//! and the spawn words the kernel hands it ([`Handoff`]).
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, after the crate it
//! serves.

use address_space_map::{PAGE, pair_page};

/// The DMA region, `designware_ethernet::layout::PAGES` pages, normal memory. The same address as
/// the virtio and `e1000e` servers' DMA windows; a `net_stack` drives one NIC, never two.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. It was the
/// driver crate's `DATA_PLANE_VA`, renamed to spell out the address as design/naming.md asks.
pub const DATA_PLANE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_0090_0000);
/// The controller's DMA (direct memory access) register page
/// (`designware_ethernet::registers::DIRECT_MEMORY_ACCESS_REGISTER_PAGE`), device memory.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `dma` spelled out as `direct_memory_access`; this
/// lane added `REGISTER` to match the crate's `DIRECT_MEMORY_ACCESS_REGISTER_PAGE`, replacing
/// `DIRECT_MEMORY_ACCESS_PAGE_VIRTUAL_ADDRESS`.
#[doc(alias = "dma_page")]
pub const DIRECT_MEMORY_ACCESS_REGISTER_PAGE_VIRTUAL_ADDRESS: u64 =
    pair_page(0x0000_0000_009c_0000);

/// The role word's low half when `arg0` is this server's handoff. Disjoint from `net_stack`'s
/// existing roles: 0 is the virtio server, small integers are socket clients, `0xe1e0` is
/// `e1000e`'s.
///
/// Name: provisional. No ruling covers it.
pub const ROLE_TAG: u64 = 0xd3e0;

/// **What the kernel hands `net_stack` at spawn**, in two of its three argument words.
///
/// Name: provisional. No ruling covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// The station address the kernel programmed. The process cannot read or change it: the
    /// MAC's page is not mapped.
    #[doc(alias = "mac_address")]
    pub station_address: [u8; 6],
    /// The DMA region's physical base. Descriptors speak physical addresses (there is no IOMMU
    /// on the JH7110), so the process cannot compute them from its virtual window.
    pub data_plane_physical_address: u64,
}

impl Handoff {
    /// `[arg0, arg1]`: the tag and the MAC address in the role word, the base in the second.
    ///
    /// Name: provisional. No ruling covers it.
    pub fn pack(&self) -> [u64; 2] {
        let mut station_address = [0u8; 8];
        station_address[..6].copy_from_slice(&self.station_address);
        [
            ROLE_TAG | u64::from_le_bytes(station_address) << 16,
            self.data_plane_physical_address,
        ]
    }

    /// The inverse of [`Handoff::pack`], refusing a role word without the tag, a base that is
    /// zero or not page aligned, and a MAC address no station can own.
    ///
    /// # Examples
    ///
    /// ```
    /// use driver_process_contract::designware_ethernet::Handoff;
    ///
    /// let h = Handoff {
    ///     station_address: [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c],
    ///     data_plane_physical_address: 0x4012_3000,
    /// };
    /// let [role, phys] = h.pack();
    /// assert_eq!(Handoff::unpack(role, phys), Some(h));
    /// assert_eq!(Handoff::unpack(0, phys), None);
    /// assert_eq!(Handoff::unpack(role, phys + 8), None);
    /// ```
    ///
    /// Name: provisional. No ruling covers it.
    pub fn unpack(arg0: u64, arg1: u64) -> Option<Handoff> {
        if arg0 & 0xffff != ROLE_TAG || arg1 == 0 || !arg1.is_multiple_of(PAGE) {
            return None;
        }
        let b = (arg0 >> 16).to_le_bytes();
        let station_address = [b[0], b[1], b[2], b[3], b[4], b[5]];
        if station_address == [0; 6] || station_address[0] & 1 != 0 {
            return None;
        }
        Some(Handoff {
            station_address,
            data_plane_physical_address: arg1,
        })
    }

    /// Is `arg0` this server's role word at all? `net_stack` dispatches on this.
    ///
    /// Name: provisional. No ruling covers it.
    pub fn is_role(arg0: u64) -> bool {
        arg0 & 0xffff == ROLE_TAG
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handoff_refuses_a_multicast_or_absent_address() {
        let h = Handoff {
            station_address: [0x01, 0, 0, 0, 0, 1],
            data_plane_physical_address: 0x4000_0000,
        };
        let [r, p] = h.pack();
        assert_eq!(Handoff::unpack(r, p), None);
        let [r, p] = Handoff {
            station_address: [0; 6],
            ..h
        }
        .pack();
        assert_eq!(Handoff::unpack(r, p), None);
        assert!(!Handoff::is_role(0xe1e0));
    }

    /// A handoff survives its own packing, and each refusal holds on its own: a role word that is
    /// not this server's is refused with a good base beside it, as is a good role word with no
    /// base or a base off a page boundary.
    #[test]
    fn the_handoff_round_trips_and_each_refusal_stands_alone() {
        let h = Handoff {
            station_address: [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c],
            data_plane_physical_address: 0x4800_0000,
        };
        let [r, p] = h.pack();
        assert!(Handoff::is_role(r));
        assert_eq!(Handoff::unpack(r, p), Some(h));
        assert_eq!(Handoff::unpack(r ^ 1, p), None, "not this server's role");
        assert_eq!(Handoff::unpack(r, 0), None, "no base");
        assert_eq!(Handoff::unpack(r, p + 8), None, "not page aligned");
    }
}
