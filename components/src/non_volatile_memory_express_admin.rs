//! **The NVMe admin process**: the `non_volatile_memory_express` image's admin role, §86 (whether
//! an NVMe driver can leave the kernel) as ruled. It owns the controller: reset, the admin rings,
//! enable, IDENTIFY, and the one I/O queue pair, built inside the DMA run the kernel confined the
//! controller to before this process existed. It then grants the data plane the doorbell page and
//! the run's data-plane pages, reports to the kernel, and waits to retire the controller. All the
//! arithmetic is `crates/non_volatile_memory_express`'s. A `#[path]` module of the one program, as
//! `net_stack`'s transports are. See notes/non-volatile-memory-express.md.

//! # Capability contract
//!
//! Held, the complete list: `administration_slots::REPORT`, `GRANTS` and `CONTROL`, three endpoints; the
//! doorbell page as a `DeviceFrame` and the data plane's pages as a `PageFrame` run, both with
//! `GRANT`, both delegated narrowed to the data plane and never mapped here; BAR0's register and
//! doorbell pages, [`REGISTERS`]; and the run's admin pages, [`ADMIN_QUEUES`], both mapped by the
//! spawner and named by no capability. Denied: the pages of BAR0 past the doorbell page, an
//! `Interrupt`, and everything else.

//! # BUGS
//!
//! - It polls: no interrupt vector, and nothing would confine one aimed from userspace (§86).
//! - Without an IOMMU nothing bounds the queues it creates; nife trusts it there (§86).
//! - A refused bring-up exits without resetting the controller.
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace, for the
//! module and its file. `non_volatile_memory_express_admin` as a program name is 33 bytes, past the
//! archive's 32-byte `NAME_LEN`, which is one reason the admin process is a role of the existing
//! program.

use driver_process_contract::non_volatile_memory_express::{
    ADMINISTRATION_COMPLETION_QUEUE_PAGE, ADMINISTRATION_PAGES, ADMINISTRATION_QUEUE_KIND,
    ADMINISTRATION_SUBMISSION_QUEUE_PAGE, AdministrationReport, BringUpError, DATA_PLANE_GRANT,
    DOORBELL_PAGE_GRANT, Handoff, IDENTIFY_PAGE, IO_COMPLETION_QUEUE_PAGE,
    IO_SUBMISSION_QUEUE_PAGE, MAXIMUM_DOORBELL_STRIDE, REGISTER_KIND, REGISTER_PAGES, RETIRE,
    administration_slots,
};
use non_volatile_memory_express::{
    Capabilities, Command, Completion, CompletionQueue, Doorbell, IdentifyNamespace,
    SubmissionQueue, registers,
};
use startup_page_contract::StartupWindow;
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::{exit, receive_request, reply, send, send_capability};

use crate::{IO_QID, NSID, SPIN_BOUND, barrier};

/// One filesystem block, the unit IDENTIFY's geometry is checked against.
const BLOCK_SIZE: u64 = filesystem_contract::blk::BLOCK_SIZE as u64;

/// A window from the startup page, as a bounds-checked [`MappedWindow`] of the length its static
/// declares.
fn mapped(window: &StartupWindow) -> MappedWindow {
    // SAFETY: each `StartupWindow` here was declared on the spawner's promise that it maps the
    // window for the life of the process, which is what `MappedWindow::new` asks.
    unsafe { MappedWindow::new(window.virtual_address(), window.length_bytes()) }
}

/// Sixteen entries per ring: one page holds 64 (submission) or 256 (completion), but the data
/// plane completes each command before submitting the next, so depth buys nothing and a small
/// ring keeps the wrap (where the phase discipline earns its keep) inside every test run.
const ENTRIES: u16 = 16;

/// BAR0's register page and doorbell page, device-typed.
// SAFETY: the spawner (`kernel/src/user/non_volatile_memory_express_service.rs`'s `start_admin`)
// lists this window on the startup page and maps all of it device-typed before `_start` runs.
static REGISTERS: StartupWindow =
    unsafe { StartupWindow::new(REGISTER_KIND, REGISTER_PAGES * PAGE) };

