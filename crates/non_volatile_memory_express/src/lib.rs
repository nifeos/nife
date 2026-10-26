#![no_std]
//! **NVMe queue mechanics, as pure logic**: everything an NVMe driver computes and nothing it
//! touches. The register file, the doorbells and the queues' DMA memory are the caller's (a thin
//! volatile layer over this), so the queue logic is host-testable and Kani-reachable. The protocol
//! (NVMe 1.4, chapters 4 and 5: submission and completion queues, doorbells, the phase tag, the
//! admin queue and PRPs) is what this crate implements, and no more of it.
//!
//! Name: ratified 2026-09-17 (calef, §154 (the acronym test is whether the phrase is spoken)); kept
//! as the crates.io name (#1806).

/// Register offsets in BAR0 (NVMe 1.4 section 3.1). All are 4-byte registers or 8-byte registers
/// the kernel accesses as two 4-byte halves (the spec permits either for the 64-bit ones).
pub mod regs {
    /// Controller Capabilities, 64-bit read-only. See [`super::Cap`].
    pub const CAP: u64 = 0x00;
    /// Controller Configuration, 32-bit. See [`super::cc_enabled`].
    pub const CC: u64 = 0x14;
    /// Controller Status, 32-bit read-only. See [`super::CSTS_RDY`].
    pub const CSTS: u64 = 0x1c;
    /// Admin Queue Attributes: (ACQ entries - 1) << 16 | (ASQ entries - 1).
    pub const AQA: u64 = 0x24;
    /// Admin Submission Queue base, 64-bit, page-aligned physical.
    pub const ASQ: u64 = 0x28;
    /// Admin Completion Queue base, 64-bit, page-aligned physical.
    pub const ACQ: u64 = 0x30;
    /// The first doorbell. Doorbell N is at `DOORBELL_BASE + N * (4 << CAP.DSTRD)`; see
    /// [`super::doorbell`].
    pub const DOORBELL_BASE: u64 = 0x1000;
}

/// CSTS.RDY: the controller has come up (after CC.EN is set) or gone down (after it is cleared).
pub const CSTS_RDY: u32 = 1;
/// CSTS.CFS: Controller Fatal Status. The controller gave up; no doorbell will help.
pub const CSTS_CFS: u32 = 1 << 1;

/// The decoded fields of CAP this driver acts on. A newtype over the raw register value so the
/// bit positions live in exactly one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cap(pub u64);

impl Cap {
    /// Maximum Queue Entries Supported, converted from the register's 0-based field to a count.
    /// Every queue this driver creates must be no deeper than this.
    pub fn max_queue_entries(self) -> u32 {
        (self.0 as u32 & 0xffff) + 1
    }
    /// Doorbell Stride: doorbell N is at `0x1000 + N * (4 << DSTRD)` (a 4-bit field, so the shift
    /// is at most 19 and the multiply cannot overflow a u64 for any real doorbell index).
    pub fn doorbell_stride(self) -> u32 {
        (self.0 >> 32) as u32 & 0xf
    }
    /// Timeout for CSTS.RDY to follow CC.EN, in 500 ms units.
    pub fn timeout_500ms(self) -> u32 {
        (self.0 >> 24) as u32 & 0xff
    }
    /// Memory Page Size Minimum, as a byte count (the field is `2^(12+MPSMIN)`). This driver runs
    /// with CC.MPS = 0, i.e. 4 KiB pages, so bring-up checks this is at most 4096.
    pub fn min_page_size(self) -> u64 {
        1u64 << (12 + ((self.0 >> 48) & 0xf))
    }
}

/// The CC value that enables the controller for this driver's configuration: EN=1, CSS=0 (the NVM
/// command set), MPS=0 (4 KiB memory pages, matching the kernel's frames), AMS=0 (round-robin),
/// IOSQES=6 (64-byte submission entries), IOCQES=4 (16-byte completion entries). The entry sizes
/// are the spec's only defined ones and the controller checks them before honoring a queue
/// creation, which is why they are set here rather than defaulted.
pub fn cc_enabled() -> u32 {
    1 | (6 << 16) | (4 << 20)
}

/// A queue's role, which picks its doorbell: submission tails are even doorbells, completion
/// heads odd (NVMe 1.4 sections 3.1.24-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doorbell {
    /// The submission queue's tail doorbell: even index.
    SubmissionTail,
    /// The completion queue's head doorbell: odd index.
    CompletionHead,
}

/// The BAR0 offset of queue `qid`'s doorbell. `dstrd` is [`Cap::doorbell_stride`].
///
/// # Examples
///
/// ```
/// use non_volatile_memory_express::{Cap, Doorbell, doorbell};
///
/// // A controller reporting DSTRD 0, so doorbells are 4 bytes apart.
/// let cap = Cap(0);
/// assert_eq!(cap.doorbell_stride(), 0);
///
/// // Admin queue (qid 0), then the one I/O queue pair this driver creates.
/// assert_eq!(doorbell(0, Doorbell::SubmissionTail, 0), 0x1000);
/// assert_eq!(doorbell(0, Doorbell::CompletionHead, 0), 0x1004);
/// assert_eq!(doorbell(1, Doorbell::SubmissionTail, 0), 0x1008);
/// assert_eq!(doorbell(1, Doorbell::CompletionHead, 0), 0x100c);
/// ```
pub fn doorbell(qid: u16, which: Doorbell, dstrd: u32) -> u64 {
    let index = 2 * qid as u64
        + match which {
            Doorbell::SubmissionTail => 0,
            Doorbell::CompletionHead => 1,
        };
    regs::DOORBELL_BASE + index * (4u64 << (dstrd & 0xf))
}

