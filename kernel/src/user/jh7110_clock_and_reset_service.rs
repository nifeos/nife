//! **The JH7110 clock and reset server's wiring** (milestone 1062 (the device control planes leave
//! the kernel), step 2), and the calls the kernel's own starters make to it.
//!
//! On a machine whose tree names a JH7110 clock and reset window, [`ensure`] spawns
//! `components/src/jh7110_clock_and_reset_server.rs` once per boot with the first page of each
//! window the tree named (STG, SYS and AON device-typed, the SYS syscon read-only) and the receive
//! side of one endpoint. A starter then calls it with the badge of the device it is about to start,
//! the device's register base physical address (§320 (the JH7110 clock and reset wire)), through
//! `sched::ipc_call_badged`. The kernel's starters are the server's only clients today. See
//! notes/jh7110-clock-and-reset.md.
//!
//! Reached only on riscv64, because the JH7110 is a RISC-V chip and no other machine nife boots
//! has one. QEMU's `virt` names no window, so [`ensure`] answers `None` on every machine CI boots,
//! and on aarch64 and `x86_64` too.
//!
//! # BUGS
//!
//! - A server that dies is not restarted, and a call to it then answers `None` for the boot.
//! - The reboot path's I2C5 bring-up and the soak's watchdog still call
//!   `drivers::jh7110_clock_and_reset` directly: milestone 1062's step 5 moves the first to the PMIC
//!   route, and the second needs the kernel's own driver, since the soak's supervisor pets it.
//!
//! Name: provisional.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use driver_process_contract::jh7110_clock_and_reset::{
    ALWAYS_ON_KIND, ALWAYS_ON_VIRTUAL_ADDRESS, REQUEST_SLOT, SYSTEM_CONTROLLER_KIND,
    SYSTEM_CONTROLLER_VIRTUAL_ADDRESS, SYSTEM_KIND, SYSTEM_TOP_GROUP_KIND,
    SYSTEM_TOP_GROUP_VIRTUAL_ADDRESS, SYSTEM_VIRTUAL_ADDRESS,
};
use jh7110_clock_and_reset_contract::{self as wire, Status, Summary};

use super::*;
use crate::capability_object::{Rights, rendezvous_capability};

/// Whether this boot has spawned the server, and the endpoint its clients call. One writer: the
/// boot or test thread that starts the first JH7110 device, the single-writer argument
/// `entropy_service::WIRED` makes.
static WIRED: AtomicBool = AtomicBool::new(false);
static REQUEST: AtomicU64 = AtomicU64::new(0);

/// **What a bring-up answered**: the server's status and the summary it sends whatever the status.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BroughtUp {
    /// The status, [`Status::Done`] when every clock runs and every reset is released.
    pub status: Status,
    /// What the server counted.
    pub summary: Summary,
}

impl BroughtUp {
    /// Whether the device's clocks run and its resets are released.
    ///
    /// Name: provisional.
    pub(crate) fn is_up(&self) -> bool {
        self.status == Status::Done
    }
}

/// **The server's endpoint, spawning the server on the first call**, or `None` on a machine whose
/// tree names no JH7110 clock and reset window, or whose archive has no server program.
///
/// Name: provisional.
pub(crate) fn ensure() -> Option<crate::sched::RendezvousId> {
    if WIRED.load(Ordering::Acquire) {
        return Some(REQUEST.load(Ordering::Relaxed));
    }
    let request = start()?;
    REQUEST.store(request, Ordering::Relaxed);
    WIRED.store(true, Ordering::Release);
    Some(request)
}

