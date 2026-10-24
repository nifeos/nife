//! The DMA-confinement validator, as pure logic: [`validate_and_shadow`], lifted out of
//! `kernel/src/virtio.rs` so Kani proves the code the kernel calls (notes/verification.md). A
//! userspace virtio driver programs the descriptors a device does DMA against, and the device has
//! no MMU in front of it (or an IOMMU, and this is defense in depth), so something trusted checks
//! every address the device will touch. Proved for every input: no descriptor the device can read
//! out of the shadow ring references memory outside `[base, base + size)`, in either direction,
//! with indirect descriptors refused, over any newly published batch. The device reads a
//! kernel-private shadow copy, so a driver mutating a descriptor after validation changes only a
//! copy nothing reads. See notes/dma.md.

//! # BUGS
//! - The shadow descriptor is published in two stores; a device can see the new `addr` with the
//!   old `len` (`notes/confinement-claims.md` row 17;
//!   design/roadmap/0799-the-shadow-descriptor-is-published-in-two-stores.md).
//!
//! Name: ratified 2026-09-24 (calef, #1229), replacing `dma_validator`, under §154 (the acronym test
//! is whether the phrase is spoken). Refused `dma_validator` (an unexpanded acronym),
//! `direct_memory_access_confinement` (names the property and drops the noun `validator`),
//! `descriptor_validator` (drops DMA, the crate's reason to exist) and `dma_validate` (a verb).

#![no_std]

/// Bit 0: this descriptor chains to another (`next` names it). The validator follows the chain,
/// bounded by the queue size, so a `next` cycle cannot spin forever.
pub const F_NEXT: u16 = 1;
/// Bit 2: this descriptor points at a **table of further descriptors** instead of a buffer. The
/// validator walks the flat chain and never follows that inner table, so a descriptor carrying this
/// flag would send the device to addresses that were never checked. It is refused here (and the
/// feature that enables it is negotiated off in the kernel), so the confinement fails closed.
pub const F_INDIRECT: u16 = 4;

/// Is `[addr, addr + len)` wholly inside `[base, base + size)`? The whole confinement, as one
/// predicate, and direction-agnostic on purpose: whether the device reads the buffer (transmit) or
/// writes it (receive), the address must be in-region, so one check closes both hazards. Total: a
/// length that overflows `addr`, or a region whose own `base + size` wraps `u64`, returns `false`
/// (refuse) rather than panicking, and a wrapping region can only over-reject, the safe direction.
/// See the `in_region_is_sound` proof and notes/dma/validator.md.
pub fn is_in_region(base: u64, size: u64, addr: u64, len: u64) -> bool {
    match (addr.checked_add(len), base.checked_add(size)) {
        (Some(end), Some(limit)) => addr >= base && end <= limit,
        _ => false,
    }
}

/// One virtqueue split-ring descriptor as it sits in memory: `{ u64 addr @0; u32 len @8; u16 flags
/// @12; u16 next @14 }`. The kernel reads the low 8 bytes as `addr` and the high 8 as one `word`
/// carrying len/flags/next, and copies both verbatim into the shadow, so this models the same two
/// 64-bit words and decodes the fields the same way.
///
/// # Examples
///
/// The refusals that make the check total: indirect descriptors, out-of-ring links, and
/// arithmetic that would overflow, which can only over-reject:
///
/// ```
/// use direct_memory_access_validator::{Desc, F_INDIRECT, F_NEXT, is_descriptor_allowed, is_in_region};
///
/// let (base, size, qsize) = (0x4000_0000u64, 0x1_0000u64, 8u16);
///
/// // An **indirect** descriptor names a table of further descriptors. The table is never copied
/// // into the shadow, so the device would follow the driver's own memory unchecked. Refused
/// // outright rather than validated, whatever it points at.
/// let indirect = Desc { addr: base, word: 16 | ((F_INDIRECT as u64) << 32) };
/// assert!(indirect.is_indirect());
/// assert!(!is_descriptor_allowed(base, size, qsize, indirect));
///
/// // A `next` link outside the ring would walk the chain off the end of the queue.
/// let bad_link = Desc { addr: base, word: 16 | ((F_NEXT as u64) << 32) | ((qsize as u64) << 48) };
/// assert!(bad_link.has_next() && bad_link.next() == qsize);
/// assert!(!is_descriptor_allowed(base, size, qsize, bad_link));
///
/// // And arithmetic that would overflow answers "refuse" rather than panicking. A wrapping region
/// // can only ever over-reject, which is the safe direction.
/// assert!(!is_in_region(base, size, u64::MAX - 4, 1500));
/// assert!(!is_in_region(u64::MAX - 4, 1500, base, 16));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Desc {
    /// The buffer's guest-physical address, the first 64-bit word.
    pub addr: u64,
    /// Length, flags and next-index packed into the second 64-bit word; decode with
    /// [`buf_len`](Self::buf_len), [`flags`](Self::flags) and [`next`](Self::next).
    pub word: u64,
}