// --- commands -------------------------------------------------------------------------------

/// Admin opcodes (NVMe 1.4 section 5).
pub const ADMIN_CREATE_IO_SQ: u8 = 0x01;
/// Create an I/O completion queue.
pub const ADMIN_CREATE_IO_CQ: u8 = 0x05;
/// Fetch a controller or namespace data structure ([`CNS_CONTROLLER`], [`CNS_NAMESPACE`]).
pub const ADMIN_IDENTIFY: u8 = 0x06;

/// NVM (I/O) opcodes (NVMe 1.4 section 6). Flush is opcode 0, and it is mandatory: section 6.8
/// requires every controller to implement it, which is why the block server can offer
/// `filesystem_contract::blk::FLUSH` unconditionally where the virtio one has to negotiate a
/// feature bit and refuse when it is absent.
pub const NVM_FLUSH: u8 = 0x00;
/// Write logical blocks.
pub const NVM_WRITE: u8 = 0x01;
/// Read logical blocks.
pub const NVM_READ: u8 = 0x02;

/// Identify CNS values: the namespace data structure, and the controller's.
pub const CNS_NAMESPACE: u32 = 0x00;
/// The controller's own identify data structure, rather than a namespace's.
pub const CNS_CONTROLLER: u32 = 0x01;

/// One 64-byte submission queue entry, as the sixteen dwords the driver writes. `#[repr(C)]` so
/// the kernel can copy it into the ring as-is; the wire format is little-endian dwords and both
/// target ISAs are little-endian (the same layout argument `filesystem_contract` records).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command(pub [u32; 16]);

impl Command {
    /// The common prefix every builder shares: opcode and command identifier in dword 0 (no fused
    /// operation, PRP data transfers), namespace id in dword 1, PRP1/PRP2 in dwords 6..10.
    fn new(opcode: u8, cid: u16, nsid: u32, prp1: u64, prp2: u64) -> Self {
        let mut dw = [0u32; 16];
        dw[0] = opcode as u32 | (cid as u32) << 16;
        dw[1] = nsid;
        dw[6] = prp1 as u32;
        dw[7] = (prp1 >> 32) as u32;
        dw[8] = prp2 as u32;
        dw[9] = (prp2 >> 32) as u32;
        Command(dw)
    }

    /// IDENTIFY (admin): the 4096-byte data structure named by `cns` lands at `prp1`.
    pub fn identify(cid: u16, cns: u32, nsid: u32, prp1: u64) -> Self {
        let mut c = Self::new(ADMIN_IDENTIFY, cid, nsid, prp1, 0);
        c.0[10] = cns;
        c
    }

    /// CREATE I/O COMPLETION QUEUE (admin): queue `qid`, `entries` deep, physically contiguous
    /// (PC=1) at `prp1`. Interrupts stay off (IEN=0): the kernel driver completes by polling the
    /// phase tag, so no vector is named and none is needed.
    pub fn create_io_cq(cid: u16, qid: u16, entries: u16, prp1: u64) -> Self {
        let mut c = Self::new(ADMIN_CREATE_IO_CQ, cid, 0, prp1, 0);
        c.0[10] = qid as u32 | ((entries as u32 - 1) << 16);
        c.0[11] = 1; // PC only; IEN=0
        c
    }

    /// CREATE I/O SUBMISSION QUEUE (admin): queue `qid`, `entries` deep, physically contiguous at
    /// `prp1`, completing into completion queue `cqid` (which must already exist; the kernel
    /// creates the pair CQ-first for exactly that reason).
    pub fn create_io_sq(cid: u16, qid: u16, entries: u16, cqid: u16, prp1: u64) -> Self {
        let mut c = Self::new(ADMIN_CREATE_IO_SQ, cid, 0, prp1, 0);
        c.0[10] = qid as u32 | ((entries as u32 - 1) << 16);
        c.0[11] = 1 | (cqid as u32) << 16; // PC, and the paired CQ
        c
    }

    /// READ (I/O): `blocks` logical blocks starting at `slba`, into the PRPs. `blocks` is the real
    /// count; the wire field is 0-based and this builder does the subtraction, so a caller cannot
    /// get the off-by-one wrong. `blocks` must be nonzero (asserted: zero would wrap to 65536).
    pub fn read(cid: u16, nsid: u32, slba: u64, blocks: u16, prp1: u64, prp2: u64) -> Self {
        assert!(blocks != 0, "a zero-block transfer has no wire encoding");
        let mut c = Self::new(NVM_READ, cid, nsid, prp1, prp2);
        c.0[10] = slba as u32;
        c.0[11] = (slba >> 32) as u32;
        c.0[12] = (blocks - 1) as u32;
        c
    }

    /// FLUSH (I/O): make everything the controller has already acknowledged for `nsid` durable
    /// (NVMe 1.4 section 6.8). No data transfer, so no PRPs and no LBA range; the command
    /// completes when the volatile write cache, if the controller has one, has been committed.
    ///
    /// **Mandatory for every controller**, which is what lets the block server answer
    /// `filesystem_contract::blk::FLUSH` without negotiating anything. A controller with no
    /// volatile write cache completes it as a no-op, and that is a truthful yes rather than the
    /// silent one a flush verb exists to forbid: the data really is durable.
    pub fn flush(cid: u16, nsid: u32) -> Self {
        Self::new(NVM_FLUSH, cid, nsid, 0, 0)
    }

