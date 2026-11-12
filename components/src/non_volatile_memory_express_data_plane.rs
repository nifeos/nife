//! **The NVMe data plane**: the `non_volatile_memory_express` image's block server role. It drives
//! a real DMA device with no authority to reset the controller, move a queue, or reach memory
//! outside the run an IOMMU already confined the controller to, including the block-range check
//! [`non_volatile_memory_express::Namespace::transfer_command`] applies. A `#[path]` module of the
//! one program. See notes/non-volatile-memory-express.md.

//! # Capability contract
//!
//! The data plane holds, the complete list: `data_plane_slots::REQUEST`, the request endpoint
//! (`filesystem_contract::blk`, as the virtio block server speaks); `READY`, a readiness endpoint;
//! `GRANTS`, where the admin process's handoff and grants arrive; `OWN_SPACE`, its own address
//! space; and the two grants, held without `GRANT`, mapped where the spawn's second and third
//! arguments say: the data-plane run (the two I/O rings and one window of [`TRANSFER_BLOCKS`]
//! pages per client) and BAR0's doorbell page. Denied, each by decision: BAR0's register page (`CC`, `CSTS`, the
//! admin ring registers), which NVMe 1.4 puts on its own page so this split costs no syscall; the
//! admin pages of the run; every admin command, so where the rings live is the admin process's
//! statement; an `Interrupt`; a `Virtio`; and everything else.

//! # BUGS
//!
//! - One command in flight, so measured throughput is a lower bound on the device.
//! - It polls: no interrupt vector, and nothing would confine one aimed from userspace (§86
//!   (whether an NVMe driver can leave the kernel)).
//! - A hung controller is `EIO` after [`SPIN_BOUND`] polls: `CSTS.CFS` is on the unmapped page.
//! - A PRP aimed backwards from [`Handoff::data_plane_physical_address`] could overwrite the admin
//!   ring; not an escape, and §86 option 4's doorbell validator would close it.
//! - One command per filesystem block (`physical_region_page_pair` builds no PRP list); bulk trails virtio.
//! - The data plane holds its own address space with `WRITE`, so it can map more over its own
//!   pages; it holds nothing else to map.
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace, for the
//! module and its file. The program's name is ratified; this module's is not.

use driver_process_contract::non_volatile_memory_express::{
    DATA_PLANE_GRANT, DATA_PLANE_PAGES, DISK_TREE_WINDOW, DOORBELL_PAGE_GRANT, FIRST_TRANSFER_PAGE,
    Handoff, IO_COMPLETION_QUEUE_PAGE, IO_SUBMISSION_QUEUE_PAGE, client_window, data_plane_slots,
};
use filesystem_contract::blk;
use non_volatile_memory_express::{
    Command, Completion, CompletionQueue, Doorbell, Namespace, SubmissionQueue,
};
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::{
    address_space_map_into, exit, receive, receive_capability, receive_request, reply, send,
};

use crate::{IO_QID, NSID, SPIN_BOUND, barrier};

/// Capability slots, as `driver_process_contract::non_volatile_memory_express::data_plane_slots`
/// names them.
const REQ: u64 = data_plane_slots::REQUEST;
const READY: u64 = data_plane_slots::READY;

/// **Whether a window's client may write the disk.** The disk tree's may not: it serves a read-only
/// view at the prompt, and refusing its `WRITE` and `FLUSH` here makes that a property of the
/// capability it holds rather than of the code it runs. Only this server asks, so it lives here
/// rather than in the contract crate.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
const fn can_write(window: u64) -> bool {
    window != DISK_TREE_WINDOW
}

/// How many blocks one request may carry, and how many pages of transfer buffer the spawner
/// mapped: the blk contract's own number, so this server and the virtio one clamp identically.
const TRANSFER_BLOCKS: usize = blk::TRANSFER_BLOCKS;

/// The data plane's layout, in byte offsets from the start of its run, as the contract crate
/// lays the pages out.
const IO_SQ_OFF: u64 = IO_SUBMISSION_QUEUE_PAGE * PAGE;
const IO_CQ_OFF: u64 = IO_COMPLETION_QUEUE_PAGE * PAGE;
const TRANSFER_OFF: u64 = FIRST_TRANSFER_PAGE * PAGE;

/// Where the spawner says to map the two grants: the data-plane run (two rings and one transfer
/// window per client, window `w` at `TRANSFER_OFF + w * TRANSFER_BLOCKS` pages) and the doorbell
/// page. The doorbell page maps device-typed, because a `DeviceFrame` maps no other way, so every
/// access is uncached and unreordered: a cached write is a command the controller never hears
/// about. The spawner budgets the room (`run_with_own_space_and_room`).
#[derive(Clone, Copy)]
struct Placement {
    data: u64,
    doorbell: u64,
}

