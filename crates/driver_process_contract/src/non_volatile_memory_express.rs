//! **What the kernel and the NVMe server agree on**: the transfer windows the server keeps, one per
//! client, the largest doorbell stride the server's one page of BAR0 can serve, and the three words
//! the kernel's admin plane hands the unprivileged data plane at spawn. The NVMe protocol itself is
//! `non_volatile_memory_express`'s; this module holds only nife's process plan around it. See
//! notes/non-volatile-memory-express.md.
//!
//! Name: provisional, minted 2026-10-26 (UTC) by lane driver-contracts-folded, after the crate it
//! serves, as this crate's other modules are named.

use ::non_volatile_memory_express::registers::DOORBELL_BASE;

// ---------------------------------------------------------------------------------------------
// The split between an admin plane and a data plane.
//
// The admin plane (reset, the admin rings, IDENTIFY, Create I/O Queue) stays in the kernel, the
// authority over where a ring lives; the data plane (build a command, ring a doorbell, watch a
// phase tag) runs as an unprivileged process. Below, the few facts the kernel tells it are
// packed, unpacked and bounds-checked where the prover reaches them.
// ---------------------------------------------------------------------------------------------

/// **How many transfer windows the EL0 server keeps, one per client.** Each window is
/// `filesystem_contract::blk::TRANSFER_BLOCKS` pages of the DMA region, and the server stages a
/// request in window `badge`, the badge on the endpoint capability the client called (one frame
/// per client channel), so two clients staging at once cannot overwrite each other.
///
/// The clients are a fixed set the kernel wires, one window each, so there is no pool and nothing
/// to hand back: [`FILESYSTEM_WINDOW`], [`INSTALLER_WINDOW`] and [`DISK_TREE_WINDOW`]. A new client
/// gets a new constant and this count grows by one; each window costs 64 KiB of the DMA region.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const CLIENT_WINDOWS: usize = 3;

/// The unbadged window: the filesystem server, and every kernel-side caller of the request endpoint.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const FILESYSTEM_WINDOW: u64 = 0;
/// `system_installer`'s window, whether it is installing or confirming a boot slot beside a live
/// filesystem server.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const INSTALLER_WINDOW: u64 = 1;
/// The read-only disk tree's window, a program that reads the namespace
/// beside a live filesystem server.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const DISK_TREE_WINDOW: u64 = 2;

/// **Which window a request's badge names**, or `None` for a badge no client was given. Refused
/// rather than clamped: a badge folded onto window 0 would stage in the filesystem server's window,
/// which is the corruption the windows exist to prevent.
///
/// ```
/// use driver_process_contract::non_volatile_memory_express::{
///     CLIENT_WINDOWS, DISK_TREE_WINDOW, client_window,
/// };
/// assert_eq!(client_window(DISK_TREE_WINDOW), Some(2));
/// assert_eq!(client_window(CLIENT_WINDOWS as u64), None);
/// ```
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const fn client_window(badge: u64) -> Option<usize> {
    if badge < CLIENT_WINDOWS as u64 {
        Some(badge as usize)
    } else {
        None
    }
}

/// **The largest `CAP.DSTRD` the data plane's one page of BAR0 can serve**, a refusal rather than
/// an assumption.
///
/// The kernel maps the EL0 data plane one page of BAR0, the doorbell page at `0x1000`
/// ([`DOORBELL_BASE`]), and the data plane rings the doorbells of queues 0 and 1 only, up to
/// doorbell index 3. Doorbell N sits at `0x1000 + N * (4 << DSTRD)`, so they fit while
/// `0x1000 + 3 * (4 << DSTRD) + 4 <= 0x2000`, which is `DSTRD <= 8`. `CAP`'s four-bit field lets a
/// controller report 9 to 15, which the kernel refuses at bring-up and [`Handoff::unpack`] refuses
/// again. Kani found the bound: `DSTRD = 15` puts queue 1's submission tail at `0x41000`, a
/// quarter of a megabyte past the mapped window, and QEMU reports 0.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `MAX_DSTRD` (it copied the field's
/// abbreviation into a name this tree authors). The ruling of 2026-10-26 (UTC) on #2749 set it in
/// this module rather than in `non_volatile_memory_express`, because the bound is nife's mapping
/// and not the specification's.
pub const MAX_DOORBELL_STRIDE: u32 = 8;