    /// WRITE (I/O): the same addressing as [`Command::read`], the other direction.
    pub fn write(cid: u16, nsid: u32, slba: u64, blocks: u16, prp1: u64, prp2: u64) -> Self {
        assert!(blocks != 0, "a zero-block transfer has no wire encoding");
        let mut c = Self::new(NVM_WRITE, cid, nsid, prp1, prp2);
        c.0[10] = slba as u32;
        c.0[11] = (slba >> 32) as u32;
        c.0[12] = (blocks - 1) as u32;
        c
    }
}

/// The two PRP words for one data transfer of `len` bytes at physical `base`, against `page_size`
/// pages. PRP1 may carry an offset; every later page pointer must not. One page of data needs only
/// PRP1; a transfer spilling into a second page puts that page in PRP2; anything longer needs a
/// PRP *list*, which this driver does not build (its unit is one 4096-byte filesystem block), so
/// `None` says "not expressible", and the caller treats it as a bug rather than a device error.
///
/// # Examples
///
/// ```
/// use non_volatile_memory_express::prp_pair;
///
/// // A page-aligned block: one pointer is enough.
/// assert_eq!(prp_pair(0x4000_0000, 4096, 4096), Some((0x4000_0000, 0)));
///
/// // The same block, offset half a page: it spills, so PRP2 carries the next page boundary, with
/// // no offset of its own. That "offset-free by construction" is the spec's rule for PRP2.
/// assert_eq!(prp_pair(0x4000_0800, 4096, 4096), Some((0x4000_0800, 0x4000_1000)));
///
/// // Three pages would need a list. Not expressible, and said so.
/// assert_eq!(prp_pair(0x4000_0000, 3 * 4096, 4096), None);
/// ```
pub fn prp_pair(base: u64, len: u64, page_size: u64) -> Option<(u64, u64)> {
    assert!(page_size.is_power_of_two() && page_size >= 512);
    if len == 0 {
        return None;
    }
    let offset = base & (page_size - 1);
    let end = offset.checked_add(len)?;
    if end <= page_size {
        Some((base, 0))
    } else if end <= 2 * page_size {
        // The second pointer is the next page boundary after base, offset-free by construction.
        // Checked: a base in the last page of the address space has no next boundary.
        Some((base, (base & !(page_size - 1)).checked_add(page_size)?))
    } else {
        None // would need a PRP list
    }
}

// --- completions ----------------------------------------------------------------------------

/// One 16-byte completion queue entry, decoded from the four dwords the controller wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Completion {
    /// Command-specific result (dword 0).
    pub result: u32,
    /// The submission queue head as the controller last consumed it: how the driver learns its
    /// submission ring entries are reusable.
    pub sq_head: u16,
    /// Which submission queue the completed command came from.
    pub sq_id: u16,
    /// The command identifier the driver assigned, echoed back.
    pub cid: u16,
    /// The phase tag: dword 3, bit 16. See [`CqState::is_owned`].
    pub phase: bool,
    /// The 15-bit status field; 0 is success. Status code type in bits 8..11, code in 0..8.
    pub status: u16,
}

impl Completion {
    /// Decode the four dwords the controller wrote into a completion queue slot.
    pub fn from_dwords(dw: [u32; 4]) -> Self {
        Completion {
            result: dw[0],
            sq_head: dw[2] as u16,
            sq_id: (dw[2] >> 16) as u16,
            cid: dw[3] as u16,
            phase: dw[3] & (1 << 16) != 0,
            status: (dw[3] >> 17) as u16,
        }
    }
}

// --- ring arithmetic ------------------------------------------------------------------------

/// A submission ring's driver-side state: where the next command lands and what tail value to
/// ring. The controller's consumption (via [`Completion::sq_head`]) is deliberately not modeled:
/// this driver completes every command before submitting the next, so the ring can never fill,
/// and [`SqState::push`] asserts the invariant that guards that assumption instead of carrying a
/// free-slot count nothing would exercise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqState {
    entries: u16,
    tail: u16,
    /// The controller's head, updated from each completion's [`Completion::sq_head`].
    head: u16,
}

impl SqState {
    /// A fresh ring of `entries` slots (at least 2: a one-slot ring cannot distinguish full from
    /// empty in tail/head form, and the spec's minimum queue size is 2).
    pub fn new(entries: u16) -> Self {
        assert!(entries >= 2);
        SqState {
            entries,
            tail: 0,
            head: 0,
        }
    }

    /// Claim the slot for one command: returns the slot index to write, advancing the tail. The
    /// caller then rings [`doorbell`] with [`SqState::tail`]. Panics if the ring is full, which
    /// for this driver is a logic bug (one command in flight per queue), not a load condition.
    pub fn push(&mut self) -> u16 {
        let next = (self.tail + 1) % self.entries;
        assert!(next != self.head, "submission ring full");
        let slot = self.tail;
        self.tail = next;
        slot
    }

    /// The tail value to write to the doorbell after [`SqState::push`].
    pub fn tail(&self) -> u16 {
        self.tail
    }

    /// Record the controller's consumption, from a completion's `sq_head`.
    pub fn note_head(&mut self, head: u16) {
        assert!(
            head < self.entries,
            "controller reported an impossible head"
        );
        self.head = head;
    }
}