impl Desc {
    /// The buffer length, the low 32 bits of the second word.
    pub const fn buf_len(self) -> u64 {
        self.word & 0xffff_ffff
    }
    /// The descriptor flags, bits 32..48 of the second word.
    pub const fn flags(self) -> u16 {
        ((self.word >> 32) & 0xffff) as u16
    }
    /// The next-descriptor index, bits 48..64 of the second word.
    pub const fn next(self) -> u16 {
        ((self.word >> 48) & 0xffff) as u16
    }
    /// Whether `F_INDIRECT` is set: the buffer holds a table of further descriptors rather than
    /// data, which this validator refuses to chase (see the confinement decision above).
    pub const fn is_indirect(self) -> bool {
        self.flags() & F_INDIRECT != 0
    }
    /// Whether `F_NEXT` is set: [`next`](Self::next) names another descriptor in the chain.
    pub const fn has_next(self) -> bool {
        self.flags() & F_NEXT != 0
    }
}

/// **The per-descriptor decision: may the device be allowed to act on this descriptor?** True only
/// when it is not indirect, its buffer stays inside `[base, base + size)`, and any `next` link it
/// declares names a real slot (`< qsize`). Every descriptor copied into the shadow passes this
/// first, so it is the property the whole confinement rests on, isolated as a loopless function the
/// `an_accepted_descriptor_is_confined` proof can quantify over all inputs.
///
/// Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-free-crates, as the
/// yes-or-no worklist recommended. Refused `check_descriptor` (`check_` reads as an action, and
/// the function only answers).
///
/// # Examples
///
/// Every address the device will touch is inside the window, in either direction:
///
/// ```
/// use direct_memory_access_validator::{Desc, is_descriptor_allowed, is_in_region};
///
/// // One 64 KiB region, granted to a userspace virtio driver.
/// let (base, size) = (0x4000_0000u64, 0x1_0000u64);
/// let qsize = 8u16;
///
/// // A descriptor pointing at a 1500-byte frame inside the region. Fine in either direction: the
/// // device may read it (transmit) or write it (receive).
/// let ok = Desc { addr: base + 0x1000, word: 1500 };
/// assert!(is_descriptor_allowed(base, size, qsize, ok));
///
/// // The same length, aimed at the kernel. Refused, and this is the whole point of the crate: a
/// // receive buffer here would let the device overwrite the kernel, and a transmit buffer would
/// // let it put the kernel on the wire.
/// let escape = Desc { addr: 0xffff_0000_0000_0000, word: 1500 };
/// assert!(!is_descriptor_allowed(base, size, qsize, escape));
///
/// // A buffer that starts inside and runs off the end is refused too. One predicate closes both,
/// // which is why there is only one.
/// let overrun = Desc { addr: base + size - 8, word: 1500 };
/// assert!(!is_descriptor_allowed(base, size, qsize, overrun));
/// assert!(!is_in_region(base, size, base + size - 8, 1500));
/// ```
pub fn is_descriptor_allowed(base: u64, size: u64, qsize: u16, d: Desc) -> bool {
    if d.is_indirect() {
        return false; // an indirect table is never copied, so the device would follow it unchecked
    }
    if !is_in_region(base, size, d.addr, d.buf_len()) {
        return false; // the buffer escapes the driver's region
    }
    if d.has_next() && d.next() >= qsize {
        return false; // a chain link out of the descriptor table
    }
    true
}

