#![no_std]
//! **What nife's kernel and its NVMe server agree on**: the transfer windows the server keeps, one
//! per client, and the three words the kernel's admin plane hands the unprivileged data plane at
//! spawn. Both binaries link this crate, so each fact is written once (rule 7 of `AGENTS.md`). The
//! NVMe protocol itself is `non_volatile_memory_express`'s; this crate holds only nife's process
//! plan around it. See notes/non-volatile-memory-express.md.
//!
//! Name: provisional, minted 2026-10-26 (UTC) by lane nvme-names-ratified. `_contract` because it
//! is a wire contract between two binaries (design/naming.md), and the stem is the crate whose
//! driver it serves.

use non_volatile_memory_express::{Command, MAX_DSTRD, prp_pair};

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
/// use non_volatile_memory_express_contract::{CLIENT_WINDOWS, DISK_TREE_WINDOW, client_window};
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

/// **The sentinel that selects the test-only DMA-escape role**, written by the confinement
/// falsification's wiring into the first `u64` of the data plane's transfer buffer before `_start`
/// runs, with the victim frame's physical address in the next `u64`. The EL0 server reads it first
/// thing; a real boot's transfer buffer is zeroed (`alloc_contiguous_zeroed`), so the magic never
/// matches. The kernel writes it and the EL0 server reads it, so it lives in this one crate. Its
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
    /// `CAP.DSTRD`, for [`doorbell`](non_volatile_memory_express::doorbell)'s arithmetic.
    pub dstrd: u32,
    /// Logical blocks per filesystem block, from IDENTIFY. `1..=8` for every namespace
    /// [`parse_identify_namespace`](non_volatile_memory_express::parse_identify_namespace) admits.
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
    /// unit), or a `dstrd` above [`MAX_DSTRD`].
    ///
    /// Name: provisional. No ruling covers it.
    pub fn unpack(words: [u64; 3]) -> Option<Handoff> {
        let dstrd = (words[0] >> 32) as u32;
        let blocks_per = (words[0] >> 16) as u16;
        let entries = words[0] as u16;
        if dstrd > MAX_DSTRD || !(1..=8).contains(&blocks_per) || entries < 2 {
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

    /// **Is `block` a filesystem block this namespace has?** The data plane's own bounds check,
    /// run before a command is built rather than left to the controller's status field, because a
    /// driver that asks for an LBA past the end learns about it far from where it computed it.
    ///
    /// `unit` is the filesystem block size in bytes. `false` for any `block` whose transfer would
    /// run past [`size_bytes`](Self::size_bytes), and for a `unit` of zero.
    ///
    /// Name: provisional. No ruling covers it.
    pub fn holds_block(&self, block: u64, unit: u64) -> bool {
        match block.checked_add(1).and_then(|n| n.checked_mul(unit)) {
            Some(end) => end <= self.size_bytes,
            None => false,
        }
    }

    /// **Build the NVM command for one whole-filesystem-block transfer**, or `None` when the
    /// arithmetic will not close: a block outside the namespace, an LBA that overflows, or a
    /// buffer that is not expressible as a PRP pair (which for a page-aligned `unit`-byte buffer
    /// with `unit <= 2 * page_size` it always is; see [`prp_pair`]).
    ///
    /// This is the whole of what the data plane computes. What is left in the program around it is
    /// a 64-byte copy, a doorbell write, and a loop watching a phase tag.
    ///
    /// Name: provisional. No ruling covers it.
    #[allow(clippy::too_many_arguments)]
    pub fn transfer_command(
        &self,
        cid: u16,
        nsid: u32,
        block: u64,
        unit: u64,
        data_phys: u64,
        page_size: u64,
        write: bool,
    ) -> Option<Command> {
        if !self.holds_block(block, unit) {
            return None;
        }
        let slba = block.checked_mul(self.blocks_per as u64)?;
        let (prp1, prp2) = prp_pair(data_phys, unit, page_size)?;
        Some(if write {
            Command::write(cid, nsid, slba, self.blocks_per, prp1, prp2)
        } else {
            Command::read(cid, nsid, slba, self.blocks_per, prp1, prp2)
        })
    }
}

#[cfg(test)]
mod tests {
    use non_volatile_memory_express::{Doorbell, NVM_READ, doorbell};

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
        // `MAX_DSTRD`, which Kani found), a ring of one, and a geometry no namespace
        // `parse_identify_namespace` admits can produce for a 4096-byte unit.
        assert!(Handoff::unpack([(MAX_DSTRD as u64 + 1) << 32 | 8 << 16 | 16, 0, 0]).is_none());
        // And the largest stride that does fit is admitted, with its last doorbell inside the
        // page: this bound is a real edge, not a round number chosen for comfort.
        let widest = Handoff::unpack([(MAX_DSTRD as u64) << 32 | 8 << 16 | 16, 0, 0])
            .expect("MAX_DSTRD itself must be servable");
        assert!(doorbell(1, Doorbell::CompletionHead, widest.dstrd) + 4 <= 0x2000);
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
            dstrd: MAX_DSTRD,
            blocks_per: 8,
            entries: 16,
            data_plane_phys: 0x4001_3000,
            size_bytes: 8 * 1024 * 1024,
        };
        let words = h.pack();
        assert_eq!(
            words[0] >> 32,
            MAX_DSTRD as u64,
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

    /// The last block of the namespace is servable and the one after it is not, which is the
    /// check that keeps an out-of-range request from becoming a command the controller answers
    /// with a status the caller has to decode backwards.
    #[test]
    fn the_data_plane_refuses_a_block_the_namespace_does_not_have() {
        let h = Handoff {
            dstrd: 0,
            blocks_per: 8,
            entries: 16,
            data_plane_phys: 0x4001_3000,
            size_bytes: 8 * 1024 * 1024,
        };
        const LAST: u64 = 8 * 1024 * 1024 / 4096 - 1;
        assert!(h.holds_block(LAST, 4096));
        assert!(!h.holds_block(LAST + 1, 4096));
        assert!(
            !h.holds_block(u64::MAX, 4096),
            "no overflow, no wrap-around"
        );

        // And the command the last block produces addresses the LBA the geometry says it should.
        let cmd = h
            .transfer_command(7, 1, LAST, 4096, 0x4001_5000, 4096, false)
            .expect("the last block is servable");
        assert_eq!(cmd.0[0] & 0xff, NVM_READ as u32);
        assert_eq!(cmd.0[10] as u64 | (cmd.0[11] as u64) << 32, LAST * 8);
        assert!(
            h.transfer_command(7, 1, LAST + 1, 4096, 0x4001_5000, 4096, false)
                .is_none()
        );
    }
}

/// Machine-checked proofs (`cargo kani`) of the spawn handoff: it round-trips, an accepted one
/// keeps every doorbell inside the one mapped page, and no transfer command addresses a block
/// outside the namespace.
#[cfg(kani)]
mod verification {
    use non_volatile_memory_express::{Doorbell, doorbell};

    use super::*;

    /// **The spawn handoff is lossless in the direction it is used**: whatever the kernel's admin
    /// plane packs into three words, the EL0 data plane unpacks unchanged. The split between a
    /// kernel admin plane and an unprivileged data plane makes this the only channel by which it
    /// learns its own geometry, so a shift expression that was not its own inverse would be a
    /// driver addressing the wrong LBA with nothing in between to notice.
    /// Falsification: replayable `crates/non_volatile_memory_express_contract/falsifications/verification.the_spawn_handoff_round_trips.patch`
    #[kani::proof]
    fn the_spawn_handoff_round_trips() {
        let h = Handoff {
            dstrd: kani::any(),
            blocks_per: kani::any(),
            entries: kani::any(),
            data_plane_phys: kani::any(),
            size_bytes: kani::any(),
        };
        kani::assume(h.dstrd <= MAX_DSTRD && (1..=8).contains(&h.blocks_per) && h.entries >= 2);
        assert_eq!(Handoff::unpack(h.pack()), Some(h));
    }

    /// **An unpacked handoff can always drive the doorbell arithmetic**, for every word the
    /// kernel could have packed. [`MAX_DSTRD`] is what buys this, and this harness is why that
    /// constant exists: a stride the spec permits and QEMU never reports computes a doorbell
    /// offset outside the one page of BAR0 the EL0 data plane is mapped, which is the one
    /// arithmetic error in this driver that reaches hardware.
    /// Falsification: replayable `crates/non_volatile_memory_express_contract/falsifications/verification.an_accepted_handoff_keeps_its_doorbells_inside_the_mapped_page.patch`
    #[kani::proof]
    fn an_accepted_handoff_keeps_its_doorbells_inside_the_mapped_page() {
        let words: [u64; 3] = [kani::any(), kani::any(), kani::any()];
        if let Some(h) = Handoff::unpack(words) {
            // Queue 0 (admin) and queue 1 (the one I/O pair the admin plane creates) are the only
            // doorbells this driver can name; with the largest stride `CAP` can report they are
            // the four lowest offsets in the doorbell page.
            for qid in 0..=1u16 {
                for which in [Doorbell::SubmissionTail, Doorbell::CompletionHead] {
                    let off = doorbell(qid, which, h.dstrd);
                    assert!((0x1000..0x2000).contains(&off));
                }
            }
        }
    }

    /// **The data plane never builds a command for a block outside its namespace, and never
    /// overflows computing one.** This is the bounds check the unprivileged data plane leans on:
    /// the IOMMU bounds
    /// where the controller may *write*, and nothing but this bounds *which LBA* is asked for, so
    /// a caller-supplied `u64` must not be able to wrap into a valid-looking address.
    /// Falsification: replayable `crates/non_volatile_memory_express_contract/falsifications/verification.a_transfer_command_is_only_built_for_a_block_the_namespace_has.patch`
    #[kani::proof]
    fn a_transfer_command_is_only_built_for_a_block_the_namespace_has() {
        let h = Handoff {
            dstrd: 0,
            blocks_per: kani::any(),
            entries: 16,
            data_plane_phys: kani::any(),
            size_bytes: kani::any(),
        };
        kani::assume((1..=8).contains(&h.blocks_per));
        let block: u64 = kani::any();
        let data_phys: u64 = kani::any();
        kani::assume(data_phys.is_multiple_of(4096) && data_phys < u64::MAX - 8192);
        if h.transfer_command(1, 1, block, 4096, data_phys, 4096, kani::any())
            .is_some()
        {
            assert!(h.holds_block(block, 4096));
            // The transfer's last byte is inside the namespace, which is the property
            // `holds_block` exists to state and the one a wrap would quietly break.
            assert!((block + 1) * 4096 <= h.size_bytes);
        }
    }
}
