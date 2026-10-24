//! The ASID allocator: **which number tags this address space's TLB entries.**
//!
//! Milestone 15 (design/roadmap/0015-asids.md). The hardware tags every TLB entry from a
//! non-global mapping with the ASID in `TTBR0_EL1` (or `satp`), so a context switch flushes
//! nothing. Each address space keeps one ASID for life and allocation is a bitmap scan, because
//! `MAX_SPACES` (160) is below the smallest ASID space (256); past 255 the fix is 16-bit ASIDs. A
//! number may be reused only once no core's TLB holds it: the kernel invalidates by ASID in
//! `AddressSpace::drop` before [`free`](Allocator::free), one `tlbi aside1is` on aarch64 and an
//! SBI RFENCE shootdown on RISC-V, whose `satp.ASID` may be narrower than these numbers. See
//! notes/address-space-identifiers.md and notes/riscv-tlb-shootdown.md.

//! Name: ratified 2026-09-18 (calef, `design/decisions/` §154 (the acronym test is whether the
//! phrase is spoken)): "address space identifier" is a phrase people say, so the expansion is the
//! name. Refused `asid`. The hardware fields `satp.ASID` and `TTBR0_EL1.ASID` keep their manuals'
//! spelling wherever the tree names the field rather than the crate (`asid_bits()`, `flush_asid()`).

#![cfg_attr(not(test), no_std)]

/// One more than the largest ASID we hand out. 8-bit ASIDs: every aarch64 implementation has at
/// least these, and 255 usable numbers exceed `MAX_SPACES` (160) with room.
pub const ASIDS: usize = 256;

/// The allocator: one bit per ASID, set = in use. ASID 0 is born allocated and is never freed:
/// it is the kernel's, carried by the reserved "nobody is home" table, so a user address space
/// can never share a tag with the state that means no user at all.
///
/// # Examples
///
/// ```
/// use address_space_identifier::Allocator;
///
/// let mut a = Allocator::new();
/// let first = a.alloc().expect("a fresh allocator has one to give");
/// let second = a.alloc().expect("and another");
/// assert_ne!(first, second, "two live address spaces never share an ASID");
///
/// // Freeing puts it back in circulation.
/// a.free(first);
/// assert!(a.alloc().is_some());
/// ```
pub struct Allocator {
    bitmap: [u64; ASIDS / 64],
}

impl Allocator {
    /// An allocator with only ASID 0 taken, reserved forever for "no user address space". `const`
    /// so the kernel can build it at compile time rather than at boot.
    pub const fn new() -> Self {
        let mut bitmap = [0u64; ASIDS / 64];
        bitmap[0] = 1; // ASID 0: reserved for "no user address space", forever
        Self { bitmap }
    }

    /// Claim a free ASID. `None` only if all 255 are live, which `MAX_SPACES` makes unreachable
    /// in the kernel; the type is honest anyway.
    pub fn alloc(&mut self) -> Option<u16> {
        for (i, word) in self.bitmap.iter_mut().enumerate() {
            if *word != u64::MAX {
                let bit = word.trailing_ones() as usize;
                *word |= 1 << bit;
                return Some((i * 64 + bit) as u16);
            }
        }
        None
    }

    /// Return an ASID. The caller has already invalidated the TLB by this ASID on every core
    /// (see the crate doc); after this, the number may tag a different address space.
    pub fn free(&mut self, asid: u16) {
        debug_assert!(asid != 0, "ASID 0 is the kernel's and is never freed");
        debug_assert!((asid as usize) < ASIDS, "ASID out of range");
        if asid != 0 && (asid as usize) < ASIDS {
            self.bitmap[asid as usize / 64] &= !(1 << (asid as usize % 64));
        }
    }
}