impl Placement {
    /// The two addresses, if each is a nonzero page boundary and the two do not overlap.
    fn new(data: u64, doorbell: u64) -> Option<Self> {
        let page_aligned = |va: u64| va != 0 && va.is_multiple_of(PAGE);
        let apart = data + DATA_PLANE_PAGES * PAGE <= doorbell || doorbell + PAGE <= data;
        (page_aligned(data) && page_aligned(doorbell) && apart).then_some(Self { data, doorbell })
    }
}

/// The bring-up step words this server reports on its readiness endpoint, in the `0xDEAD_..`
/// shape `entropy_contract::bring_up_failure` established, so a boot transcript reads the same way
/// across services. A report of [`filesystem_contract::READY`] means a command went to
/// the controller and came back.
const STEP_BAD_HANDOFF: u64 = 0xDEAD_0001;
const STEP_FIRST_READ: u64 = 0xDEAD_0002;

/// The server's whole mutable state: the two ring cursors and a command id.
struct Plane {
    handoff: Handoff,
    /// The data-plane run, mapped where [`Placement::data`] says.
    data: MappedWindow,
    /// The doorbell page, mapped where [`Placement::doorbell`] says.
    bells: MappedWindow,
    sq: SubmissionQueue,
    cq: CompletionQueue,
    cid: u16,
    /// How many device flushes have completed, answered back on every successful `blk::FLUSH`.
    /// A strictly increasing count is what makes that verb's success falsifiable; see the
    /// contract's own argument at `filesystem_contract::blk::FLUSH`.
    flushes: i64,
}

impl Plane {
    fn next_cid(&mut self) -> u16 {
        self.cid = self.cid.wrapping_add(1);
        self.cid
    }

    /// **Submit one command on the I/O queue and poll its completion.** The copy into the
    /// submission ring, the publish barrier, the tail doorbell, the phase-gated poll, the head
    /// doorbell. Every one of those is a volatile access through a window this process mapped;
    /// every index and offset in it came out of `crates/non_volatile_memory_express`.
    ///
    /// `Err(status)` carries the controller's status field, or [`u16::MAX`] for a completion that
    /// never arrived, which this server cannot distinguish from a fatal controller error because
    /// `CSTS` is not in its address space.
    fn transact(&mut self, cmd: Command) -> Result<(), u16> {
        let expect_cid = (cmd.0[0] >> 16) as u16;

        let slot = self
            .sq
            .push()
            .expect("one command in flight per queue, so the ring cannot be full");
        self.data.write(IO_SQ_OFF + slot as u64 * 64, cmd.0);
        // Publish the command before the doorbell: the controller is another observer.
        barrier();
        self.bells.w32(
            non_volatile_memory_express::doorbell(
                IO_QID,
                Doorbell::SubmissionTail,
                self.handoff.dstrd,
            ) - PAGE,
            self.sq.tail() as u32,
        );

        // Poll the completion ring at the head slot until the phase tag says the entry is this
        // lap's. That is the whole completion mechanism: no shared index, no memory the controller
        // and this process both write.
        let cqe = IO_CQ_OFF + self.cq.head() as u64 * 16;
        let mut done: Option<Completion> = None;
        for _ in 0..SPIN_BOUND {
            let c = Completion::from_dwords(self.data.read::<[u32; 4]>(cqe));
            if self.cq.is_owned(&c) {
                done = Some(c);
                break;
            }
            core::hint::spin_loop();
        }
        let c = done.ok_or(u16::MAX)?;
        // Order the phase read before the payload reads that follow (the block's bytes).
        barrier();

        let new_head = self.cq.pop();
        self.bells.w32(
            non_volatile_memory_express::doorbell(
                IO_QID,
                Doorbell::CompletionHead,
                self.handoff.dstrd,
            ) - PAGE,
            new_head as u32,
        );
        // A head the controller cannot have reached is the same protocol violation as a wrong
        // command identifier, so it answers EIO too.
        if self.sq.record_head(c.submission_queue_head).is_err() {
            return Err(u16::MAX);
        }

        // The one-at-a-time discipline makes any other cid a protocol violation. Answering EIO
        // rather than panicking, because this is a server and a client is waiting on it: a
        // process that traps here takes the disk down for everyone holding the endpoint.
        if c.command_id != expect_cid {
            return Err(u16::MAX);
        }
        if c.status != 0 {
            return Err(c.status);
        }
        Ok(())
    }