/// The one page of BAR0 the data plane is mapped, from the doorbell base.
const DOORBELL_PAGE_END: u64 = DOORBELL_BASE + 0x1000;

// The bound is the page's edge, at compile time: doorbell index 3 ends inside the page at
// `MAX_DOORBELL_STRIDE` and past it one stride wider.
const _: () = assert!(DOORBELL_BASE + 3 * (4 << MAX_DOORBELL_STRIDE) + 4 <= DOORBELL_PAGE_END);
const _: () = assert!(DOORBELL_BASE + 3 * (4 << (MAX_DOORBELL_STRIDE + 1)) + 4 > DOORBELL_PAGE_END);

/// **The sentinel that selects the test-only DMA-escape role**, written by the confinement
/// falsification's wiring into the first `u64` of the data plane's transfer buffer before `_start`
/// runs, with the victim frame's physical address in the next `u64`. The EL0 server reads it first
/// thing; a real boot's transfer buffer is zeroed (`alloc_contiguous_zeroed`), so the magic never
/// matches. The kernel writes it and the EL0 server reads it, so it lives in this one module. Its
/// one caller is a test that breaks the data plane's DMA confinement and watches it fail, the NVMe
/// twin of `block_driver`'s two virtio attacker roles. Behind the
/// `confinement_attackers` feature, so a shipped server cannot even name it.
///
/// Name: provisional. No ruling covers it.
#[cfg(feature = "confinement_attackers")]
pub const ESCAPE_MAGIC: u64 = 0xE5CA_9E00_4E56_4D45;

/// **What the admin plane tells the data plane at spawn**, and the whole of it: the physical base,
/// because PRP fields carry physical addresses and a process knows virtual ones; the geometry,
/// because reading it means IDENTIFY, the admin authority this split withholds; and the doorbell
/// stride, because it lives in `CAP`, on the register page the process is not mapped.
///
/// **Three `u64`s because a spawn carries three scalars** (`kernel/src/user/entry.rs`'s `Spawn`).
/// The kernel packs them and the server unpacks them, so the packing is written once, here, and a
/// Kani round trip checks its inverse.
///
/// Name: provisional. No ruling covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// `CAP.DSTRD`, for [`doorbell`](::non_volatile_memory_express::doorbell)'s arithmetic.
    pub dstrd: u32,
    /// Logical blocks per filesystem block, from IDENTIFY. `1..=8` for every namespace
    /// [`parse_identify_namespace`](::non_volatile_memory_express::parse_identify_namespace) admits.
    pub blocks_per: u16,
    /// Entries per I/O ring, as the admin plane created them.
    pub entries: u16,
    /// The **physical** base of the run of pages the data plane was mapped: the I/O submission
    /// ring, the I/O completion ring, then the transfer buffer.
    pub data_plane_phys: u64,
    /// The namespace's capacity in bytes, the `SIZE` answer.
    pub size_bytes: u64,
}

impl Handoff {
    /// The three words, in `arg0`, `arg1`, `arg2` order.
    ///
    /// Name: provisional. No ruling covers it.
    pub fn pack(&self) -> [u64; 3] {
        [
            (self.dstrd as u64) << 32 | (self.blocks_per as u64) << 16 | self.entries as u64,
            self.data_plane_phys,
            self.size_bytes,
        ]
    }

    /// [`pack`](Self::pack)'s inverse, refusing a word that cannot describe a controller this
    /// driver could serve rather than carrying the nonsense forward into a doorbell offset.
    /// `None` for a zero ring depth, a zero or out-of-range `blocks_per` (the shift bounds
    /// `parse_identify_namespace` enforces make `1..=8` the whole reachable range for a 4096-byte
    /// unit), or a `dstrd` above [`MAX_DOORBELL_STRIDE`].
    ///
    /// Name: provisional. No ruling covers it.
    pub fn unpack(words: [u64; 3]) -> Option<Handoff> {
        let dstrd = (words[0] >> 32) as u32;
        let blocks_per = (words[0] >> 16) as u16;
        let entries = words[0] as u16;
        if dstrd > MAX_DOORBELL_STRIDE || !(1..=8).contains(&blocks_per) || entries < 2 {
            return None;
        }
        Some(Handoff {
            dstrd,
            blocks_per,
            entries,
            data_plane_phys: words[1],
            size_bytes: words[2],
        })
    }
}

