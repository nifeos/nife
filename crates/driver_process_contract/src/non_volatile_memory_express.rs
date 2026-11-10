//! **What the kernel, the NVMe admin process and the NVMe data plane agree on**: the DMA run's
//! layout, each process's capability slots and windows, the admin process's report to the kernel,
//! the three words and two grants the admin process hands the data plane, the transfer windows the
//! data plane keeps, one per client, and the largest doorbell stride its one page of BAR0 can
//! serve. The NVMe protocol itself is `non_volatile_memory_express`'s; this module holds only
//! nife's process plan around it. See notes/non-volatile-memory-express.md.
//!
//! Name: provisional, minted 2026-10-26 (UTC) by lane driver-contracts-folded, after the crate it
//! serves, as this crate's other modules are named.

use ::non_volatile_memory_express::registers::FIRST_DOORBELL_REGISTER;
use startup_page_contract::WindowKind;

// ---------------------------------------------------------------------------------------------
// The split between an admin plane and a data plane (§86 (whether an NVMe driver can leave the
// kernel)).
//
// The kernel finds the controller, confines its requester id to one DMA run, and knows nothing
// else about NVMe. The admin process holds BAR0's register and doorbell pages and the run's admin
// pages: it resets the controller, builds the admin rings, IDENTIFYs the namespace and creates the
// one I/O queue pair. It then grants the data plane the doorbell page and the run's data-plane
// pages. The data plane builds commands, rings doorbells and watches phase tags. Below, every fact
// the three exchange is packed, unpacked and bounds-checked where the prover reaches it.
// ---------------------------------------------------------------------------------------------

/// **The role word an NVMe process finds in `arg0`**: the one program image serves as the admin
/// process or as the data plane, and the capabilities each spawn grants are what separate them.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const ADMINISTRATION_ROLE: u64 = 0x4E56_4D45_4144_4D31;
/// The data plane's role word, [`ADMINISTRATION_ROLE`]'s twin.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const DATA_PLANE_ROLE: u64 = 0x4E56_4D45_4441_5431;

/// Pages of BAR0 the admin process is mapped, from BAR0's base: the register page (`CAP`, `CC`,
/// `CSTS`, `AQA`, `ASQ`, `ACQ`) and the doorbell page at [`FIRST_DOORBELL_REGISTER`]. The pages
/// past them hold no register this driver uses, and on a controller that puts its MSI-X table
/// there, mapping them would hand a process the power to aim an interrupt.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const REGISTER_PAGES: u64 = 2;

/// The admin submission ring's page in the DMA run.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const ADMINISTRATION_SUBMISSION_QUEUE_PAGE: u64 = 0;
/// The admin completion ring's page in the DMA run.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const ADMINISTRATION_COMPLETION_QUEUE_PAGE: u64 = 1;
/// The page IDENTIFY writes the namespace structure into.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const IDENTIFY_PAGE: u64 = 2;
/// How many pages at the run's start are the admin process's alone: the two admin rings and the
/// IDENTIFY page. The data plane is never granted them.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const ADMINISTRATION_PAGES: u64 = 3;
/// Pages in one client's transfer window: one request's worth of filesystem blocks.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const TRANSFER_PAGES: u64 = filesystem_contract::blk::TRANSFER_BLOCKS as u64;
/// Pages the data plane is granted, from [`ADMINISTRATION_PAGES`] on: the I/O submission ring, the I/O
/// completion ring, then one [`TRANSFER_PAGES`] window per client.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const DATA_PLANE_PAGES: u64 = 2 + TRANSFER_PAGES * CLIENT_WINDOWS as u64;
/// The whole DMA run the kernel allocates and confines the controller to.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const DIRECT_MEMORY_ACCESS_PAGES: u64 = ADMINISTRATION_PAGES + DATA_PLANE_PAGES;
/// The I/O submission ring's page, counted from the start of the data plane's pages.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const IO_SUBMISSION_QUEUE_PAGE: u64 = 0;
/// The I/O completion ring's page, counted from the start of the data plane's pages.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const IO_COMPLETION_QUEUE_PAGE: u64 = 1;
/// The first transfer window's page, counted from the start of the data plane's pages.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const FIRST_TRANSFER_PAGE: u64 = 2;