impl Default for Allocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Machine-checked proofs (DECISIONS §14). notes/verification.md predicted "the ASID allocator
/// when it lands" as a frontier crate; this is it landing. The properties are the ones the
/// privilege boundary rests on: no user space ever tags its entries 0 (the kernel's), and no
/// two live spaces ever share a tag (sharing one would let the TLB serve one process the
/// other's memory).
#[cfg(kani)]
mod verification {
    use super::*;

    /// **Zero is never handed out**, from any reachable allocator state. State is modeled
    /// symbolically: any bitmap with bit 0 set, which is every state `new` + any operation
    /// sequence can produce, since `alloc` only sets bits and `free` refuses ASID 0.
    /// Falsification: replayable `crates/address_space_identifier/falsifications/verification.the_kernel_asid_is_never_allocated.patch`
    #[kani::proof]
    fn the_kernel_asid_is_never_allocated() {
        let mut a = Allocator {
            bitmap: [kani::any(), kani::any(), kani::any(), kani::any()],
        };
        a.bitmap[0] |= 1; // the invariant free() preserves: bit 0 stays set
        if let Some(asid) = a.alloc() {
            assert_ne!(asid, 0);
            assert!((asid as usize) < ASIDS);
        }
    }

    /// **Two allocations never alias.** From any state, two back-to-back allocs return distinct
    /// numbers: alloc claims the bit before returning, so the second scan cannot find it.
    /// Falsification: replayable `crates/address_space_identifier/falsifications/verification.two_live_asids_are_distinct.patch`
    #[kani::proof]
    fn two_live_asids_are_distinct() {
        let mut a = Allocator {
            bitmap: [kani::any(), kani::any(), kani::any(), kani::any()],
        };
        if let (Some(x), Some(y)) = (a.alloc(), a.alloc()) {
            assert_ne!(x, y);
        }
    }

    /// **Free really frees, and only its own bit.** From any state with `asid` live, freeing it
    /// makes it allocatable again and leaves every other bit exactly as it was.
    /// Falsification: replayable `crates/address_space_identifier/falsifications/verification.free_releases_exactly_its_own_asid.patch`
    #[kani::proof]
    fn free_releases_exactly_its_own_asid() {
        let mut a = Allocator {
            bitmap: [kani::any(), kani::any(), kani::any(), kani::any()],
        };
        let asid: u16 = kani::any();
        kani::assume(asid != 0 && (asid as usize) < ASIDS);

        let before = a.bitmap;
        a.free(asid);

        let (word, bit) = (asid as usize / 64, asid as usize % 64);
        assert_eq!(a.bitmap[word] & !(1 << bit), before[word] & !(1 << bit));
        assert_eq!(a.bitmap[word] & (1 << bit), 0);
        for (i, (now, was)) in a.bitmap.iter().zip(before.iter()).enumerate() {
            if i != word {
                assert_eq!(now, was);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first user address space does not get the kernel's tag.
    #[test]
    fn the_first_asid_is_one_not_zero() {
        let mut a = Allocator::new();
        assert_eq!(a.alloc(), Some(1));
    }

    /// All 255 usable ASIDs allocate, distinctly; the 256th request is refused.
    #[test]
    fn every_usable_asid_allocates_once_then_none() {
        let mut a = Allocator::new();
        let mut seen = [false; ASIDS];
        for _ in 0..ASIDS - 1 {
            let asid = a.alloc().expect("ran out early") as usize;
            assert!(!seen[asid], "ASID {asid} handed out twice");
            assert_ne!(asid, 0);
            seen[asid] = true;
        }
        assert_eq!(a.alloc(), None, "a 256th ASID appeared from nowhere");
    }

    /// Freed numbers come back; the kernel's never does.
    #[test]
    fn free_then_alloc_reuses() {
        let mut a = Allocator::new();
        let x = a.alloc().unwrap();
        let y = a.alloc().unwrap();
        a.free(x);
        assert_eq!(a.alloc(), Some(x), "the freed ASID was not reused");
        a.free(y);
        assert_eq!(a.alloc(), Some(y));
    }
}