/// A completion ring's driver-side state: where the next completion will appear and which phase
/// value marks it as fresh. The phase starts true (the ring starts zeroed, so the controller's
/// first lap writes 1s) and flips every time the head wraps.
///
/// # Examples
///
/// A zeroed ring reads as phase 0, the first lap writes 1s, and every lap flips:
///
/// ```
/// use non_volatile_memory_express::{Completion, CqState};
///
/// let mut cq = CqState::new(2); // a two-slot ring, so it laps quickly
///
/// // The initial zeros. Phase 0 against an expected phase of 1: not ours, do not read the result.
/// let stale = Completion::from_dwords([0, 0, 0, 0]);
/// assert!(!cq.is_owned(&stale));
///
/// // The controller's first completion, phase bit set, for command id 7.
/// let fresh = Completion::from_dwords([0, 0, 0, 7 | (1 << 16)]);
/// assert!(cq.is_owned(&fresh));
/// assert_eq!(fresh.cid, 7);
/// assert_eq!(fresh.status, 0); // 0 is success
/// assert_eq!(cq.pop(), 1); // the value the driver writes to the completion head doorbell
///
/// // Slot 1, same lap, still phase 1.
/// assert!(cq.is_owned(&fresh));
/// assert_eq!(cq.pop(), 0); // wrapped, so the expected phase flipped
///
/// // Now the ring has lapped, and last lap's entries are the stale ones. This is the whole trick:
/// // the bytes in slot 0 have not changed, and their meaning has.
/// assert!(!cq.is_owned(&fresh));
/// assert!(cq.is_owned(&stale));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CqState {
    entries: u16,
    head: u16,
    phase: bool,
}

impl CqState {
    /// A fresh ring of `entries` slots, phase starting true (see the struct docs).
    pub fn new(entries: u16) -> Self {
        assert!(entries >= 2);
        CqState {
            entries,
            head: 0,
            phase: true,
        }
    }

    /// The slot to poll for the next completion.
    pub fn head(&self) -> u16 {
        self.head
    }

    /// Does this completion belong to the driver yet? True when its phase tag matches the phase
    /// this lap expects; a stale entry (last lap's, or the initial zeros) fails the match.
    pub fn is_owned(&self, c: &Completion) -> bool {
        c.phase == self.phase
    }

    /// Consume the entry at the head: advance, flipping the expected phase on wrap. Returns the
    /// new head, which the caller writes to this queue's [`doorbell`].
    pub fn pop(&mut self) -> u16 {
        self.head += 1;
        if self.head == self.entries {
            self.head = 0;
            self.phase = !self.phase;
        }
        self.head
    }
}

// --- identify parsing -----------------------------------------------------------------------

/// The two facts the driver needs from the 4096-byte IDENTIFY namespace structure (NVMe 1.4
/// section 5.15.2, figure 245): the namespace size in logical blocks (NSZE, bytes 0..8) and the
/// data shift of the LBA format in use (FLBAS names one of 16 formats; each format's LBADS is
/// `1 << shift` bytes per block). `None` if the structure is shorter than the fields it must
/// carry, or describes a block size outside what a driver of 4096-byte filesystem blocks can
/// serve (LBADS below 9 is the spec's "unsupported" marker; above 12 a block would outgrow the
/// transfer buffer).
pub fn parse_identify_namespace(data: &[u8]) -> Option<IdentifyNamespace> {
    if data.len() < 384 {
        return None; // NSZE..the LBA format table must be present
    }
    let nsze = u64::from_le_bytes(data[0..8].try_into().ok()?);
    let flbas = data[26] & 0xf;
    // LBA format `flbas` is 4 bytes at 128 + 4*flbas; LBADS is its third byte.
    let lbads = data[128 + 4 * flbas as usize + 2];
    if !(9..=12).contains(&lbads) {
        return None;
    }
    Some(IdentifyNamespace {
        blocks: nsze,
        lba_shift: lbads as u32,
    })
}

/// **The LBA data shift the namespace is formatted with, whatever it is**: the same LBADS byte
/// [`parse_identify_namespace`] reads, before that function's bounds refuse it. `None` only when
/// the structure is too short to carry the format table.
///
/// It exists so a refusal can say what it refused. A drive on the bench must be formatted so that
/// `blocks_per` falls in `1..=8`, and a namespace this driver declines has to print its LBA size
/// rather than arrive at the bench as a server that never started and a test that reads "skipped".
pub fn lba_format_shift(data: &[u8]) -> Option<u8> {
    if data.len() < 384 {
        return None;
    }
    let flbas = data[26] & 0xf;
    Some(data[128 + 4 * flbas as usize + 2])
}

/// See [`parse_identify_namespace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentifyNamespace {
    /// The namespace's size in logical blocks.
    pub blocks: u64,
    /// `1 << lba_shift` bytes per logical block; between 9 and 12 by construction.
    pub lba_shift: u32,
}

impl IdentifyNamespace {
    /// The namespace's capacity in bytes.
    pub fn bytes(&self) -> u64 {
        self.blocks << self.lba_shift
    }
    /// How many logical blocks one `unit`-byte transfer covers, or `None` if `unit` is not a
    /// whole number of logical blocks (a namespace this driver cannot serve 4096-byte filesystem
    /// blocks on; `parse_identify_namespace`'s shift bounds make it impossible for unit 4096).
    pub fn blocks_per(&self, unit: u64) -> Option<u16> {
        if unit == 0 || unit & ((1 << self.lba_shift) - 1) != 0 {
            return None;
        }
        Some((unit >> self.lba_shift) as u16)
    }
}

// ---------------------------------------------------------------------------------------------
// The split between an admin plane and a data plane.
//
// The admin plane (reset, the admin rings, IDENTIFY, Create I/O Queue) stays in the kernel, the
// authority over where a ring lives; the data plane (build a command, ring a doorbell, watch a
// phase tag) runs as an unprivileged process. Below, the few facts the kernel tells it are
// packed, unpacked and bounds-checked where the prover reaches them.
// ---------------------------------------------------------------------------------------------