/// The admin process's register window on its startup page: [`REGISTER_PAGES`] pages of BAR0,
/// device memory.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const REGISTER_KIND: WindowKind = WindowKind(1);
/// The admin process's queue window on its startup page: the run's first [`ADMINISTRATION_PAGES`] pages.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const ADMINISTRATION_QUEUE_KIND: WindowKind = WindowKind(2);

/// **The admin process's capability slots**, in the order its spawn grants them.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub mod administration_slots {
    /// `WRITE`: the endpoint the admin process sends its one [`super::AdministrationReport`] on.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const REPORT: u64 = 0;
    /// `WRITE`: the endpoint the data plane receives its handoff and its two grants on.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const GRANTS: u64 = 1;
    /// `WRITE | GRANT`: the doorbell page, as a `DeviceFrame`, for the data plane.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const DOORBELL_PAGE: u64 = 2;
    /// `READ | WRITE | GRANT`: the data plane's pages of the DMA run, as one `PageFrame` run.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const DATA_PLANE: u64 = 3;
    /// `READ`: the endpoint the kernel calls with [`super::RETIRE`].
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const CONTROL: u64 = 4;
}

/// **The data plane's capability slots.** The first three come from its spawn; the doorbell page
/// and the data-plane run arrive later over [`GRANTS`](data_plane_slots::GRANTS), in slots the
/// kernel chooses.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub mod data_plane_slots {
    /// `READ`: the request endpoint the data plane serves `filesystem_contract::blk` on.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const REQUEST: u64 = 0;
    /// `WRITE`: the readiness endpoint, written once.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const READY: u64 = 1;
    /// `READ`: where the admin process's handoff and grants arrive.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const GRANTS: u64 = 2;
    /// `WRITE`: the data plane's own address space, which it maps its two grants into.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const OWN_SPACE: u64 = 3;
}

/// The data word riding the doorbell page's `SEND_CAPABILITY`, so the data plane maps each grant
/// as what it is rather than by arrival order alone.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const DOORBELL_PAGE_GRANT: u64 = 0x4E56_0001;
/// The data word riding the data-plane run's `SEND_CAPABILITY`.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const DATA_PLANE_GRANT: u64 = 0x4E56_0002;

/// **The one verb on the admin process's control endpoint**: take the controller down (`CC.EN`
/// cleared, `CSTS.RDY` followed) and exit. A `CALL` with this in word 0; the reply's word 0 is 0
/// once the controller reports not-ready, and nonzero when it never does. Only a caller about to
/// free the DMA run needs it: freeing under an enabled controller is use-after-free by hardware.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub const RETIRE: u64 = 0x4E56_5254;

/// **Why the admin process refused the controller**, each naming the phase it died in, because
/// "the disk did not come up" from a five-step bring-up is the kind of message this project's
/// debugging notes complain about. It crosses from the admin process to the kernel in
/// [`AdministrationReport`].
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BringUpError {
    /// `CSTS.RDY` never followed `CC.EN` within the spin bound (either direction).
    ControllerTimeout,
    /// `CSTS.CFS`: the controller declared a fatal internal error.
    ControllerFatal,
    /// A completion never arrived for a submitted admin command.
    CompletionTimeout,
    /// The controller completed an admin command with a nonzero status field (the field is
    /// carried).
    Command(u16),
    /// The identify data described a namespace this driver cannot serve (a logical block format
    /// outside 512..=4096 bytes, or a size of zero). `logical_block_size_shift` is the format's
    /// size shift as the controller reported it, 0 if the structure was too short to carry one, so
    /// the refusal names the logical block size it refused (fatal risk 6's second night-of
    /// condition).
    UnsupportedNamespace {
        /// The logical block format's size shift.
        logical_block_size_shift: u8,
        /// The namespace's size in logical blocks.
        blocks: u64,
    },
    /// `CAP` describes a controller this driver cannot drive: queues too shallow, a minimum page
    /// size above 4 KiB, or a doorbell stride above [`MAXIMUM_DOORBELL_STRIDE`].
    UnsupportedController,
}

