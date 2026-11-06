//! **Physical Region Page pointers** (section 4.3, Physical Region Page Entry and List): the two
//! pointer words a data transfer carries.
//!
//! Name: ratified 2026-11-06 (UTC). The module holding [`PhysicalRegionPagePair`] and
//! [`physical_region_page_pair`].

/// **The two Physical Region Page pointers of one transfer**: PRP1 and PRP2, as a command carries
/// them. `second_entry` is zero when one page holds the whole transfer.
///
/// Name: ratified 2026-11-06 (UTC). The pair [`physical_region_page_pair`] returned as an unnamed tuple, now a
/// struct so a caller cannot swap the two entries. The specification's terms stay searchable
/// through the aliases `PRP` and `physical region page`.
#[doc(alias = "PRP")]
#[doc(alias = "physical region page")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRegionPagePair {
    /// PRP1: the transfer's first byte, which may carry an offset into its page.
    ///
    /// Name: ratified 2026-11-06 (UTC).
    pub first_entry: u64,
    /// PRP2: the next page boundary when the transfer spills into a second page, otherwise zero.
    ///
    /// Name: ratified 2026-11-06 (UTC).
    pub second_entry: u64,
}

/// **Physical Region Page pair** (PRP1 and PRP2, section 4.3, Physical Region Page Entry and
/// List): the two PRP words for one data transfer of `length_bytes` at physical
/// `base_physical_address`, against pages of `page_size_bytes`. PRP1 may carry an offset; every
/// later page pointer must not. One page of data needs only PRP1; a transfer spilling into a
/// second page puts that page in PRP2; anything longer needs a PRP *list*, which this crate does
/// not build (its unit is one 4096-byte block), so `None` says "not expressible", and the caller
/// treats it as a bug rather than a device error.
///
/// # Examples
///
/// ```
/// use non_volatile_memory_express::{PhysicalRegionPagePair, physical_region_page_pair};
///
/// // A page-aligned block: one pointer is enough.
/// let aligned = physical_region_page_pair(0x4000_0000, 4096, 4096).unwrap();
/// assert_eq!(aligned, PhysicalRegionPagePair { first_entry: 0x4000_0000, second_entry: 0 });
///
/// // The same block, offset half a page: it spills, so the second entry carries the next page
/// // boundary, with no offset of its own. That "offset-free by construction" is the
/// // specification's rule for PRP2.
/// let straddling = physical_region_page_pair(0x4000_0800, 4096, 4096).unwrap();
/// assert_eq!(straddling.first_entry, 0x4000_0800);
/// assert_eq!(straddling.second_entry, 0x4000_1000);
///
/// // Three pages would need a list. Not expressible, and said so.
/// assert!(physical_region_page_pair(0x4000_0000, 3 * 4096, 4096).is_none());
/// ```
///
/// Name: ratified 2026-11-05 (UTC), on #3266, renamed from `prp_pair` (the acronym spelled out, as
/// the acronym test asks). The specification's terms stay searchable through the aliases
/// `physical region page` and `PRP`.
#[doc(alias = "physical region page")]
#[doc(alias = "PRP")]
pub fn physical_region_page_pair(
    base_physical_address: u64,
    length_bytes: u64,
    page_size_bytes: u64,
) -> Option<PhysicalRegionPagePair> {
    assert!(page_size_bytes.is_power_of_two() && page_size_bytes >= 512);
    if length_bytes == 0 {
        return None;
    }
    let offset = base_physical_address & (page_size_bytes - 1);
    let end = offset.checked_add(length_bytes)?;
    if end <= page_size_bytes {
        Some(PhysicalRegionPagePair {
            first_entry: base_physical_address,
            second_entry: 0,
        })
    } else if end <= 2 * page_size_bytes {
        // The second pointer is the next page boundary after the base, offset-free by
        // construction. Checked: a base in the last page of the address space has no next
        // boundary.
        Some(PhysicalRegionPagePair {
            first_entry: base_physical_address,
            second_entry: (base_physical_address & !(page_size_bytes - 1))
                .checked_add(page_size_bytes)?,
        })
    } else {
        None // would need a PRP list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(first_entry: u64, second_entry: u64) -> Option<PhysicalRegionPagePair> {
        Some(PhysicalRegionPagePair {
            first_entry,
            second_entry,
        })
    }

    #[test]
    fn pairs_cover_one_and_two_pages_and_refuse_more() {
        // A page-aligned single page: PRP1 only.
        assert_eq!(
            physical_region_page_pair(0x5000, 4096, 4096),
            pair(0x5000, 0)
        );
        // An offset transfer spilling into the next page: PRP2 is that page's boundary.
        assert_eq!(
            physical_region_page_pair(0x5800, 4096, 4096),
            pair(0x5800, 0x6000)
        );
        // Exactly two aligned pages: still expressible.
        assert_eq!(
            physical_region_page_pair(0x5000, 8192, 4096),
            pair(0x5000, 0x6000)
        );
        // More needs a PRP list, which this crate does not build.
        assert_eq!(physical_region_page_pair(0x5800, 8192, 4096), None);
        assert_eq!(
            physical_region_page_pair(0x5000, 0, 4096),
            None,
            "empty transfers refused"
        );
    }
}