/// **The largest `CAP.DSTRD` a one-page doorbell mapping can serve**, a refusal rather than an
/// assumption.
///
/// Doorbell N sits at `0x1000 + N * (4 << DSTRD)`, so the stride scales the doorbell *file*. The
/// EL0 data plane's one page of BAR0 names queues 0 and 1, up to
/// doorbell index 3, so it fits while `0x1000 + 3 * (4 << DSTRD) + 4 <= 0x2000`, which is
/// `DSTRD <= 8`. `CAP`'s four-bit field lets a controller report 9 to 15, which this refuses.
/// Kani found the bound: `DSTRD = 15` puts queue 1's submission tail at `0x41000`, a quarter of a
/// megabyte past the mapped window, and QEMU reports 0.
pub const MAX_DSTRD: u32 = 8;

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

/// **Whether a window's client may write the disk.** The disk tree's may not: it serves a read-only
/// view at the prompt, and refusing its `WRITE` and `FLUSH` here makes that a property of the
/// capability it holds rather than of the code it runs.
///
/// ```
/// use non_volatile_memory_express::{DISK_TREE_WINDOW, FILESYSTEM_WINDOW, can_write};
/// assert!(can_write(FILESYSTEM_WINDOW));
/// assert!(!can_write(DISK_TREE_WINDOW));
/// ```
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const fn can_write(window: u64) -> bool {
    window != DISK_TREE_WINDOW
}

/// **Which window a request's badge names**, or `None` for a badge no client was given. Refused
/// rather than clamped: a badge folded onto window 0 would stage in the filesystem server's window,
/// which is the corruption the windows exist to prevent.
///
/// ```
/// use non_volatile_memory_express::{CLIENT_WINDOWS, DISK_TREE_WINDOW, client_window};
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
/// twin of `block_driver`'s two virtio attacker roles. Named provisionally. Behind the
/// `confinement_attackers` feature, so a shipped server cannot even name it.
#[cfg(feature = "confinement_attackers")]
pub const ESCAPE_MAGIC: u64 = 0xE5CA_9E00_4E56_4D45;