/// The admin submission ring, the admin completion ring and the IDENTIFY page.
// SAFETY: as [`REGISTERS`], mapped read/write normal memory.
static ADMIN_QUEUES: StartupWindow =
    unsafe { StartupWindow::new(ADMINISTRATION_QUEUE_KIND, ADMINISTRATION_PAGES * PAGE) };

/// How many bytes of the IDENTIFY namespace structure the parse reads: through the LBA format
/// table, which ends at byte 384.
const IDENTIFY_BYTES: usize = 384;

/// The admin plane's state: the run's physical base, the stride from `CAP`, the two admin ring
/// cursors and a command id.
struct Controller {
    /// The DMA run's **physical** base, which queue creation and IDENTIFY name.
    dma: u64,
    dstrd: u32,
    admin_sq: SubmissionQueue,
    admin_cq: CompletionQueue,
    /// Echoed back in completions and checked there; monotonic and wrapping, so a stale
    /// completion cannot impersonate a fresh one within 65536 commands.
    cid: u16,
}

impl Controller {
    fn reg32(&self, off: u64) -> u32 {
        mapped(&REGISTERS).r32(off)
    }

    fn wr32(&self, off: u64, v: u32) {
        mapped(&REGISTERS).w32(off, v)
    }

    /// The spec allows 64-bit registers to be accessed as aligned 32-bit halves (NVMe 1.4 §3.1),
    /// which keeps the accessor surface one width on every ISA.
    fn reg64(&self, off: u64) -> u64 {
        self.reg32(off) as u64 | (self.reg32(off + 4) as u64) << 32
    }

    fn wr64(&self, off: u64, v: u64) {
        self.wr32(off, v as u32);
        self.wr32(off + 4, (v >> 32) as u32);
    }

    fn next_cid(&mut self) -> u16 {
        self.cid = self.cid.wrapping_add(1);
        self.cid
    }