/// Spawn the server over whichever windows this machine's tree named.
fn start() -> Option<crate::sched::RendezvousId> {
    let system_top_group = crate::memory::jh7110_clock_and_reset();
    let system = crate::memory::jh7110_sys_window();
    let always_on = crate::memory::jh7110_ethernet().map(|e| e.aon);
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_device(),
    }; 4];
    let mut windows = [startup_page_contract::Window {
        kind: SYSTEM_TOP_GROUP_KIND,
        virtual_address: 0,
        pages: 0,
    }; 4];
    let mut count = 0;
    let mut add = |va: u64, phys: u64, flags: Flags, kind| {
        maps[count] = Mapping { va, phys, flags };
        windows[count] = startup_page_contract::Window {
            kind,
            virtual_address: va,
            pages: 1,
        };
        count += 1;
    };
    for (window, va, kind) in [
        (
            system_top_group,
            SYSTEM_TOP_GROUP_VIRTUAL_ADDRESS,
            SYSTEM_TOP_GROUP_KIND,
        ),
        (system, SYSTEM_VIRTUAL_ADDRESS, SYSTEM_KIND),
        (always_on, ALWAYS_ON_VIRTUAL_ADDRESS, ALWAYS_ON_KIND),
    ] {
        if let Some(w) = window {
            add(va, w.base_physical_address, Flags::user_device(), kind);
        }
    }
    // The syscon has no discovery of its own: the SYS window's presence says the machine is a
    // JH7110, and both published trees put the syscon at the one address.
    if system.is_some() {
        add(
            SYSTEM_CONTROLLER_VIRTUAL_ADDRESS,
            jh7110_clock_and_reset::SYSTEM_CONTROLLER_BASE_PHYSICAL_ADDRESS,
            Flags::user_device_read_only(),
            SYSTEM_CONTROLLER_KIND,
        );
    }
    if count == 0 {
        return None;
    }
    let image = crate::user::program("jh7110_clock_and_reset_server")?;
    let request = crate::sched::create_rendezvous();
    const _: () = assert!(REQUEST_SLOT == 0);
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                // Slot 0: receive every client's calls, each with its badge.
                grants: &[rendezvous_capability(request, Rights::READ)],
                maps: &maps[..count],
                windows: &windows[..count],
            },
        )
    })
    .expect("could not spawn the JH7110 clock and reset server");
    Some(request)
}

/// **One call to the server as the device at `device`**, or `None` when there is no server or the
/// reply is no status the server sends.
fn call(device: u64, w0: u64, w1: u64) -> Option<(Status, u64)> {
    let request = ensure()?;
    let [status, word, _] = crate::sched::ipc_call_badged(request, [w0, w1], device);
    Some((Status::from_word(status)?, word))
}

/// **Bring up the device whose registers start at `device`.** `None` when there is no server.
///
/// Name: provisional.
pub(crate) fn bring_up(device: u64) -> Option<BroughtUp> {
    let (status, word) = call(device, wire::request(wire::BRING_UP, 0), 0)?;
    Some(BroughtUp {
        status,
        summary: Summary::unpack(word).unwrap_or_default(),
    })
}

/// **The `index`th clock word of the device at `device`**, as the server read it. `None` when there
/// is no server or the server refused.
///
/// Name: provisional.
#[cfg_attr(not(feature = "storage_bench"), allow(dead_code))] // the storage bench prints them
pub(crate) fn read_clock(device: u64, index: u8) -> Option<u32> {
    match call(device, wire::request(wire::READ_CLOCK, index), 0)? {
        (Status::Done, word) => u32::try_from(word).ok(),
        _ => None,
    }
}

/// **Run the rated clock of the device at `device` at no more than `most_hertz`**: the rate it now
/// runs at, or the status that refused it. `None` when there is no server.
///
/// Name: provisional.
#[cfg_attr(not(feature = "storage_bench"), allow(dead_code))] // the storage bench is its caller
pub(crate) fn set_rate(device: u64, most_hertz: u64) -> Option<Result<u64, Status>> {
    match call(device, wire::request(wire::SET_RATE, 0), most_hertz)? {
        (Status::Done, hertz) => Some(Ok(hertz)),
        (status, _) => Some(Err(status)),
    }
}