/// Validate the driver's newly published descriptors and copy the validated ones into the shadow
/// ring the device reads: the security-critical function. For each head in `from_idx .. to_idx` it
/// walks the chain in the driver's table, checks every descriptor with [`is_descriptor_allowed`],
/// copies it into the shadow table at the same index, mirrors the head into the shadow available
/// ring, and publishes the shadow's `avail.idx` last. The device reads only the shadow, which the
/// driver cannot write, so it acts on exactly the bytes validated here; that closes the
/// time-of-check/time-of-use race. Returns `false`, leaving the published index untouched, if any
/// descriptor escapes the region or is indirect, or the batch claims more entries than the ring
/// holds. Memory is reached through the caller's read and write closures (the direct map, arrays,
/// or Kani's symbolic memory), and the walks are bounded by `qsize`, the negotiated queue size.
#[allow(clippy::too_many_arguments)]
pub fn validate_and_shadow(
    direct_memory_access_base: u64,
    direct_memory_access_size: u64,
    driver_desc: u64,
    driver_avail: u64,
    shadow_desc: u64,
    shadow_avail: u64,
    from_idx: u16,
    to_idx: u16,
    qsize: u16,
    read16: &dyn Fn(u64) -> u16,
    read64: &dyn Fn(u64) -> u64,
    write16: &dyn Fn(u64, u16),
    write64: &dyn Fn(u64, u64),
) -> bool {
    // At most `qsize` descriptors can be newly available since the last validation: the available
    // ring has only `qsize` slots, so a driver cannot have published more than that without the
    // device consuming some. A larger jump in avail.idx is malformed or hostile, and walking it would
    // spin this loop up to 65535 times under the caller's lock with interrupts masked. Refuse it
    // before touching a single descriptor. (wrapping_sub because avail.idx wraps at u16.)
    if to_idx.wrapping_sub(from_idx) > qsize {
        return false;
    }

    // avail: { u16 flags; u16 idx; u16 ring[qsize]; }
    let mut idx = from_idx;
    while idx != to_idx {
        let slot = (idx % qsize) as u64;
        let head = read16(driver_avail + 4 + slot * 2);
        if head >= qsize {
            return false; // head index out of the descriptor table
        }

        // Walk this head's chain, validate every descriptor, and copy the validated ones into the
        // shadow. The confinement theorem is proved against `shadow_one_head` directly, so refuse the
        // whole batch if it refuses this head.
        if !shadow_one_head(
            direct_memory_access_base,
            direct_memory_access_size,
            driver_desc,
            shadow_desc,
            head,
            qsize,
            read64,
            write64,
        ) {
            return false;
        }

        // Mirror the head into the shadow available ring.
        write16(shadow_avail + 4 + slot * 2, head);
        idx = idx.wrapping_add(1);
    }

    // Publish LAST: the device reads the shadow's avail.idx to learn what is ready, so it must not
    // advance until every descriptor it points at is already in the shadow.
    write16(shadow_avail + 2, to_idx);
    true
}

/// Validate and shadow one head's descriptor chain: the core of the confinement. Every descriptor
/// from `head` is checked with [`is_descriptor_allowed`] before it is copied verbatim into the
/// shadow table at the same index, so no descriptor the device can read out of the shadow is
/// out-of-region or indirect (the `validate_and_shadow_confines_every_chain` proof). The walk is
/// bounded by `qsize`, so a `next` cycle cannot loop forever. Returns `false` if a buffer escapes
/// the region, a descriptor is indirect, or a `next` is out of range, perhaps after copying earlier
/// validated descriptors, which is harmless: the caller does not publish the head.
///
/// BUGS: the two `write64`s below are not one publish (the crate's BUGS), and the walk overwrites
/// a slot the device may still own, because nothing here knows which shadow slots are idle.
#[allow(clippy::too_many_arguments)]
fn shadow_one_head(
    direct_memory_access_base: u64,
    direct_memory_access_size: u64,
    driver_desc: u64,
    shadow_desc: u64,
    head: u16,
    qsize: u16,
    read64: &dyn Fn(u64) -> u64,
    write64: &dyn Fn(u64, u64),
) -> bool {
    let mut d = head;
    for _ in 0..qsize {
        let src = driver_desc + d as u64 * 16;
        let desc = Desc {
            addr: read64(src),
            word: read64(src + 8),
        };

        if !is_descriptor_allowed(
            direct_memory_access_base,
            direct_memory_access_size,
            qsize,
            desc,
        ) {
            return false;
        }

        // Copy the validated descriptor into the shadow, byte-for-byte. From here the device reads
        // this, not the driver's copy.
        let dst = shadow_desc + d as u64 * 16;
        write64(dst, desc.addr);
        write64(dst + 8, desc.word);

        if !desc.has_next() {
            break;
        }
        d = desc.next();
    }
    true
}

