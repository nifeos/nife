//! **The JH7110 PMIC route** (milestone 1062 (the device control planes leave the kernel), step 5;
//! §251 (restarting the machine is a kernel object the progenitor hands out), "A board's PMIC route
//! runs in userspace"): the one process that may write the board's power controller.
//!
//! It holds the first page of the I2C controller the power controller hangs off, device-typed, and
//! one endpoint, slot [`REQUEST_SLOT`], whose only client is `reboot`. Its spawner brought the
//! controller's clocks and reset up through the clock and reset server, and passed the target and
//! the standard-mode timing in the spawn words (`driver_process_contract::power_management_chip_route`).
//! One request, `RESET`: read the reset register, write it back with the reset bit set, then wait
//! [`WAIT_NANOSECONDS`] for the board to go. A board that resets never answers; every other outcome
//! is a `Status` the caller prints before it asks the kernel's reboot object instead. See
//! notes/board-reboot.md.
//!
//! # BUGS
//!
//! - A route that stops answering hangs `reboot`: the call has no timeout of its own, so the wait
//!   is the route's, and a route that dies mid-request leaves the caller blocked.
//! - The I2C bounds are counts of register reads, not durations (the crate's BUGS).
//! - The wait spins on the monotonic clock, yielding, rather than sleeping on a timer.
//!
//! Name: provisional.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates: one lint
// policy)'s ratchet tracks (§107 (`missing_docs` moves to `workspace.lints.rust`, opt-out rather
// than opt-in)).
#![allow(missing_docs)]
#![no_main]

use designware_inter_integrated_circuit::{Controller, Registers, Timing, with_bit};
use driver_process_contract::power_management_chip_route::{
    self as wire, CONTROLLER_KIND, REQUEST_SLOT, Status, Target,
};
use startup_page_contract::StartupPage;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{receive_request, reply};

/// How many register reads each I2C wait may take: the kernel route's bound, far longer than a
/// two-byte transfer at 100 kHz needs.
const POLL_LIMIT: u32 = 1_000_000;

/// How long the route waits, after the write, for the board to reset before it answers
/// [`Status::StillRunning`]. The AXP15060 acts within milliseconds; two seconds keeps a slow one
/// from being called a failure, and a caller from waiting long on one that ignored the write.
const WAIT_NANOSECONDS: u64 = 2_000_000_000;

/// The bytes of the controller page this route touches: every register up to `COMPONENT_TYPE`.
const USED_BYTES: u64 = 0x100;

/// The controller page, as the I2C crate's register file.
struct Page(MappedWindow);

impl Registers for Page {
    fn read(&mut self, offset: u32) -> u32 {
        self.0.r32(u64::from(offset))
    }

    fn write(&mut self, offset: u32, value: u32) {
        self.0.w32(u64::from(offset), value);
    }
}

/// **Reset the board**, or say why it is still running: the status and reply word 1.
fn reset(controller: &mut Controller<Page>, timing: &Timing, target: Target) -> (Status, u64) {
    let value = match controller.read_register(timing, target.address, target.register) {
        Ok(value) => value,
        Err(f) => return (Status::ReadFailed, wire::pack_failure(f.kind(), f.word())),
    };
    let set = with_bit(value, target.bit);
    if let Err(f) = controller.write_register(timing, target.address, target.register, set) {
        return (Status::WriteFailed, wire::pack_failure(f.kind(), f.word()));
    }
    let start = user_mode_runtime::monotonic_nanoseconds();
    while user_mode_runtime::monotonic_nanoseconds().wrapping_sub(start) < WAIT_NANOSECONDS {
        user_mode_runtime::yield_now();
    }
    (Status::StillRunning, u64::from(set))
}

user_mode_runtime::program_entry!(start);

fn start(arg0: u64, arg1: u64, arg2: u64) -> ! {
    let target = Target::unpack(arg0).expect("the spawner packs a valid target");
    let timing = wire::unpack_timing([arg1, arg2]);
    // SAFETY: the spawner (`kernel/src/user/power_management_chip_route_service.rs`) writes a
    // startup page at its fixed address before `_start` runs and never changes it.
    let page = unsafe { StartupPage::mapped() }.expect("no startup page");
    let window = page
        .window(CONTROLLER_KIND)
        .expect("the spawner lists the controller page");
    assert!(u64::from(window.pages) * address_space_map::PAGE >= USED_BYTES);
    // SAFETY: the spawner maps the controller page device-typed for the life of the process, and
    // nothing else in this address space touches it.
    let registers = Page(unsafe { MappedWindow::new(window.virtual_address, USED_BYTES) });
    let mut controller = Controller::new(registers, POLL_LIMIT);
    loop {
        let request = receive_request(REQUEST_SLOT);
        let Some(capability) = request.delivered.into_reply() else {
            // A plain SEND on a CALL-only wire: nobody waits for an answer.
            continue;
        };
        let (status, word) = if wire::operation(request.w0) == wire::RESET {
            reset(&mut controller, &timing, target)
        } else {
            (Status::UnknownOperation, 0)
        };
        reply(capability, status.to_word(), word);
    }
}

user_mode_runtime::panic_handler!();