/// **What the admin plane tells the data plane at spawn**, and the whole of it: the physical base,
/// because PRP fields carry physical addresses and a process knows virtual ones; the geometry,
/// because reading it means IDENTIFY, the admin authority this split withholds; and the doorbell
/// stride, because it lives in `CAP`, on the register page the process is not mapped.
///
/// **Three `u64`s because a spawn carries three scalars** in the kernel this crate was written for.
/// It is that kernel's own convention with the one program it spawns, not part of NVMe, but it
/// lives here so the packing is written once and a Kani round trip checks its inverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// `CAP.DSTRD`, for [`doorbell`]'s arithmetic.
    pub dstrd: u32,
    /// Logical blocks per filesystem block, from IDENTIFY. `1..=8` for every namespace
    /// [`parse_identify_namespace`] admits.
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
    use super::*;

    #[test]
    fn cap_fields_decode_from_their_spec_positions() {
        // QEMU's real CAP value for a default nvme device is in this family: MQES=0x7ff,
        // TO=0xf, DSTRD=0, MPSMIN=0. Build one by hand and read every field back.
        let cap = Cap(0x7ffu64 | (0xf << 24) | (2 << 32) | (1 << 48));
        assert_eq!(cap.max_queue_entries(), 0x800);
        assert_eq!(cap.timeout_500ms(), 0xf);
        assert_eq!(cap.doorbell_stride(), 2);
        assert_eq!(cap.min_page_size(), 8192);
    }

    #[test]
    fn csts_cfs_is_a_different_bit_from_csts_rdy() {
        // Nothing in this crate's host-tested half reads CSTS; the kernel's volatile layer
        // does. This transcription check is the only
        // place on the host that would catch a copied-to-the-wrong-bit constant (the NVMe
        // Controller Status register: CFS is bit 1, RDY is bit 0).
        assert_eq!(CSTS_CFS, 0b10);
        assert_ne!(CSTS_CFS, CSTS_RDY);
    }

    #[test]
    fn cc_enabled_sets_en_and_the_admin_queue_entry_sizes() {
        let cc = cc_enabled();
        assert_eq!(cc & 1, 1, "EN");
        assert_eq!(
            (cc >> 16) & 0xf,
            6,
            "IOSQES: 64-byte submission entries, 2^6"
        );
        assert_eq!(
            (cc >> 20) & 0xf,
            4,
            "IOCQES: 16-byte completion entries, 2^4"
        );
    }

    #[test]
    fn cc_enableds_three_fields_share_no_bit_so_or_and_xor_agree() {
        // Both `|` operators in `cc_enabled` survive being mutated to `^`, and this is why:
        // EN (bit 0), IOSQES (`6 << 16`, bits 17-18) and IOCQES (`4 << 20`, bit 22) never share a
        // bit, so `|` and `^` compute the exact same `u32` for these three literals. `cc_enabled`
        // takes no argument, so there is no wider case to check; these ARE the only operands it
        // ever combines.
        assert_eq!(1u32 & (6u32 << 16), 0);
        assert_eq!(1u32 & (4u32 << 20), 0);
        assert_eq!((6u32 << 16) & (4u32 << 20), 0);
    }

    #[test]
    fn doorbells_land_where_the_spec_puts_them() {
        // NVMe 1.4 sections 3.1.24-25 worked example, stride 0: SQ0 tail at 0x1000, CQ0 head at
        // 0x1004, SQ1 tail 0x1008, CQ1 head 0x100c.
        assert_eq!(doorbell(0, Doorbell::SubmissionTail, 0), 0x1000);
        assert_eq!(doorbell(0, Doorbell::CompletionHead, 0), 0x1004);
        assert_eq!(doorbell(1, Doorbell::SubmissionTail, 0), 0x1008);
        assert_eq!(doorbell(1, Doorbell::CompletionHead, 0), 0x100c);
        // A stride spreads them: with DSTRD=2 each doorbell is 16 bytes apart.
        assert_eq!(doorbell(1, Doorbell::SubmissionTail, 2), 0x1020);
    }

    #[test]
    fn a_read_command_encodes_the_spec_dwords() {
        let c = Command::read(7, 1, 0x1_0000_0008, 8, 0xdead_b000, 0);
        assert_eq!(c.0[0], 0x02 | 7 << 16, "opcode and cid share dword 0");
        assert_eq!(c.0[1], 1, "nsid");
        assert_eq!((c.0[6], c.0[7]), (0xdead_b000, 0), "prp1 split across 6/7");
        assert_eq!((c.0[10], c.0[11]), (8, 1), "slba split across 10/11");
        assert_eq!(c.0[12], 7, "the block count is 0-based on the wire");
    }

    #[test]
    fn a_write_command_encodes_the_spec_dwords() {
        // The mirror of `a_read_command_encodes_the_spec_dwords`, and `Command::write` had no
        // test of its own: `transfer_command`'s own test always passes `write: false`, so this
        // whole function's body had never run under a host test.
        let c = Command::write(7, 1, 0x1_0000_0008, 8, 0xdead_b000, 0);
        assert_eq!(c.0[0], 0x01 | 7 << 16, "opcode and cid share dword 0");
        assert_eq!(c.0[1], 1, "nsid");
        assert_eq!((c.0[6], c.0[7]), (0xdead_b000, 0), "prp1 split across 6/7");
        assert_eq!((c.0[10], c.0[11]), (8, 1), "slba split across 10/11");
        assert_eq!(c.0[12], 7, "the block count is 0-based on the wire");
    }

    #[test]
    fn queue_creation_packs_size_and_pairing() {
        let cq = Command::create_io_cq(1, 3, 16, 0x2000);
        assert_eq!(cq.0[10], 3 | 15 << 16, "qid, and the 0-based size");
        assert_eq!(cq.0[11], 1, "physically contiguous, interrupts off");
        let sq = Command::create_io_sq(2, 3, 16, 3, 0x3000);
        assert_eq!(sq.0[10], 3 | 15 << 16);
        assert_eq!(sq.0[11], 1 | 3 << 16, "PC, and the paired cqid");
    }

    #[test]
    fn queue_creation_words_pack_disjoint_halves_so_or_and_xor_agree() {
        // Three `| -> ^` mutants survive in `create_io_cq` and `create_io_sq`, all the same
        // shape: a `u16`-width field on the right of `|`, shifted left by 16, next to a value
        // that only ever occupies the low 16 bits. A left shift by 16 zeroes the low 16 bits of
        // *anything*, so the two halves can never share a bit, checked here at the widest a
        // `u16` can be (if the extremes do not collide, nothing narrower can either).
        assert_eq!(
            0xffffu32 & (0xffffu32 << 16),
            0,
            "qid/1 against the shifted half"
        );
    }

    #[test]
    fn completions_decode_and_the_phase_gates_ownership() {
        let cq = CqState::new(4);
        // The controller writes phase=1 on its first lap: owned.
        let fresh = Completion::from_dwords([0, 0, 5 | 1 << 16, 42 | 1 << 16]);
        assert_eq!((fresh.sq_head, fresh.sq_id, fresh.cid), (5, 1, 42));
        assert_eq!(fresh.status, 0);
        assert!(cq.is_owned(&fresh));
        // A zeroed (never-written) entry is not.
        let stale = Completion::from_dwords([0; 4]);
        assert!(!cq.is_owned(&stale));
        // An error status decodes: dword 3 bits 17.. carry it.
        let failed = Completion::from_dwords([0, 0, 0, 1 << 16 | 0x2 << 17]);
        assert_eq!(failed.status, 0x2, "Invalid Field in Command");
    }

    #[test]
    fn the_phase_flips_every_lap_and_only_at_the_wrap() {
        let mut cq = CqState::new(3);
        let mut flips = 0;
        let mut last = true;
        for _ in 0..9 {
            assert!(cq.head() < 3);
            cq.pop();
            let phase_now = cq.is_owned(&Completion::from_dwords([0, 0, 0, 1 << 16]));
            if phase_now != last {
                flips += 1;
                last = phase_now;
            }
        }
        assert_eq!(flips, 3, "nine pops of a 3-ring wrap exactly three times");
    }

    #[test]
    fn head_reports_the_actual_next_slot_not_a_constant() {
        // Every existing caller only ever asserts `head() < entries`, which a constant 0 or a
        // constant 1 both satisfy. Assert the actual progression instead.
        let mut cq = CqState::new(4);
        assert_eq!(cq.head(), 0);
        cq.pop();
        assert_eq!(cq.head(), 1);
        cq.pop();
        assert_eq!(cq.head(), 2);
    }

    #[test]
    fn the_submission_ring_walks_its_slots_in_order() {
        let mut sq = SqState::new(4);
        // With completions consuming as we go (head following), the ring cycles forever.
        for i in 0..12u16 {
            let slot = sq.push();
            assert_eq!(slot, i % 4);
            assert_eq!(sq.tail(), (i + 1) % 4);
            sq.note_head(sq.tail()); // the controller caught up
        }
    }

    #[test]
    #[should_panic(expected = "submission ring full")]
    fn a_full_submission_ring_is_a_driver_bug_not_a_wrap() {
        let mut sq = SqState::new(2);
        sq.push(); // tail 1, head 0: one free slot left is the full condition
        sq.push();
    }

    #[test]
    fn prp_pairs_cover_one_and_two_pages_and_refuse_more() {
        // A page-aligned single page: PRP1 only.
        assert_eq!(prp_pair(0x5000, 4096, 4096), Some((0x5000, 0)));
        // An offset transfer spilling into the next page: PRP2 is that page's boundary.
        assert_eq!(prp_pair(0x5800, 4096, 4096), Some((0x5800, 0x6000)));
        // Exactly two aligned pages: still expressible.
        assert_eq!(prp_pair(0x5000, 8192, 4096), Some((0x5000, 0x6000)));
        // More needs a PRP list, which this driver does not build.
        assert_eq!(prp_pair(0x5800, 8192, 4096), None);
        assert_eq!(prp_pair(0x5000, 0, 4096), None, "empty transfers refused");
    }

    #[test]
    fn identify_namespace_yields_size_and_shift() {
        let mut data = [0u8; 4096];
        data[0..8].copy_from_slice(&0x4000u64.to_le_bytes()); // NSZE: 16384 blocks
        data[26] = 0; // FLBAS: format 0
        data[130] = 9; // format 0's LBADS: 512-byte blocks
        let id = parse_identify_namespace(&data).unwrap();
        assert_eq!(id.blocks, 0x4000);
        assert_eq!(id.lba_shift, 9);
        assert_eq!(id.bytes(), 0x4000 * 512);
        assert_eq!(id.blocks_per(4096), Some(8));

        data[130] = 13; // an LBA format this driver cannot serve
        assert!(parse_identify_namespace(&data).is_none());
        assert!(parse_identify_namespace(&[0u8; 100]).is_none(), "truncated");
    }

    /// A refused format still says what it was, which is what lets the bench print "8192-byte
    /// LBAs" instead of "skipped". Read through FLBAS, so a namespace formatted with its second
    /// format reports that one rather than format 0.
    #[test]
    fn a_refused_lba_format_still_reports_its_shift() {
        let mut data = [0u8; 4096];
        data[26] = 1; // FLBAS: format 1
        data[130] = 9; // format 0: 512 bytes, not the one in use
        data[134] = 13; // format 1: 8192 bytes
        assert!(parse_identify_namespace(&data).is_none());
        assert_eq!(lba_format_shift(&data), Some(13));
        assert_eq!(lba_format_shift(&[0u8; 383]), None);
    }

    /// The shift is read at `128 + 4 * FLBAS + 2` for every format the nibble can name, from a
    /// structure exactly 384 bytes long, with a different shift planted in every slot so a wrong
    /// stride lands on another format's value.
    #[test]
    fn the_reported_shift_is_the_one_in_the_flbas_slot_for_every_format() {
        for flbas in 0..16u8 {
            let mut data = [0u8; 384];
            for format in 0..16usize {
                data[128 + 4 * format + 2] = 20 + format as u8;
            }
            data[26] = flbas | 0x70; // the bits above the format number are not part of it
            assert_eq!(lba_format_shift(&data), Some(20 + flbas), "flbas {flbas}");
        }
        assert_eq!(
            lba_format_shift(&[0u8; 384]),
            Some(0),
            "384 bytes is enough"
        );
    }

    #[test]
    fn identify_namespace_accepts_the_minimum_length_and_refuses_one_byte_less() {
        // The `[0u8; 100]` case above is far short of the boundary and cannot tell `< 384` from
        // `<= 384`. Sit exactly on it instead.
        let mut data = [0u8; 384];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[26] = 0;
        data[130] = 9;
        assert!(
            parse_identify_namespace(&data).is_some(),
            "384 bytes is documented as enough"
        );
        assert!(
            parse_identify_namespace(&data[..383]).is_none(),
            "one byte short of the documented minimum must be refused"
        );
    }

    #[test]
    fn identify_namespace_reads_the_lba_format_table_at_flbas_not_format_zero() {
        // The test above uses FLBAS=0, where the table offset `128 + 4*flbas` happens to equal
        // `128 - 4*flbas`: both are 128. Use FLBAS=1 so the two diverge, and plant a different,
        // still-valid LBADS at format 0's slot so a lane reading the wrong slot gets caught by
        // value rather than by accidentally landing on unset (and therefore refused) bytes.
        let mut data = [0u8; 384];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[26] = 1; // FLBAS: format 1
        data[128 + 2] = 10; // format 0's LBADS: a decoy
        data[128 + 4 + 2] = 11; // format 1's LBADS: the one that must be read
        let id = parse_identify_namespace(&data).unwrap();
        assert_eq!(id.lba_shift, 11, "format 1's slot, not format 0's");
    }

    #[test]
    fn blocks_per_masks_by_the_actual_block_size() {
        // `identify_namespace_yields_size_and_shift`'s only `blocks_per` call is 4096, which is a
        // multiple of every mask this function could plausibly compute (511, 512, or 513: none
        // of their low bits reach as high as bit 12), so it cannot tell them apart, and it never
        // calls `blocks_per(0)` either.
        let id = IdentifyNamespace {
            blocks: 1,
            lba_shift: 9, // 512-byte blocks; mask is 511
        };
        assert_eq!(
            id.blocks_per(0),
            None,
            "a zero unit is refused even though 0 & anything is 0"
        );
        // 1536 = 3 * 512, so the correct mask (511) says it divides evenly. The `+1` mutant's
        // mask (513) and the `/1` mutant's mask (512) both share bit 9 with 1536 and would
        // wrongly refuse it.
        assert_eq!(id.blocks_per(1536), Some(3));
    }

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
        // The spec's minimum queue depth is 2 (`SqState::new`'s own doc says why: a one-slot
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