/// **What the admin process tells the kernel once**, after bring-up: the geometry the kernel's
/// wiring reports, or why there is no disk.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdministrationReport {
    /// The controller is enabled with its one I/O pair created, and the data plane's handoff is
    /// next on [`administration_slots::GRANTS`].
    Ready {
        /// The namespace's capacity in bytes.
        size_bytes: u64,
        /// Logical blocks per filesystem block, `1..=8`.
        blocks_per_transfer: u16,
    },
    /// Bring-up stopped at the named phase, and the admin process granted nothing.
    Refused(BringUpError),
}

/// [`AdministrationReport::Ready`]'s word 0.
const REPORT_READY: u64 = 0x4E56_4144_0000_0001;
/// [`AdministrationReport::Refused`]'s word 0; the low byte of word 1 names the variant.
const REPORT_REFUSED: u64 = 0x4E56_4144_0000_0002;

impl AdministrationReport {
    /// The three words of one `SEND`.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const fn pack(&self) -> [u64; 3] {
        match *self {
            AdministrationReport::Ready {
                size_bytes,
                blocks_per_transfer,
            } => [REPORT_READY, size_bytes, blocks_per_transfer as u64],
            AdministrationReport::Refused(why) => {
                let (kind, detail, blocks) = match why {
                    BringUpError::ControllerTimeout => (1, 0, 0),
                    BringUpError::ControllerFatal => (2, 0, 0),
                    BringUpError::CompletionTimeout => (3, 0, 0),
                    BringUpError::Command(status) => (4, status as u64, 0),
                    BringUpError::UnsupportedNamespace {
                        logical_block_size_shift,
                        blocks,
                    } => (5, logical_block_size_shift as u64, blocks),
                    BringUpError::UnsupportedController => (6, 0, 0),
                };
                [REPORT_REFUSED, kind + (detail << 8), blocks]
            }
        }
    }

    /// [`pack`](Self::pack)'s inverse, or `None` for words no admin process sends: a wrong word 0,
    /// a variant byte that names none, or a ready geometry outside `1..=8`.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
    pub const fn unpack(words: [u64; 3]) -> Option<AdministrationReport> {
        match words[0] {
            REPORT_READY => {
                if words[2] < 1 || words[2] > 8 {
                    return None;
                }
                Some(AdministrationReport::Ready {
                    size_bytes: words[1],
                    blocks_per_transfer: words[2] as u16,
                })
            }
            REPORT_REFUSED => {
                let detail = words[1] >> 8;
                let why = match words[1] & 0xff {
                    1 => BringUpError::ControllerTimeout,
                    2 => BringUpError::ControllerFatal,
                    3 => BringUpError::CompletionTimeout,
                    4 => BringUpError::Command(detail as u16),
                    5 => BringUpError::UnsupportedNamespace {
                        logical_block_size_shift: detail as u8,
                        blocks: words[2],
                    },
                    6 => BringUpError::UnsupportedController,
                    _ => return None,
                };
                Some(AdministrationReport::Refused(why))
            }
            _ => None,
        }
    }
}

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
/// The admin process grants the data plane one page of BAR0, the doorbell page at `0x1000`
/// ([`FIRST_DOORBELL_REGISTER`]), and the data plane rings the doorbells of queues 0 and 1 only, up to
/// doorbell index 3. Doorbell N sits at `0x1000 + N * (4 << DSTRD)`, so they fit while
/// `0x1000 + 3 * (4 << DSTRD) + 4 <= 0x2000`, which is `DSTRD <= 8`. `CAP`'s four-bit field lets a
/// controller report 9 to 15, which the admin process refuses at bring-up and [`Handoff::unpack`]
/// refuses again. Kani found the bound: `DSTRD = 15` puts queue 1's submission tail at `0x41000`, a
/// quarter of a megabyte past the mapped window, and QEMU reports 0.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `MAX_DSTRD` (it copied the field's
/// abbreviation into a name this tree authors) and `MAX_DOORBELL_STRIDE` (max spelled out, as the
/// tree-wide ruling asks). The ruling of 2026-10-26 (UTC) on #2749 set it in
/// this module rather than in `non_volatile_memory_express`, because the bound is nife's mapping
/// and not the specification's.
pub const MAXIMUM_DOORBELL_STRIDE: u32 = 8;