#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests {
    use ::non_volatile_memory_express::{Doorbell, doorbell};

    use super::*;

    /// The handoff the kernel's admin plane builds is the handoff the EL0 data plane reads, and a
    /// word that cannot describe a servable controller is refused rather than decoded.
    #[test]
    fn the_handoff_survives_three_words_and_refuses_nonsense() {
        let h = Handoff {
            dstrd: 0,
            blocks_per: 8,
            entries: 16,
            data_plane_phys: 0x4001_3000,
            size_bytes: 8 * 1024 * 1024,
        };
        assert_eq!(Handoff::unpack(h.pack()), Some(h));

        // A stride whose doorbell file would not fit the one page of BAR0 this wiring maps (see
        // `MAX_DOORBELL_STRIDE`, which Kani found), a ring of one, and a geometry no namespace
        // `parse_identify_namespace` admits can produce for a 4096-byte unit.
        assert!(
            Handoff::unpack([(MAX_DOORBELL_STRIDE as u64 + 1) << 32 | 8 << 16 | 16, 0, 0])
                .is_none()
        );
        // And the largest stride that does fit is admitted, with its last doorbell inside the
        // page: this bound is a real edge, not a round number chosen for comfort.
        let widest = Handoff::unpack([(MAX_DOORBELL_STRIDE as u64) << 32 | 8 << 16 | 16, 0, 0])
            .expect("MAX_DOORBELL_STRIDE itself must be servable");
        assert!(doorbell(1, Doorbell::CompletionHead, widest.dstrd) + 4 <= DOORBELL_PAGE_END);
        assert!(Handoff::unpack([8 << 16 | 1, 0, 0]).is_none());
        assert!(Handoff::unpack([9 << 16 | 16, 0, 0]).is_none());
        assert!(Handoff::unpack([16, 0, 0]).is_none(), "blocks_per zero");
        // The spec's minimum queue depth is 2 (`SubmissionQueue::new`'s own doc says why: a one-slot
        // ring cannot distinguish full from empty), and nothing above sits exactly on that
        // boundary: `entries: 1` is refused either by `< 2` or by the `<= 2` this guards against.
        assert!(
            Handoff::unpack([8 << 16 | 2, 0, 0]).is_some(),
            "the minimum queue depth must be accepted, not refused"
        );
    }

    #[test]
    fn pack_shifts_dstrd_left_not_right() {
        // The round-trip test above uses `dstrd: 0`, where `<< 32` and `>> 32` both give 0.
        // Nothing else calls `pack` at all, so a nonzero `dstrd` never reached it.
        let h = Handoff {
            dstrd: MAX_DOORBELL_STRIDE,
            blocks_per: 8,
            entries: 16,
            data_plane_phys: 0x4001_3000,
            size_bytes: 8 * 1024 * 1024,
        };
        let words = h.pack();
        assert_eq!(
            words[0] >> 32,
            MAX_DOORBELL_STRIDE as u64,
            "dstrd lands in the high word, shifted left, not right"
        );
        assert_eq!(Handoff::unpack(words), Some(h));
    }

    #[test]
    fn pack_words_three_fields_share_no_bit_so_or_and_xor_agree() {
        // The remaining two `| -> ^` survivors in `pack` are both this shape: `entries` (a plain
        // `u16`, bits 0-15), `blocks_per as u64 << 16` (bits 16-31, a `u16` shifted left 16), and
        // `dstrd as u64 << 32` (bits 32-63, a `u32` shifted left 32). Each shift zeroes exactly
        // the bits the field below it occupies, checked at the widest each field can be.
        assert_eq!(
            0xffffu64 & (0xffffu64 << 16),
            0,
            "entries against blocks_per"
        );
        assert_eq!(
            0xffff_ffffu64 & (0xffff_ffffu64 << 32),
            0,
            "the low 32 bits against dstrd"
        );
    }

    /// The stride bound is the page's edge: the widest admitted stride's last doorbell ends inside
    /// the page, and the next stride's begins past it.
    #[test]
    fn max_dstrd_is_the_widest_stride_whose_doorbells_fit_one_page() {
        let page_end = DOORBELL_PAGE_END;
        assert!(doorbell(1, Doorbell::CompletionHead, MAX_DOORBELL_STRIDE) + 4 <= page_end);
        assert!(doorbell(1, Doorbell::CompletionHead, MAX_DOORBELL_STRIDE + 1) >= page_end);
    }
}
