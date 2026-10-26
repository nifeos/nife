//! The `e1000e` server's windows, which `net_stack`'s transport reads and the kernel maps, and the
//! spawn words the kernel hands it ([`Handoff`]).
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, after the crate it
//! serves.

use address_space_map::{PAGE, pair_page};

/// The DMA region, `e1000e::layout::PAGES` pages, normal memory. The virtio server's one DMA
/// page lives at the same address; a `net_stack` drives one NIC or the other, never both.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. It was the
/// driver crate's `DATA_PLANE_VA`, renamed to spell out the address as design/naming.md asks.
pub const DATA_PLANE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_0090_0000);
/// BAR0's receive-queue page, device memory.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. Ruled 2026-10-26
/// (UTC), tree-wide: `rx` and `tx` are spelled out. Refused `RX_QUEUE_VA` (an abbreviated
/// address), `RX_QUEUE_VIRTUAL_ADDRESS` (rx abbreviates receive).
pub const RECEIVE_QUEUE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_009c_0000);
/// BAR0's transmit-queue page, device memory.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts. Ruled 2026-10-26
/// (UTC), tree-wide: `rx` and `tx` are spelled out. Refused `TX_QUEUE_VA` (an abbreviated
/// address), `TX_QUEUE_VIRTUAL_ADDRESS` (tx abbreviates transmit).
pub const TRANSMIT_QUEUE_VIRTUAL_ADDRESS: u64 = pair_page(0x0000_0000_009c_1000);

/// The role word's low half when `arg0` is an `e1000e` server handoff. Disjoint from `net_stack`'s
/// existing roles (0 is the virtio server, small integers are socket clients).
///
/// Name: provisional. No ruling covers it.
pub const ROLE_TAG: u64 = 0xe1e0;

/// **What the kernel hands `net_stack` at spawn**, in two of its three argument words (the third
/// stays the listen grant, as for the virtio server).
///
/// [`Handoff::unpack`] refuses what [`Handoff::pack`] did not make.
///
/// # Examples
///
/// ```
/// use driver_process_contract::e1000e::Handoff;
///
/// let h = Handoff { mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x57], data_plane_phys: 0x4012_3000 };
/// let [role, phys] = h.pack();
/// assert_eq!(Handoff::unpack(role, phys), Some(h));
/// // Role 0 is the virtio server and 1..=6 are socket clients; neither unpacks as this.
/// assert_eq!(Handoff::unpack(0, phys), None);
/// assert_eq!(Handoff::unpack(2, phys), None);
/// // A data plane that is not page aligned was not made by the kernel.
/// assert_eq!(Handoff::unpack(role, phys + 8), None);
/// ```
///
/// Name: provisional. No ruling covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// The station address, read by the kernel from receive-address entry 0. The process has no
    /// way to read it itself (page 5 is not mapped) and no way to change it.
    pub mac: [u8; 6],
    /// The DMA region's physical base. Descriptors speak physical addresses (identity-mapped by
    /// the IOMMU domain), so the process cannot compute them from its virtual window.
    pub data_plane_phys: u64,
}

impl Handoff {
    /// `[arg0, arg1]`: the tag and the MAC address in the role word, the base in the second.
    ///
    /// Name: provisional. No ruling covers it.
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
    /// Name: provisional. No ruling covers it.
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

    /// Is `arg0` an `e1000e` server's role word at all? `net_stack` dispatches on this.
    ///
    /// Name: provisional. No ruling covers it.
    pub fn is_role(arg0: u64) -> bool {
        arg0 & 0xffff == ROLE_TAG
    }
}

#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests {
    use super::*;

    const PHYS: u64 = 0x8000_0000;

    #[test]
    fn the_handoff_role_never_collides_with_net_stacks_other_roles() {
        for role in 0..=255u64 {
            assert!(!Handoff::is_role(role));
        }
        let h = Handoff {
            mac: [0x52, 0x54, 0, 0x12, 0x34, 0x56],
            data_plane_phys: PHYS,
        };
        assert!(Handoff::is_role(h.pack()[0]));
    }

    /// The handoff refuses each bad field on its own: a role word with a valid-looking MAC but the
    /// wrong tag, and a group MAC under the right tag.
    #[test]
    fn the_handoff_refuses_a_wrong_tag_and_a_group_address_each_alone() {
        let h = Handoff {
            mac: [0x52, 0x54, 0, 0x12, 0x34, 0x56],
            data_plane_phys: PHYS,
        };
        let [role, phys] = h.pack();
        assert_eq!(Handoff::unpack(role, phys), Some(h));
        assert_eq!(Handoff::unpack(role ^ 1, phys), None, "wrong tag");
        let group = Handoff {
            mac: [0x53, 0x54, 0, 0x12, 0x34, 0x56],
            ..h
        };
        assert_eq!(
            Handoff::unpack(group.pack()[0], phys),
            None,
            "group address"
        );
    }
}