/// The one page of BAR0 the data plane is mapped, from the doorbell base.
const DOORBELL_PAGE_END: u64 = FIRST_DOORBELL_REGISTER + 0x1000;

// The bound is the page's edge, at compile time: doorbell index 3 ends inside the page at
// `MAXIMUM_DOORBELL_STRIDE` and past it one stride wider.
const _: () =
    assert!(FIRST_DOORBELL_REGISTER + 3 * (4 << MAXIMUM_DOORBELL_STRIDE) + 4 <= DOORBELL_PAGE_END);
const _: () = assert!(
    FIRST_DOORBELL_REGISTER + 3 * (4 << (MAXIMUM_DOORBELL_STRIDE + 1)) + 4 > DOORBELL_PAGE_END
);

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

/// **What the admin process tells the data plane before its two grants**, and the whole of it: the
/// physical base, because physical region page (PRP) fields carry physical addresses and a process knows virtual ones; the
/// geometry, because reading it means IDENTIFY, the admin authority this split withholds; and the
/// doorbell stride, because it lives in `CAP`, on the register page the data plane is not granted.
///
/// **Three `u64`s because one `SEND` carries three words**, the first message on
/// [`administration_slots::GRANTS`]. The admin process packs them and the data plane unpacks them, so the
/// packing is written once, here, and a Kani round trip checks its inverse.
///
/// Name: provisional. No ruling covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// `CAP.DSTRD`, for [`doorbell`](::non_volatile_memory_express::doorbell)'s arithmetic.
    pub dstrd: u32,
    /// Logical blocks per filesystem block, from IDENTIFY. `1..=8` for every namespace
    /// [`parse_identify_namespace`](::non_volatile_memory_express::parse_identify_namespace) admits.
    pub blocks_per_transfer: u16,
    /// Entries per I/O ring, as the admin plane created them.
    pub entries: u16,
    /// The **physical** base of the run of pages the data plane is granted: the I/O submission
    /// ring, the I/O completion ring, then the transfer windows.
    pub data_plane_physical_address: u64,
    /// The namespace's capacity in bytes, the `SIZE` answer.
    pub size_bytes: u64,
}

/// The handoff word's three fields share no bit (dstrd, a `u32`, shifted above bit 32;
/// `blocks_per_transfer`, a `u16`, above bit 16; `entries`, a `u16`, below it), which the compiler
/// checks here; the mutation exclusion for [`Handoff::pack`] names this constant. The
/// constant-assertion lint is off on purpose: the assertion is constant by construction, and the
/// compiler checking it is its job.
///
/// Name: provisional, minted 2026-10-28 (UTC) by lane/954-remaining-crates.
#[allow(clippy::assertions_on_constants, clippy::eq_op)]
pub const THE_HANDOFF_WORD_FIELDS_SHARE_NO_BIT: () =
    assert!(((u32::MAX as u64) << 32) + ((u16::MAX as u64) << 16) + u16::MAX as u64 == u64::MAX);

impl Handoff {
    /// The three words, in `arg0`, `arg1`, `arg2` order.
    ///
    /// Name: provisional. No ruling covers it.
    pub fn pack(&self) -> [u64; 3] {
        [
            (self.dstrd as u64) << 32
                | (self.blocks_per_transfer as u64) << 16
                | self.entries as u64,
            self.data_plane_physical_address,
            self.size_bytes,
        ]
    }