/// Machine-checked proofs (`cargo kani`).
///
/// The ring arithmetic runs forever against a device that writes into the same memory, so the
/// properties proved are the ones a lapped or hostile ring could otherwise erode: indices stay
/// inside the ring for every reachable state, the phase discipline is exactly one flip per lap,
/// and the doorbell/PRP arithmetic is total (no register value the controller reports can make an
/// offset computation panic or two doorbells collide).
#[cfg(kani)]
mod verification {
    use super::*;

    /// **The completion head stays in bounds and flips the phase exactly at the wrap**, for every
    /// reachable state, not just the ones a test happened to walk.
    /// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.cq_pop_stays_in_bounds_and_flips_only_at_the_wrap.patch`
    #[kani::proof]
    fn cq_pop_stays_in_bounds_and_flips_only_at_the_wrap() {
        let entries: u16 = kani::any();
        let head: u16 = kani::any();
        let phase: bool = kani::any();
        kani::assume(entries >= 2 && head < entries);
        let mut cq = CqState {
            entries,
            head,
            phase,
        };
        let ret = cq.pop();
        assert!(cq.head < entries);
        assert_eq!(ret, cq.head);
        // The head advanced by exactly one, modulo the ring. Without this the two assertions above
        // hold for a pop that wraps one slot early (or late by a lap), which a device writing
        // completions into every slot would then never be read from.
        assert_eq!(
            u32::from(cq.head),
            (u32::from(head) + 1) % u32::from(entries)
        );
        // The phase flipped if and only if the head wrapped to zero.
        assert_eq!(cq.phase != phase, cq.head == 0);
    }