    /// Wait for `CSTS.RDY` to become `want`, bounded. `CFS` short-circuits: a controller that has
    /// declared a fatal error will never become ready and the caller should hear which happened.
    fn wait_ready(&self, want: bool) -> Result<(), BringUpError> {
        for _ in 0..SPIN_BOUND {
            let csts = self.reg32(registers::CONTROLLER_STATUS_REGISTER);
            if csts & non_volatile_memory_express::CONTROLLER_STATUS_FATAL != 0 {
                return Err(BringUpError::ControllerFatal);
            }
            if (csts & non_volatile_memory_express::CONTROLLER_STATUS_READY != 0) == want {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(BringUpError::ControllerTimeout)
    }

    /// Submit one **admin** command and poll its completion: the copy into the ring, the publish
    /// barrier, the tail doorbell, the phase-gated poll, the head doorbell. The data plane rides
    /// the identical discipline on the I/O queue.
    fn transact(&mut self, cmd: Command) -> Result<(), BringUpError> {
        let expect_cid = (cmd.0[0] >> 16) as u16;
        let queues = mapped(&ADMIN_QUEUES);
        let slot = self
            .admin_sq
            .push()
            .expect("one command in flight per queue, so the admin ring cannot be full");
        queues.write(
            ADMINISTRATION_SUBMISSION_QUEUE_PAGE * PAGE + slot as u64 * 64,
            cmd.0,
        );
        // The entry before the doorbell that announces it; the same barrier orders the poll
        // reads below.
        barrier();
        self.wr32(
            non_volatile_memory_express::doorbell(0, Doorbell::SubmissionTail, self.dstrd),
            self.admin_sq.tail() as u32,
        );

        let cqe = ADMINISTRATION_COMPLETION_QUEUE_PAGE * PAGE + self.admin_cq.head() as u64 * 16;
        let mut done: Option<Completion> = None;
        for _ in 0..SPIN_BOUND {
            let c = Completion::from_dwords(queues.read::<[u32; 4]>(cqe));
            if self.admin_cq.is_owned(&c) {
                done = Some(c);
                break;
            }
            if self.reg32(registers::CONTROLLER_STATUS_REGISTER)
                & non_volatile_memory_express::CONTROLLER_STATUS_FATAL
                != 0
            {
                return Err(BringUpError::ControllerFatal);
            }
            core::hint::spin_loop();
        }
        let c = done.ok_or(BringUpError::CompletionTimeout)?;
        // Order the completion's phase read before the payload reads that follow (the identify
        // parse).
        barrier();

        let new_head = self.admin_cq.pop();
        self.wr32(
            non_volatile_memory_express::doorbell(0, Doorbell::CompletionHead, self.dstrd),
            new_head as u32,
        );
        // A head the controller cannot have reached, or a completion for another command, is a
        // protocol violation: refuse the controller rather than misattribute a completion.
        if self.admin_sq.record_head(c.submission_queue_head).is_err() || c.command_id != expect_cid
        {
            return Err(BringUpError::CompletionTimeout);
        }
        if c.status != 0 {
            return Err(BringUpError::Command(c.status));
        }
        Ok(())
    }

    /// **Bring the controller from reset to ready to serve I/O**: check `CAP`, reset, the admin
    /// queues, enable, identify the namespace, create the I/O queue pair in the data plane's
    /// pages. Returns the namespace it identified.
    fn bring_up(&mut self) -> Result<IdentifyNamespace, BringUpError> {
        let capabilities = Capabilities(self.reg64(registers::CONTROLLER_CAPABILITIES_REGISTER));
        // The doorbell stride is also checked in `Handoff::unpack`; here it fails at bring-up,
        // naming the controller, not in a process that cannot reach its doorbells.
        if capabilities.maximum_queue_entries() < ENTRIES as u32
            || capabilities.minimum_page_size() > PAGE
            || capabilities.doorbell_stride() > MAXIMUM_DOORBELL_STRIDE
        {
            return Err(BringUpError::UnsupportedController);
        }
        self.dstrd = capabilities.doorbell_stride();

        // A controller that was already enabled (a warm reboot, or a previous test) must be taken
        // down before the admin queue registers may change; RDY follows EN in both directions.
        if self.reg32(registers::CONTROLLER_STATUS_REGISTER)
            & non_volatile_memory_express::CONTROLLER_STATUS_READY
            != 0
        {
            self.wr32(registers::CONTROLLER_CONFIGURATION_REGISTER, 0);
            self.wait_ready(false)?;
        }

        // The admin queue pair, by register: attributes (both depths, 0-based), then the two
        // page-aligned ring bases. The rings are zeroed pages, which is what makes the phase
        // discipline sound: the controller's first lap writes phase 1 into memory that reads 0.
        self.wr32(
            registers::ADMIN_QUEUE_ATTRIBUTES_REGISTER,
            (ENTRIES as u32 - 1) << 16 | (ENTRIES as u32 - 1),
        );
        self.wr64(
            registers::ADMIN_SUBMISSION_QUEUE_BASE_PHYSICAL_ADDRESS_REGISTER,
            self.dma + ADMINISTRATION_SUBMISSION_QUEUE_PAGE * PAGE,
        );
        self.wr64(
            registers::ADMIN_COMPLETION_QUEUE_BASE_PHYSICAL_ADDRESS_REGISTER,
            self.dma + ADMINISTRATION_COMPLETION_QUEUE_PAGE * PAGE,
        );
        self.wr32(
            registers::CONTROLLER_CONFIGURATION_REGISTER,
            non_volatile_memory_express::enabled_controller_configuration(),
        );
        self.wait_ready(true)?;

        // IDENTIFY the namespace: its block count and LBA format are the two facts the block
        // arithmetic stands on, and refusing an exotic format here beats corrupting it later.
        let cid = self.next_cid();
        self.transact(Command::identify(
            cid,
            non_volatile_memory_express::CONTROLLER_OR_NAMESPACE_STRUCTURE_NAMESPACE,
            NSID,
            self.dma + IDENTIFY_PAGE * PAGE,
        ))?;
        let mut data = [0u8; IDENTIFY_BYTES];
        mapped(&ADMIN_QUEUES).copy_to_slice(IDENTIFY_PAGE * PAGE, &mut data);
        let refused = BringUpError::UnsupportedNamespace {
            logical_block_size_shift: non_volatile_memory_express::logical_block_format_size_shift(
                &data,
            )
            .unwrap_or(0),
            blocks: u64::from_le_bytes(data[0..8].try_into().unwrap_or([0; 8])),
        };
        let namespace =
            non_volatile_memory_express::parse_identify_namespace(&data).ok_or(refused)?;
        if namespace.blocks == 0 || namespace.blocks_per_transfer(BLOCK_SIZE).is_none() {
            return Err(refused);
        }

        // Completion queue first: the submission queue names it, so the other order is an Invalid
        // Queue Identifier. Both rings sit in the data plane's pages.
        let data_plane = self.dma + ADMINISTRATION_PAGES * PAGE;
        let cid = self.next_cid();
        self.transact(Command::create_io_completion_queue(
            cid,
            IO_QID,
            ENTRIES,
            data_plane + IO_COMPLETION_QUEUE_PAGE * PAGE,
        ))?;
        let cid = self.next_cid();
        self.transact(Command::create_io_submission_queue(
            cid,
            IO_QID,
            ENTRIES,
            IO_QID,
            data_plane + IO_SUBMISSION_QUEUE_PAGE * PAGE,
        ))?;
        Ok(namespace)
    }

    /// Take the controller down: `CC.EN` cleared, `CSTS.RDY` followed.
    fn retire(&self) -> Result<(), BringUpError> {
        self.wr32(registers::CONTROLLER_CONFIGURATION_REGISTER, 0);
        self.wait_ready(false)
    }
}

/// **The admin role**, `dma` the run's physical base the spawn passed. Never returns.
pub(crate) fn run(dma: u64) -> ! {
    let mut controller = Controller {
        dma,
        dstrd: 0,
        admin_sq: SubmissionQueue::new(ENTRIES),
        admin_cq: CompletionQueue::new(ENTRIES),
        cid: 0,
    };
    let namespace = match controller.bring_up() {
        Ok(namespace) => namespace,
        Err(why) => {
            let [w0, w1, w2] = AdministrationReport::Refused(why).pack();
            send(administration_slots::REPORT, w0, w1, w2);
            exit();
        }
    };
    // `bring_up` refused a namespace this returns `None` for.
    let Some(blocks_per_transfer) = namespace.blocks_per_transfer(BLOCK_SIZE) else {
        exit();
    };
    let report = AdministrationReport::Ready {
        size_bytes: namespace.capacity_bytes(),
        blocks_per_transfer,
    };
    let [w0, w1, w2] = report.pack();
    send(administration_slots::REPORT, w0, w1, w2);

    // The data plane's whole world from this process: three words, then the doorbell page and its
    // pages of the run, each narrowed so it cannot pass them on.
    let handoff = Handoff {
        dstrd: controller.dstrd,
        blocks_per_transfer,
        entries: ENTRIES,
        data_plane_physical_address: dma + ADMINISTRATION_PAGES * PAGE,
        size_bytes: namespace.capacity_bytes(),
    };
    let [w0, w1, w2] = handoff.pack();
    send(administration_slots::GRANTS, w0, w1, w2);
    send_capability(
        administration_slots::GRANTS,
        administration_slots::DOORBELL_PAGE,
        abi::rights::WRITE,
        DOORBELL_PAGE_GRANT,
    );
    send_capability(
        administration_slots::GRANTS,
        administration_slots::DATA_PLANE,
        abi::rights::READ | abi::rights::WRITE,
        DATA_PLANE_GRANT,
    );

    // Hold the controller until asked to give it up.
    loop {
        let request = receive_request(administration_slots::CONTROL);
        let Some(to) = request.delivered.into_reply() else {
            continue;
        };
        if request.w0 != RETIRE {
            reply(to, 1, 0);
            continue;
        }
        let retired = controller.retire().is_ok();
        reply(to, u64::from(!retired), 0);
        if retired {
            exit();
        }
    }
}