// ---------------------------------------------------------------------------------------------
// The queue ring layout, owned here and used by the kernel (kernel/src/virtio.rs defines its
// `QSIZE`, `RING_BLOCK`, `MAX_QUEUES` and the three ring offsets as aliases of these), because a
// proof about a copy of the layout proves nothing about the layout that runs. The kernel keeps its
// own compile-time asserts as well, so drift is a compile error in two places.
// ---------------------------------------------------------------------------------------------

/// The negotiated queue size the kernel drives (`QSIZE` in kernel/src/virtio.rs).
pub const LAYOUT_QSIZE: u16 = 8;
/// Offsets of a queue's three rings, relative to its ring block.
pub const DESC_OFF: u64 = 0x000;
/// Offset of the avail ring: past the descriptor table, `6 + 2*QSIZE` bytes in.
pub const AVAIL_OFF: u64 = 0x080; // 6 + 2*QSIZE
/// Offset of the used ring: past the avail ring, `6 + 8*QSIZE` bytes in.
pub const USED_OFF: u64 = 0x100; // 6 + 8*QSIZE
/// The whole ring area of one queue must fit under this; a queue's data buffers live above it.
pub const RING_END: u64 = USED_OFF + 6 + 8 * LAYOUT_QSIZE as u64;
/// The stride between successive queues' ring areas, in both the driver's region and the shadow.
pub const RING_BLOCK: u64 = 0x200;
/// The most virtqueues the confinement drives per device (RX = queue 0, TX = queue 1 on a NIC).
pub const MAX_QUEUES: u64 = 2;

/// Queue `q`'s ring block base, relative to a region (driver DMA or shadow) base. Queue `q`'s
/// descriptor table, available ring, and used ring sit at `queue_block(q) + {DESC,AVAIL,USED}_OFF`,
/// so distinct queues validate on disjoint blocks and never read or write each other's rings.
pub const fn queue_block(q: u16) -> u64 {
    q as u64 * RING_BLOCK
}

// =============================================================================================
// Machine-checked proofs (Kani). Behind `#[cfg(kani)]`, so an ordinary build/test never sees them;
// only `cargo kani` (script/verify) sets that cfg. See notes/verification.md.
// =============================================================================================
#[cfg(kani)]
mod verification;