    /// **A submission push from any non-full state lands in bounds**, and the doorbell value it
    /// leaves behind is a legal tail.
    /// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.sq_push_stays_in_bounds.patch`
    #[kani::proof]
    fn sq_push_stays_in_bounds() {
        let entries: u16 = kani::any();
        let tail: u16 = kani::any();
        let head: u16 = kani::any();
        kani::assume(entries >= 2 && tail < entries && head < entries);
        kani::assume((tail + 1) % entries != head); // not full, the asserted precondition
        let mut sq = SqState {
            entries,
            tail,
            head,
        };
        let slot = sq.push();
        assert!(slot < entries);
        assert!(sq.tail() < entries);
    }

    /// **No two doorbells collide**, for any queue ids and any stride the controller can report:
    /// distinct (queue, role) pairs always compute distinct offsets, so a tail write can never
    /// land on a head register. This is the arithmetic the kernel's volatile writes trust.
    /// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.distinct_doorbells_never_collide.patch`
    #[kani::proof]
    fn distinct_doorbells_never_collide() {
        let qa: u16 = kani::any();
        let qb: u16 = kani::any();
        let ra = if kani::any() {
            Doorbell::SubmissionTail
        } else {
            Doorbell::CompletionHead
        };
        let rb = if kani::any() {
            Doorbell::SubmissionTail
        } else {
            Doorbell::CompletionHead
        };
        let dstrd: u32 = kani::any();
        if (qa, ra) != (qb, rb) {
            assert!(doorbell(qa, ra, dstrd) != doorbell(qb, rb, dstrd));
        }
    }

    /// **PRP construction is total and never spans more pages than it names.** For any base,
    /// length and power-of-two page size, `prp_pair` either refuses or returns a pair whose two
    /// pointers cover the whole transfer: PRP2 is zero exactly when one page suffices, and is
    /// page-aligned whenever it is used (the spec's requirement on every pointer after the first).
    /// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.prp_pair_is_total_and_page_disciplined.patch`
    #[kani::proof]
    fn prp_pair_is_total_and_page_disciplined() {
        let base: u64 = kani::any();
        let len: u64 = kani::any();
        let shift: u32 = kani::any();
        kani::assume((9..=16).contains(&shift));
        let page = 1u64 << shift;
        if let Some((prp1, prp2)) = prp_pair(base, len, page) {
            assert_eq!(prp1, base);
            let offset = base & (page - 1);
            if offset + len <= page {
                assert_eq!(prp2, 0);
            } else {
                assert_eq!(prp2 & (page - 1), 0);
                assert_eq!(prp2, (base & !(page - 1)) + page);
            }
        }
    }

    /// **Identify parsing is total for any device response.** The 4096 bytes come from the
    /// controller's DMA; no contents may panic the parse, and an accepted answer always carries a
    /// shift the block driver's arithmetic (`blocks_per(4096)`) can serve.
    /// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.identify_parse_is_total_and_bounds_the_shift.patch`
    #[kani::proof]
    fn identify_parse_is_total_and_bounds_the_shift() {
        let data: [u8; 384] = kani::any();
        if let Some(id) = parse_identify_namespace(&data) {
            assert!((9..=12).contains(&id.lba_shift));
            let per = id.blocks_per(4096).unwrap();
            assert!((1..=8).contains(&per));
        }
    }

    /// **The spawn handoff is lossless in the direction it is used**: whatever the kernel's admin
    /// plane packs into three words, the EL0 data plane unpacks unchanged. The split between a
    /// kernel admin plane and an unprivileged data plane makes this the only channel by which it
    /// learns its own geometry, so a shift expression that was not its own inverse would be a
    /// driver addressing the wrong LBA with nothing in between to notice.
    /// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.the_spawn_handoff_round_trips.patch`
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
    /// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.an_accepted_handoff_keeps_its_doorbells_inside_the_mapped_page.patch`
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
    /// Falsification: replayable `crates/non_volatile_memory_express/falsifications/verification.a_transfer_command_is_only_built_for_a_block_the_namespace_has.patch`
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
