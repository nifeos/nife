//! **The NVMe block server, at EL0**: option 2a of §86 (whether an NVMe driver can leave the
//! kernel), and notes/non-volatile-memory-express.md. It drives a real DMA device with no authority
//! to reset the controller, move a queue, or reach memory outside the region an IOMMU already
//! confined the controller to. All the arithmetic is in `crates/non_volatile_memory_express`,
//! host-tested and Kani-reachable, including the block-range check
//! [`non_volatile_memory_express::Handoff::transfer_command`] applies; what is left here is
//! volatile access, two barriers and a bounded poll.

//! # Capability contract
//!
//! Held, the complete list: slot 0, the request endpoint (`filesystem_contract::blk`, as the virtio
//! block server speaks); slot 1, a readiness endpoint; the data plane at [`DATA_PLANE_VA`] (the two
//! I/O rings and one window of [`TRANSFER_BLOCKS`] pages per client); and BAR0's doorbell page at
//! [`DOORBELL_VA`]. Denied, each by decision: BAR0's register page (`CC`, `CSTS`, the admin ring
//! registers), which NVMe 1.4 puts on its own page so this split costs no syscall; the admin
//! plane's pages; every admin command, so where the rings live is the kernel's statement; an `Irq`;
//! a `Virtio`; any name for the two windows, installed before `_start`; and everything else.

//! # BUGS
//!
//! - One command in flight, so measured throughput is a lower bound on the device.
//! - It polls: no interrupt vector, and nothing would confine one aimed from userspace (§86).
//! - A hung controller is `EIO` after [`SPIN_BOUND`] polls: `CSTS.CFS` is on the unmapped page.
//! - A PRP aimed backwards from [`Handoff::data_plane_phys`] could overwrite the admin ring; not an
//!   escape, and §86 option 4's doorbell validator would close it.
//! - One command per filesystem block (`prp_pair` builds no PRP list); bulk trails virtio.
//!
//! Name: ratified 2026-09-17 (calef, §154 (the acronym test is whether the phrase is spoken)), the
//! program's half 2026-09-18: it shares the crate's name, as `compositor` does. Refused
//! `nvme_server`, `nvme_driver` and `non_volatile_memory_express_server` (34 bytes, past the
//! 32-byte `NAME_LEN`, which was not raised for it).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use filesystem_contract::blk;
use non_volatile_memory_express::{
    CLIENT_WINDOWS, Command, Completion, CqState, Doorbell, Handoff, SqState, can_write,
    client_window,
};
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::{exit, receive_request, reply, send};

/// Capability slots, by convention with `kernel/src/user/non_volatile_memory_express_service.rs`.
const REQ: u64 = 0;
const READY: u64 = 1;

/// Where the spawner maps this server's data plane. **Must match
/// `kernel/src/user/non_volatile_memory_express_service.rs`'s `DATA_PLANE_VA`.**
const DATA_PLANE_VA: u64 = address_space_map::pair_page(0x0000_0000_0098_0000);

/// Where the spawner maps the doorbell page of BAR0, device-typed. **Must match
/// `kernel/src/user/non_volatile_memory_express_service.rs`'s `DOORBELL_VA`.**
const DOORBELL_VA: u64 = address_space_map::pair_page(0x0000_0000_009c_0000);

/// How many blocks one request may carry, and how many pages of transfer buffer the spawner
/// mapped: the blk contract's own number, so this server and the virtio one clamp identically.
const TRANSFER_BLOCKS: usize = blk::TRANSFER_BLOCKS;

/// The data plane's layout, in byte offsets from [`DATA_PLANE_VA`]. It is the spawner's layout,
/// stated again here because the two halves live in different address spaces and nothing but the
/// spawn contract joins them; `kernel/src/non_volatile_memory_express.rs`'s page constants are the other end.
const IO_SQ_OFF: u64 = 0;
const IO_CQ_OFF: u64 = PAGE;
const TRANSFER_OFF: u64 = 2 * PAGE;