    /// [`pack`](Self::pack)'s inverse, refusing a word that cannot describe a controller this
    /// driver could serve rather than carrying the nonsense forward into a doorbell offset.
    /// `None` for a zero ring depth, a zero or out-of-range `blocks_per_transfer` (the shift bounds
    /// `parse_identify_namespace` enforces make `1..=8` the whole reachable range for a 4096-byte
    /// unit), or a `dstrd` above [`MAXIMUM_DOORBELL_STRIDE`].
    ///
    /// Name: provisional. No ruling covers it.
    pub fn unpack(words: [u64; 3]) -> Option<Handoff> {
        let dstrd = (words[0] >> 32) as u32;
        let blocks_per_transfer = (words[0] >> 16) as u16;
        let entries = words[0] as u16;
        if dstrd > MAXIMUM_DOORBELL_STRIDE || !(1..=8).contains(&blocks_per_transfer) || entries < 2
        {
            return None;
        }
        Some(Handoff {
            dstrd,
            blocks_per_transfer,
            entries,
            data_plane_physical_address: words[1],
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

    /// The handoff the admin process builds is the handoff the data plane reads, and a
    /// word that cannot describe a servable controller is refused rather than decoded.
    #[test]
    fn the_handoff_survives_three_words_and_refuses_nonsense() {
        let h = Handoff {
            dstrd: 0,
            blocks_per_transfer: 8,
            entries: 16,
            data_plane_physical_address: 0x4001_3000,
            size_bytes: 8 * 1024 * 1024,
        };
        assert_eq!(Handoff::unpack(h.pack()), Some(h));

        // A stride whose doorbell file would not fit the one page of BAR0 this wiring maps (see
        // `MAXIMUM_DOORBELL_STRIDE`, which Kani found), a ring of one, and a geometry no namespace
        // `parse_identify_namespace` admits can produce for a 4096-byte unit.
        assert!(
            Handoff::unpack([
                (MAXIMUM_DOORBELL_STRIDE as u64 + 1) << 32 | 8 << 16 | 16,
                0,
                0
            ])
            .is_none()
        );
        // And the largest stride that does fit is admitted, with its last doorbell inside the
        // page: this bound is a real edge, not a round number chosen for comfort.
        let widest = Handoff::unpack([(MAXIMUM_DOORBELL_STRIDE as u64) << 32 | 8 << 16 | 16, 0, 0])
            .expect("MAXIMUM_DOORBELL_STRIDE itself must be servable");
        assert!(doorbell(1, Doorbell::CompletionHead, widest.dstrd) + 4 <= DOORBELL_PAGE_END);
        assert!(Handoff::unpack([8 << 16 | 1, 0, 0]).is_none());
        assert!(Handoff::unpack([9 << 16 | 16, 0, 0]).is_none());
        assert!(
            Handoff::unpack([16, 0, 0]).is_none(),
            "blocks_per_transfer zero"
        );
        // The spec's minimum queue depth is 2 (`SubmissionQueue::new`'s own doc says why: a one-slot
        // ring cannot distinguish full from empty), and nothing above sits exactly on that
        // boundary: `entries: 1` is refused either by `< 2` or by the `<= 2` this guards against.
        assert!(
            Handoff::unpack([8 << 16 | 2, 0, 0]).is_some(),
            "the minimum queue depth must be accepted, not refused"
        );
    }

    /// Every report the admin process can send comes back as itself, each refusal keeping its
    /// variant and detail apart from every other's.
    #[test]
    fn every_administration_report_round_trips() {
        let reports = [
            AdministrationReport::Ready {
                size_bytes: 8 * 1024 * 1024,
                blocks_per_transfer: 8,
            },
            AdministrationReport::Ready {
                size_bytes: u64::MAX,
                blocks_per_transfer: 1,
            },
            AdministrationReport::Refused(BringUpError::ControllerTimeout),
            AdministrationReport::Refused(BringUpError::ControllerFatal),
            AdministrationReport::Refused(BringUpError::CompletionTimeout),
            AdministrationReport::Refused(BringUpError::Command(0x4002)),
            AdministrationReport::Refused(BringUpError::Command(u16::MAX)),
            AdministrationReport::Refused(BringUpError::UnsupportedNamespace {
                logical_block_size_shift: 13,
                blocks: 0x1_0000_0001,
            }),
            AdministrationReport::Refused(BringUpError::UnsupportedController),
        ];
        for (n, r) in reports.iter().enumerate() {
            assert_eq!(AdministrationReport::unpack(r.pack()), Some(*r), "{r:?}");
            for other in &reports[..n] {
                assert_ne!(r.pack(), other.pack(), "{r:?} packs as {other:?}");
            }
        }
    }

    /// Words no admin process sends are refused: a wrong first word, a variant byte that names
    /// none, and a ready geometry outside `1..=8`.
    #[test]
    fn an_administration_report_no_administration_process_sends_is_refused() {
        assert_eq!(AdministrationReport::unpack([0, 0, 0]), None);
        assert_eq!(AdministrationReport::unpack([REPORT_READY, 4096, 0]), None);
        assert_eq!(AdministrationReport::unpack([REPORT_READY, 4096, 9]), None);
        assert!(AdministrationReport::unpack([REPORT_READY, 4096, 1]).is_some());
        assert_eq!(AdministrationReport::unpack([REPORT_REFUSED, 0, 0]), None);
        assert_eq!(AdministrationReport::unpack([REPORT_REFUSED, 7, 0]), None);
        // The detail rides above the variant byte, so a status whose low byte is a variant number
        // still decodes as the status it is.
        assert_eq!(
            AdministrationReport::unpack([REPORT_REFUSED, 4 | 0x0106 << 8, 0]),
            Some(AdministrationReport::Refused(BringUpError::Command(0x0106)))
        );
    }

    /// The run's layout: the admin pages first, then the data plane's rings and windows, with
    /// nothing between or past them.
    #[test]
    fn the_direct_memory_access_run_is_the_administration_pages_then_the_data_plane() {
        let administration = [
            ADMINISTRATION_SUBMISSION_QUEUE_PAGE,
            ADMINISTRATION_COMPLETION_QUEUE_PAGE,
            IDENTIFY_PAGE,
        ];
        assert!(administration.iter().all(|&p| p < ADMINISTRATION_PAGES));
        assert_eq!(ADMINISTRATION_PAGES, 3);
        assert_eq!(
            FIRST_TRANSFER_PAGE + TRANSFER_PAGES * CLIENT_WINDOWS as u64,
            DATA_PLANE_PAGES
        );
        assert!(IO_SUBMISSION_QUEUE_PAGE < FIRST_TRANSFER_PAGE);
        assert!(IO_COMPLETION_QUEUE_PAGE < FIRST_TRANSFER_PAGE);
        assert_ne!(IO_SUBMISSION_QUEUE_PAGE, IO_COMPLETION_QUEUE_PAGE);
        assert_eq!(DIRECT_MEMORY_ACCESS_PAGES, 53);
    }

    #[test]
    fn pack_shifts_dstrd_left_not_right() {
        // The round-trip test above uses `dstrd: 0`, where `<< 32` and `>> 32` both give 0.
        // Nothing else calls `pack` at all, so a nonzero `dstrd` never reached it.
        let h = Handoff {
            dstrd: MAXIMUM_DOORBELL_STRIDE,
            blocks_per_transfer: 8,
            entries: 16,
            data_plane_physical_address: 0x4001_3000,
            size_bytes: 8 * 1024 * 1024,
        };
        let words = h.pack();
        assert_eq!(
            words[0] >> 32,
            MAXIMUM_DOORBELL_STRIDE as u64,
            "dstrd lands in the high word, shifted left, not right"
        );
        assert_eq!(Handoff::unpack(words), Some(h));
    }

    #[test]
    fn pack_words_three_fields_share_no_bit_so_or_and_xor_agree() {
        // The remaining two `| -> ^` survivors in `pack` are both this shape: `entries` (a plain
        // `u16`, bits 0-15), `blocks_per_transfer as u64 << 16` (bits 16-31, a `u16` shifted left
        // 16), and `dstrd as u64 << 32` (bits 32-63, a `u32` shifted left 32). Each shift zeroes
        // exactly the bits the field below it occupies, checked at the widest each field can be.
        assert_eq!(
            0xffffu64 & (0xffffu64 << 16),
            0,
            "entries against blocks_per_transfer"
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
    fn maximum_doorbell_stride_is_the_widest_whose_doorbells_fit_one_page() {
        let page_end = DOORBELL_PAGE_END;
        assert!(doorbell(1, Doorbell::CompletionHead, MAXIMUM_DOORBELL_STRIDE) + 4 <= page_end);
        assert!(doorbell(1, Doorbell::CompletionHead, MAXIMUM_DOORBELL_STRIDE + 1) >= page_end);
    }
}