    /// One whole-filesystem-block transfer into or out of page `slot_index` of client window
    /// `window`.
    fn transfer(
        &mut self,
        block: u64,
        window: usize,
        slot_index: usize,
        write: bool,
    ) -> Result<(), u16> {
        let page = (window * TRANSFER_BLOCKS + slot_index) as u64;
        let data_phys = self.handoff.data_plane_physical_address + TRANSFER_OFF + page * PAGE;
        let cid = self.next_cid();
        let namespace = Namespace::new(self.handoff.size_bytes, self.handoff.blocks_per_transfer);
        let cmd = namespace
            .transfer_command(cid, NSID, block, PAGE, data_phys, PAGE, write)
            // `None` means the block is outside the namespace or the arithmetic would not close;
            // either way it is the caller's request that is wrong, and the crate refused to build
            // a command rather than let the controller answer for it.
            .ok_or(u16::MAX)?;
        self.transact(cmd)
    }
}

/// **Take the admin process's grants and map them**: three words, then the doorbell page and the
/// data-plane run, each tagged with what it is. `None` for anything else, which is a wiring bug.
fn take_grants(placement: Placement) -> Option<(Handoff, MappedWindow, MappedWindow)> {
    let (w0, w1, w2) = receive(data_plane_slots::GRANTS);
    let handoff = Handoff::unpack([w0, w1, w2])?;
    let (mut data, mut bells) = (false, false);
    for _ in 0..2 {
        let (what, slot, _) = receive_capability(data_plane_slots::GRANTS);
        let va = match what {
            DOORBELL_PAGE_GRANT if !bells => {
                bells = true;
                placement.doorbell
            }
            DATA_PLANE_GRANT if !data => {
                data = true;
                placement.data
            }
            _ => return None,
        };
        if address_space_map_into(
            data_plane_slots::OWN_SPACE,
            va,
            slot,
            abi::address_space::MAP_READ_WRITE,
        ) != 0
        {
            return None;
        }
    }
    // SAFETY: both runs are mapped read/write into this space by the two `MAP_INTO`s above, which
    // returned 0, for the life of the process: nothing here unmaps them.
    let windows = unsafe {
        (
            MappedWindow::new(placement.data, DATA_PLANE_PAGES * PAGE),
            MappedWindow::new(placement.doorbell, PAGE),
        )
    };
    Some((handoff, windows.0, windows.1))
}

/// **The data plane**: take the grants, map them at `data_address` and `doorbell_address` (the
/// spawn's second and third arguments), prove one read, report, serve. Never returns.
pub(crate) fn run(data_address: u64, doorbell_address: u64) -> ! {
    // The placement, the handoff and two grants are the whole of what this process was told;
    // anything else is a wiring bug, reported rather than acted on.
    let Some((handoff, data, bells)) =
        Placement::new(data_address, doorbell_address).and_then(take_grants)
    else {
        send(READY, STEP_BAD_HANDOFF, 0, 0);
        exit();
    };
    let mut plane = Plane {
        handoff,
        data,
        bells,
        sq: SubmissionQueue::new(handoff.entries),
        cq: CompletionQueue::new(handoff.entries),
        cid: 0,
        flushes: 0,
    };

    // **Test-only: the DMA-escape attacker role**, compiled only with `confinement_attackers`,
    // which only `cargo xtask test` turns on. `start_dma_escape` in
    // `kernel/src/user/non_volatile_memory_express_service.rs` writes `[ESCAPE_MAGIC, victim_phys]`
    // into the transfer buffer's first two `u64`s before either process runs; a real boot's is
    // zeroed. Read before the readiness transfer, which reuses transfer page 0. See
    // notes/non-volatile-memory-express/the-direct-memory-access-escape-test.md.
    #[cfg(feature = "confinement_attackers")]
    if plane.data.r64(TRANSFER_OFF)
        == driver_process_contract::non_volatile_memory_express::ESCAPE_MAGIC
    {
        let victim = plane.data.r64(TRANSFER_OFF + 8);
        run_dma_escape(&mut plane, victim);
    }

    // **Read one block before reporting ready** (`entropy.rs`'s discipline): "up" means a client
    // that asks is answered. It is also the only end-to-end check here, since `CSTS` is unmapped.
    let first = plane.transfer(0, 0, 0, false);
    let report = if first.is_ok() {
        filesystem_contract::READY
    } else {
        STEP_FIRST_READ
    };
    // Word 1 is the namespace size this process was handed, so a boot transcript can see the
    // geometry reached ring 3 intact; word 2 is the status of that first read, which is the one
    // number that says whether the controller answered and what it said if it refused.
    send(
        READY,
        report,
        handoff.size_bytes,
        first.err().unwrap_or(0) as u64,
    );

    serve(plane)
}