/// The whole data-plane window: two rings and one transfer window per client, window `w` at
/// `TRANSFER_OFF + w * TRANSFER_BLOCKS` pages.
// SAFETY: the spawner maps exactly these pages read/write at DATA_PLANE_VA before `_start` runs
// (`kernel/src/user/non_volatile_memory_express_service.rs`), for the lifetime of this process.
const DATA: MappedWindow = unsafe {
    MappedWindow::new(
        DATA_PLANE_VA,
        TRANSFER_OFF + (CLIENT_WINDOWS * TRANSFER_BLOCKS) as u64 * PAGE,
    )
};

/// The doorbell page. Device-typed, so every access is uncached and unreordered, which is what a
/// doorbell needs: a cached write is a command the controller never hears about.
// SAFETY: the spawner maps one device-typed page at DOORBELL_VA before `_start` runs.
const BELLS: MappedWindow = unsafe { MappedWindow::new(DOORBELL_VA, PAGE) };

/// The one I/O queue pair the kernel's admin plane created. Ringing any other queue's doorbell
/// would be ringing a queue that does not exist.
const IO_QID: u16 = 1;

/// The one namespace QEMU's `-device nvme,drive=` carries; the admin plane identified it.
const NSID: u32 = 1;

/// How many polls of the completion ring before this server declares the command lost. QEMU
/// completes synchronously inside the doorbell write, so this is paranoia there; on hardware it is
/// roughly a second of spinning. **Not a measured bound**, and it is the only thing standing
/// between a dead controller and a server that never answers, because `CSTS` is unreachable from
/// here. See this module's `BUGS`.
const SPIN_BOUND: u32 = 50_000_000;

/// The bring-up step words this server reports on its readiness endpoint, in the `0xDEAD_..`
/// shape `entropy_contract::bringup_failure` established, so a boot transcript reads the same way
/// across services. A report of [`filesystem_contract::READY`] means a command went to
/// the controller and came back.
const STEP_BAD_HANDOFF: u64 = 0xDEAD_0001;
const STEP_FIRST_READ: u64 = 0xDEAD_0002;

/// **Order our writes to the rings before the doorbell that announces them, and the phase-tag read
/// before the payload it gates.** The controller is another observer of this memory, and on a
/// weakly-ordered CPU nothing else stops a doorbell write hoisting above the command it announces.
///
/// Stronger than [`user_mode_runtime::virtio::virtio_ring_barrier`] on purpose: those drivers
/// notify through a syscall instruction, so `x86_64` needs only a compiler fence and aarch64 only
/// `dmb ish`. This doorbell is a direct store to a device-typed page with no syscall between the
/// ring stores and it, so it keeps `dmb sy` and a real `mfence`. Sharing the two would be an
/// ordering change, not deduplication.
fn barrier() {
    // Shared with the `e1000e` data plane (milestone 494 (a driver for the network card a PC
    // actually has)); the paragraph above is the reason for its instructions.
    user_mode_runtime::mapped_window::doorbell_barrier();
}