// =============================================================================================
// Host tests. Fast (`cargo test`), and a redundant check that the extracted logic matches the
// attacker cases the kernel's QEMU suite exercises against the same function.
// =============================================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const QSIZE: u16 = 8;
    const F_WRITE: u16 = 2; // device-writable (an RX buffer): the validator bounds it the same way

    /// A pair of ring regions in ordinary host memory, addressed like the kernel's physical layout.
    struct Rings {
        driver_desc: [Desc; 8],
        driver_ring: [u16; 8],
        driver_idx: u16,
        shadow_desc: core::cell::RefCell<[Desc; 8]>,
        shadow_ring: core::cell::RefCell<[u16; 8]>,
        shadow_idx: core::cell::RefCell<u16>,
    }

    const DD: u64 = 0x1000;
    const DA: u64 = 0x2000;
    const SD: u64 = 0x3000;
    const SA: u64 = 0x4000;

    impl Rings {
        fn new() -> Self {
            Rings {
                driver_desc: [Desc { addr: 0, word: 0 }; 8],
                driver_ring: [0; 8],
                driver_idx: 0,
                shadow_desc: core::cell::RefCell::new([Desc { addr: 0, word: 0 }; 8]),
                shadow_ring: core::cell::RefCell::new([0; 8]),
                shadow_idx: core::cell::RefCell::new(0),
            }
        }
        fn set_desc(&mut self, i: usize, addr: u64, len: u32, flags: u16, next: u16) {
            self.driver_desc[i] = Desc {
                addr,
                word: (len as u64) | ((flags as u64) << 32) | ((next as u64) << 48),
            };
        }
        fn run(&self) -> bool {
            self.run_range(0, self.driver_idx)
        }
        /// The same run with explicit batch indices, for the tests that need a slot other than
        /// 0..2: `avail.idx` is a free-running u16, so real batches start at arbitrary indices and
        /// the slot arithmetic (`idx % qsize`, `4 + slot * 2`) only shows its shape there.
        fn run_range(&self, from: u16, to: u16) -> bool {
            validate_and_shadow(
                0x1_0000_0000, // direct_memory_access_base: an arbitrary in-test region
                0x1000,        // direct_memory_access_size
                DD,
                DA,
                SD,
                SA,
                from,
                to,
                QSIZE,
                &|p| {
                    if p == DA + 2 {
                        self.driver_idx
                    } else {
                        self.driver_ring[((p - DA - 4) / 2) as usize]
                    }
                },
                &|p| {
                    let off = p - DD;
                    let d = self.driver_desc[(off / 16) as usize];
                    if off.is_multiple_of(16) {
                        d.addr
                    } else {
                        d.word
                    }
                },
                &|p, v| {
                    let off = p - SA;
                    if off == 2 {
                        *self.shadow_idx.borrow_mut() = v;
                    } else {
                        self.shadow_ring.borrow_mut()[((off - 4) / 2) as usize] = v;
                    }
                },
                &|p, v| {
                    let off = p - SD;
                    let d = (off / 16) as usize;
                    if off.is_multiple_of(16) {
                        self.shadow_desc.borrow_mut()[d].addr = v;
                    } else {
                        self.shadow_desc.borrow_mut()[d].word = v;
                    }
                },
            )
        }
    }

    const BASE: u64 = 0x1_0000_0000;

    #[test]
    fn a_chain_wholly_inside_the_region_validates() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 16, F_NEXT, 1);
        r.set_desc(1, BASE + 0x400, 512, F_NEXT, 2);
        r.set_desc(2, BASE + 0x600, 1, 0, 0);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(r.run(), "an in-region chain was refused");
        assert_eq!(r.shadow_desc.borrow()[0].addr, BASE + 0x200);
        assert_eq!(
            *r.shadow_idx.borrow(),
            1,
            "the shadow avail.idx was not published"
        );
    }

    #[test]
    fn a_descriptor_pointing_outside_the_region_is_refused() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 16, F_NEXT, 1);
        r.set_desc(1, 0xffff_0000_4008_0000, 512, F_NEXT | F_WRITE, 2); // kernel memory
        r.set_desc(2, BASE + 0x600, 1, 0, 0);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(
            !r.run(),
            "a descriptor aimed at kernel memory was not refused"
        );
    }

    #[test]
    fn a_length_that_overflows_the_region_is_refused() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x1000 - 256, 512, 0, 0); // runs 256 bytes past the region end
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(
            !r.run(),
            "a descriptor running past the region end was not refused"
        );
    }

    #[test]
    fn an_indirect_descriptor_is_refused_even_in_region() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 128, F_INDIRECT, 0); // legal address, but indirect
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(!r.run(), "an indirect descriptor was accepted");
    }

    #[test]
    fn a_device_writable_rx_buffer_is_bounded_the_same_as_a_tx_buffer() {
        // An in-region receive buffer validates; the same buffer aimed at the kernel is refused.
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x400, 1514, F_WRITE, 0);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(r.run(), "an in-region device-writable buffer was refused");

        r.set_desc(0, 0xffff_0000_4008_0000, 1514, F_WRITE, 0);
        assert!(
            !r.run(),
            "a device-writable buffer aimed at the kernel was not refused"
        );
    }

    #[test]
    fn a_batch_larger_than_the_ring_is_refused() {
        let mut r = Rings::new();
        for i in 0..8u64 {
            r.set_desc(i as usize, BASE + 0x200 + i * 8, 8, 0, 0);
            r.driver_ring[i as usize] = i as u16;
        }
        r.driver_idx = QSIZE; // exactly a full ring is legal
        assert!(r.run(), "a full ring of valid entries was refused");
        r.driver_idx = QSIZE + 1; // one more than the ring can hold
        assert!(!r.run(), "an oversized batch was walked instead of refused");
    }

    #[test]
    fn a_next_cycle_terminates() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 16, F_NEXT, 1);
        r.set_desc(1, BASE + 0x400, 16, F_NEXT, 0); // 1 -> 0 -> 1 -> ...
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(
            r.run(),
            "a bounded cyclic chain with valid addresses should validate and terminate"
        );
    }

    /// **The walk follows the descriptor's own `next` field, not a fixed successor.** Chain 0 -> 2
    /// with descriptor 1 poisoned (aimed at kernel memory): a walk that misdecodes `next` and steps
    /// to slot 1 refuses the batch, and one that never steps lands nothing at slot 2. The existing
    /// chain test used 0 -> 1 -> 2, where "always step to 1" still validates.
    #[test]
    fn the_chain_walk_lands_on_the_declared_next_slot() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 16, F_NEXT, 2); // chains to 2, skipping 1
        r.set_desc(1, 0xffff_0000_4008_0000, 16, 0, 0); // poison: reached only by a wrong step
        r.set_desc(2, BASE + 0x400, 32, 0, 0);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(r.run(), "a 0 -> 2 chain was refused");
        assert_eq!(
            r.shadow_desc.borrow()[2].addr,
            BASE + 0x400,
            "the chain did not shadow the slot its next field names"
        );
    }

    /// **A clear `F_NEXT` means the `next` field is dead, whatever else the flags say.** The
    /// descriptor is device-writable (bit 1 set, so the flag word is nonzero) with `F_NEXT` (bit 0)
    /// clear and a garbage `next` of 15 in an 8-slot queue. A correct validator ignores both; one
    /// that reads the chain bit with `|` instead of `&`, or hardcodes "has next", trips the
    /// `next >= qsize` refusal on a valid descriptor.
    #[test]
    fn a_clear_next_flag_makes_the_next_field_dead() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 64, F_WRITE, 15);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(
            r.run(),
            "a valid unchained descriptor was refused for its dead next field"
        );
    }

    /// **The available-ring slot arithmetic, pinned at an index where the operators diverge.**
    /// `avail.idx` free-runs, so a batch starting at 11 uses slot 11 % 8 = 3, and the head is read
    /// at `avail + 4 + 3*2` = avail + 10 (mirrored at the same offset in the shadow). The
    /// wrong-operator addresses all differ there: 11 / 8 = 1 lands on ring[1], 4 + (3+2) = 9 maps
    /// to ring[2], 4 + 3/2 = 5 maps to ring[0]. Indices 0..2, which every other test uses, cannot
    /// tell these apart (0*2 = 0+2 is off by the same slot, 2*2 = 2+2 exactly). Every slot except 3
    /// holds head 9, out of the 8-entry table, so any misdirected read refuses the batch; the
    /// mirror is checked by where head 5 lands in the shadow ring.
    #[test]
    fn the_slot_arithmetic_holds_at_a_free_running_index() {
        let mut r = Rings::new();
        r.set_desc(5, BASE + 0x200, 16, 0, 0);
        r.driver_ring = [9; 8]; // every slot but 3 is an out-of-table head
        r.driver_ring[3] = 5;
        assert!(
            r.run_range(11, 12),
            "the head at slot 3 was not read from slot 3"
        );
        assert_eq!(
            r.shadow_ring.borrow()[3],
            5,
            "the head was not mirrored at slot 3 of the shadow ring"
        );
        assert_eq!(r.shadow_ring.borrow()[2], 0, "the mirror strayed to slot 2");
        assert_eq!(r.shadow_ring.borrow()[0], 0, "the mirror strayed to slot 0");
        assert_eq!(
            *r.shadow_idx.borrow(),
            12,
            "the shadow avail.idx was not published"
        );
    }

    /// **`RING_END` is the value its formula documents: 0x100 + 6 + 8*8 = 0x146.** The used ring is
    /// 2 (flags) + 2 (idx) + 8 bytes per element times `LAYOUT_QSIZE` = 8 elements + 2 (avail event).
    /// Nothing else pins the value: every relation the proofs and the compile-time asserts state
    /// (`RING_END <= RING_BLOCK`, disjoint blocks) still holds for the smaller wrong values an
    /// operator slip produces, and the kernel aliases this constant, so a silent shrink would
    /// under-reserve the ring area a real device writes into.
    #[test]
    fn ring_end_is_the_documented_value() {
        assert_eq!(RING_END, 0x146);
    }

    #[test]
    fn in_region_rejects_overflow() {
        assert!(!is_in_region(0, 0x1000, u64::MAX, 1)); // addr + len wraps
        assert!(is_in_region(0x1000, 0x1000, 0x1000, 0x1000)); // exact fit
        assert!(!is_in_region(0x1000, 0x1000, 0x1000, 0x1001)); // one past
    }

    #[test]
    fn ring_blocks_are_disjoint() {
        // Compile-time, because the relations are over constants: a queue's ring area fits inside its
        // block, and queue 0's area ends before queue 1's block begins (so the two never overlap).
        const { assert!(RING_END <= RING_BLOCK) };
        const { assert!(queue_block(0) + RING_END <= queue_block(1)) };
    }
    // The eighth outsider pass's host attacks (claims 13 to 17): the shapes around the cited
    // tests, host-run the way every test in this module is. Name: provisional (this pass's lane,
    // 2026-10-13 UTC).

    /// **An address-plus-length that wraps the u64 is refused, and so is one that straddles the
    /// region's end by a single byte** (claim 13).
    #[test]
    fn a_length_that_wraps_or_straddles_the_end_is_refused() {
        assert!(
            !is_in_region(BASE, 0x1000, u64::MAX - 8, 16),
            "a length whose sum wraps the u64 was treated as inside",
        );
        assert!(
            !is_in_region(0, u64::MAX, 1, u64::MAX),
            "a buffer starting one page into the widest region wrapped and claimed to fit",
        );
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0xfff, 2, 0, 0);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(
            !r.run(),
            "a buffer straddling the region's end by one byte validated",
        );
        assert_eq!(
            *r.shadow_idx.borrow(),
            0,
            "a refused descriptor still published a shadow index",
        );
    }

    /// **An indirect descriptor refuses the batch wherever it sits in the chain** (claim 14).
    #[test]
    fn an_indirect_descriptor_mid_chain_refuses_the_whole_batch() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 16, F_NEXT, 1);
        r.set_desc(1, BASE + 0x400, 16, F_INDIRECT, 0);
        r.set_desc(2, BASE + 0x600, 16, F_NEXT, 3);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(
            !r.run(),
            "a chain carrying an indirect descriptor in its middle validated",
        );
        assert_eq!(
            *r.shadow_idx.borrow(),
            0,
            "a batch refused at its second descriptor still published a shadow index",
        );
    }

    /// **A `next` at or past the ring's size refuses the batch** (claim 15). A two-slot cycle, by
    /// contrast, terminates and validates by design (the cited `a_next_cycle_terminates`): the
    /// walk is bounded by the ring, so a cycle cannot make it walk forever, and this pass adds
    /// only the edge the cited tests leave, a `next` one past the last slot.
    #[test]
    fn an_out_of_ring_next_refuses_the_batch() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 16, F_NEXT, QSIZE);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(
            !r.run(),
            "a next equal to the ring's size walked outside the rings",
        );

        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 16, F_NEXT, u16::MAX);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(!r.run(), "the widest next walked outside the rings",);
    }

    /// **A batch spanning the ring's wrap point validates when it fits and is refused when it does
    /// not** (claim 15).
    #[test]
    fn a_batch_across_the_wrap_validates_only_while_it_fits() {
        let mut r = Rings::new();
        r.set_desc(7, BASE + 0x200, 16, F_NEXT, 0);
        r.set_desc(0, BASE + 0x400, 16, 0, 0);
        r.driver_ring[7] = 0;
        r.driver_idx = 9;
        assert!(
            r.run_range(7, 9),
            "a two-descriptor batch across the wrap was refused"
        );

        let mut r = Rings::new();
        r.set_desc(7, BASE + 0x200, 16, F_NEXT, 0);
        r.driver_ring[7] = 0;
        r.driver_idx = 7;
        assert!(
            !r.run_range(7, 17),
            "a batch of ten against a ring of eight validated across the wrap",
        );
    }

    /// **One queue's validation touches nothing in another queue's block** (claim 16).
    #[test]
    fn a_validate_on_queue_one_touches_nothing_in_queue_zero_block() {
        let mut other_desc = [Desc { addr: 0, word: 0 }; 8];
        other_desc[0] = Desc {
            addr: BASE + 0x200,
            word: (16u64) | ((F_NEXT as u64) << 32) | (1 << 48),
        };
        other_desc[1] = Desc {
            addr: BASE + 0x400,
            word: 16,
        };
        let driver_ring = [0u16; 8];
        let escaped = core::cell::RefCell::new([0u64; 8]);
        let escapes = core::cell::Cell::new(0usize);
        let published = core::cell::Cell::new(false);

        // Queue 1's blocks sit one `queue_block` past queue 0's, the layout `queue_block` pins.
        let dd = DD + queue_block(1);
        let da = DA + queue_block(1);
        let sd = SD + queue_block(1);
        let sa = SA + queue_block(1);

        let ok = validate_and_shadow(
            BASE,
            0x1000,
            dd,
            da,
            sd,
            sa,
            0,
            1,
            QSIZE,
            &|p| {
                if p == da + 2 {
                    1
                } else {
                    driver_ring[((p - da - 4) / 2) as usize]
                }
            },
            &|p| {
                let off = (p - dd) / 16;
                if (p - dd).is_multiple_of(16) {
                    other_desc[off as usize].addr
                } else {
                    other_desc[off as usize].word
                }
            },
            &|p, _v| {
                if !(sa..sa + RING_END).contains(&p) {
                    let mut e = escaped.borrow_mut();
                    let n = escapes.get();
                    if n < e.len() {
                        e[n] = p;
                        escapes.set(n + 1);
                    }
                } else if p == sa + 2 {
                    published.set(true);
                }
            },
            &|p, _v| {
                if !(sd..sd + RING_END).contains(&p) {
                    let mut e = escaped.borrow_mut();
                    let n = escapes.get();
                    if n < e.len() {
                        e[n] = p;
                        escapes.set(n + 1);
                    }
                }
            },
        );
        assert!(ok, "the queue-1 chain was refused");
        assert!(
            published.get(),
            "queue 1's shadow index was never published"
        );
        assert_eq!(
            escapes.get(),
            0,
            "queue 1's validation reached outside its own block",
        );
    }

    /// **A descriptor mutated after validation cannot reach the device** (claim 17): the device
    /// reads the shadow, and the shadow holds what was validated.
    #[test]
    fn a_descriptor_mutated_after_validation_reads_back_validated() {
        let mut r = Rings::new();
        r.set_desc(0, BASE + 0x200, 16, 0, 0);
        r.driver_ring[0] = 0;
        r.driver_idx = 1;
        assert!(r.run(), "the honest descriptor was refused");

        r.driver_desc[0].addr = u64::MAX - 0x1000;
        r.driver_desc[0].word = 0xffff_ffff;

        let shadow_addr = r.shadow_desc.borrow()[0].addr;
        let shadow_word = r.shadow_desc.borrow()[0].word;
        assert_eq!(
            shadow_addr,
            BASE + 0x200,
            "the device's copy followed a mutation made after validation",
        );
        assert_ne!(
            shadow_word, 0xffff_ffff,
            "the device's copy followed a mutated length and flags word",
        );
    }
}