/// The serve loop: one endpoint, one wait point, forever. `filesystem_contract::blk`, the same
/// four verbs the virtio block server answers, so the FS server and the disk tools cannot tell
/// which kind of disk is underneath.
///
/// **A request stages in the window its badge names** (milestone 834 (ioping on nife and Linux)). Each client maps only its
/// own window (`kernel/src/user/non_volatile_memory_express_service.rs`), and this server answers
/// one request at a time, so two clients staging at once touch different pages. A badge no client
/// was given is refused before anything is staged.
fn serve(mut plane: Plane) -> ! {
    loop {
        let req = receive_request(REQ);
        let (w0, block) = (req.w0, req.w1);
        let Some(reply_to) = req.delivered.into_reply() else {
            // A plain SEND or a SEND_CAPABILITY on a CALL-only contract: nothing to answer, and a
            // delegation is deleted rather than answered into (milestone 706 (a `CALL` server can
            // tell a Reply from a delegation)).
            continue;
        };
        // **Clamp**, the defence the virtio block server states at the same line: every caller
        // today sends at most `TRANSFER_BLOCKS` because the field cannot encode more, and a
        // request is never trusted to stay inside the region it shares just because the packing
        // allows a larger number to be spelled.
        let count = blk::req_blocks(w0).min(TRANSFER_BLOCKS);
        let Some(window) = client_window(req.badge) else {
            reply(reply_to, filesystem_contract::reply_err(22) as u64, 0); // EINVAL
            continue;
        };
        let op = filesystem_contract::operation(w0);
        if (op == blk::WRITE || op == blk::FLUSH) && !can_write(req.badge) {
            reply(reply_to, filesystem_contract::reply_err(30) as u64, 0); // EROFS
            continue;
        }
        let r0: i64 = match op {
            blk::READ => transfer_range(&mut plane, block, window, count, false),
            blk::WRITE => transfer_range(&mut plane, block, window, count, true),
            blk::SIZE => plane.handoff.size_bytes as i64,
            blk::FLUSH => {
                let cid = plane.next_cid();
                match plane.transact(Command::flush(cid, NSID)) {
                    // Never EOPNOTSUPP, unlike the virtio server: NVMe 1.4 section 6.8 makes Flush
                    // mandatory. The rising count is the contract's: two syncs returning one
                    // number would mean the second never reached the device.
                    Ok(()) => {
                        plane.flushes += 1;
                        plane.flushes
                    }
                    Err(_) => filesystem_contract::reply_err(5), // EIO
                }
            }
            _ => filesystem_contract::reply_err(22), // EINVAL: an opcode this server has no verb for
        };
        reply(reply_to, r0 as u64, 0);
    }
}

/// **Aim DMA outside this server's region, both directions, and report what the controller did**
/// (milestone 261 (the NVMe driver leaves the kernel)'s Outstanding item; milestone 202 (every
/// confinement test is a ritual until somebody breaks the confinement)). It goes around
/// [`Namespace::transfer_command`]'s refusal and hands [`Command::read`]/[`Command::write`] a PRP of
/// its own, `victim`, a physical address outside this process's DMA window: a `READ` would
/// overwrite a frame this server was never granted, a `WRITE` would leak `victim` onto the disk.
/// With the IOMMU confining the controller to its own region both fault, and the kernel test reads
/// `victim`'s untouched canary and the IOMMU's fault record. **This process holds no name for
/// `victim`**: a raw number out of shared memory, the authority an escaping driver would fabricate.
#[cfg(feature = "confinement_attackers")]
fn run_dma_escape(plane: &mut Plane, victim: u64) -> ! {
    let blocks = plane.handoff.blocks_per_transfer;
    let pages = non_volatile_memory_express::PhysicalRegionPagePair {
        first_entry: victim,
        second_entry: 0,
    };
    // disk -> victim: the controller would overwrite a frame outside the region.
    let rd = {
        let cid = plane.next_cid();
        plane.transact(Command::read(cid, NSID, 0, blocks, pages))
    };
    // victim -> disk: the controller would read a frame outside the region.
    let wr = {
        let cid = plane.next_cid();
        plane.transact(Command::write(cid, NSID, 0, blocks, pages))
    };
    // Word 1 the read-escape status, word 2 the write-escape status: each an Err from a confined
    // controller (its status, or u16::MAX for no completion), and for the transcript only. The
    // kernel test's proof is the IOMMU fault and the untouched canary.
    let code = |r: Result<(), u16>| r.err().unwrap_or(0) as u64;
    send(READY, code(rd), code(wr), 0);
    exit();
}

/// `count` contiguous blocks, one command each. See this module's `BUGS` for why it is not one
/// command: `non_volatile_memory_express::physical_region_page_pair` refuses a transfer needing a PRP list, and a multi-page one does.
fn transfer_range(plane: &mut Plane, block: u64, window: usize, count: usize, write: bool) -> i64 {
    for i in 0..count {
        if plane
            .transfer(block.saturating_add(i as u64), window, i, write)
            .is_err()
        {
            return filesystem_contract::reply_err(5); // EIO
        }
    }
    0
}