/// The server's whole mutable state: the two ring cursors and a command id.
struct Plane {
    handoff: Handoff,
    sq: SqState,
    cq: CqState,
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
    /// doorbell. Every one of those is a volatile access through a window the spawner installed;
    /// every index and offset in it came out of `crates/non_volatile_memory_express`.
    ///
    /// `Err(status)` carries the controller's status field, or [`u16::MAX`] for a completion that
    /// never arrived, which this server cannot distinguish from a fatal controller error because
    /// `CSTS` is not in its address space.
    fn transact(&mut self, cmd: Command) -> Result<(), u16> {
        let expect_cid = (cmd.0[0] >> 16) as u16;

        let slot = self.sq.push();
        DATA.write(IO_SQ_OFF + slot as u64 * 64, cmd.0);
        // Publish the command before the doorbell: the controller is another observer.
        barrier();
        BELLS.w32(
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
            let c = Completion::from_dwords(DATA.read::<[u32; 4]>(cqe));
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
        BELLS.w32(
            non_volatile_memory_express::doorbell(
                IO_QID,
                Doorbell::CompletionHead,
                self.handoff.dstrd,
            ) - PAGE,
            new_head as u32,
        );
        self.sq.note_head(c.sq_head);

        // The one-at-a-time discipline makes any other cid a protocol violation. Answering EIO
        // rather than panicking, because this is a server and a client is waiting on it: a
        // process that traps here takes the disk down for everyone holding the endpoint.
        if c.cid != expect_cid {
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
        let data_phys = self.handoff.data_plane_phys + TRANSFER_OFF + page * PAGE;
        let cid = self.next_cid();
        let cmd = self
            .handoff
            .transfer_command(cid, NSID, block, PAGE, data_phys, PAGE, write)
            // `None` means the block is outside the namespace or the arithmetic would not close;
            // either way it is the caller's request that is wrong, and the crate refused to build
            // a command rather than let the controller answer for it.
            .ok_or(u16::MAX)?;
        self.transact(cmd)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(arg0: u64, arg1: u64, arg2: u64) -> ! {
    // The three words are the whole of what this process was told; a word that cannot describe a
    // controller it could serve is a wiring bug, reported rather than acted on.
    let Some(handoff) = Handoff::unpack([arg0, arg1, arg2]) else {
        send(READY, STEP_BAD_HANDOFF, arg0, 0);
        exit();
    };
    let mut plane = Plane {
        handoff,
        sq: SqState::new(handoff.entries),
        cq: CqState::new(handoff.entries),
        cid: 0,
        flushes: 0,
    };

    // **Test-only: the DMA-escape attacker role**, compiled only with `confinement_attackers`,
    // which only `cargo xtask test` turns on. `start_dma_escape` in
    // `kernel/src/user/non_volatile_memory_express_service.rs` writes `[ESCAPE_MAGIC, victim_phys]`
    // into the transfer buffer's first two `u64`s before this runs; a real boot's is zeroed. Read
    // before the readiness transfer, which reuses transfer page 0. See
    // notes/non-volatile-memory-express/the-dma-escape-test.md.
    #[cfg(feature = "confinement_attackers")]
    if DATA.r64(TRANSFER_OFF) == non_volatile_memory_express::ESCAPE_MAGIC {
        let victim = DATA.r64(TRANSFER_OFF + 8);
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
            // A plain SEND or a SEND_CAP on a CALL-only contract: nothing to answer, and a
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
/// [`Handoff::transfer_command`]'s refusal and hands [`Command::read`]/[`Command::write`] a PRP of
/// its own, `victim`, a physical address outside this process's DMA window: a `READ` would
/// overwrite a frame this server was never granted, a `WRITE` would leak `victim` onto the disk.
/// With the IOMMU confining the controller to its own region both fault, and the kernel test reads
/// `victim`'s untouched canary and the IOMMU's fault record. **This process holds no name for
/// `victim`**: a raw number out of shared memory, the authority an escaping driver would fabricate.
#[cfg(feature = "confinement_attackers")]
fn run_dma_escape(plane: &mut Plane, victim: u64) -> ! {
    let blocks = plane.handoff.blocks_per;
    // disk -> victim: the controller would overwrite a frame outside the region.
    let rd = {
        let cid = plane.next_cid();
        plane.transact(Command::read(cid, NSID, 0, blocks, victim, 0))
    };
    // victim -> disk: the controller would read a frame outside the region.
    let wr = {
        let cid = plane.next_cid();
        plane.transact(Command::write(cid, NSID, 0, blocks, victim, 0))
    };
    // Word 1 the read-escape status, word 2 the write-escape status: each an Err from a confined
    // controller (its status, or u16::MAX for no completion), and for the transcript only. The
    // kernel test's proof is the IOMMU fault and the untouched canary.
    let code = |r: Result<(), u16>| r.err().unwrap_or(0) as u64;
    send(READY, code(rd), code(wr), 0);
    exit();
}

/// `count` contiguous blocks, one command each. See this module's `BUGS` for why it is not one
/// command: `non_volatile_memory_express::prp_pair` refuses a transfer needing a PRP list, and a multi-page one does.
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

user_mode_runtime::panic_handler!();
