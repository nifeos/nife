//! **The JH7110 clock and reset server** (milestone 1062 (the device control planes leave the
//! kernel), step 2): the one process that writes the chip's shared clock and reset windows.
//!
//! It holds the first page of each window its spawner mapped (STG, SYS and AON device-typed, the
//! SYS syscon read-only) and one endpoint, slot [`REQUEST_SLOT`]. A client's capability on that
//! endpoint carries a badge, its device's register base physical address, and the server answers
//! for that device alone: `jh7110_clock_and_reset::device` holds what each badge brings up, and
//! `jh7110_clock_and_reset_contract` the wire. No request word names a device, so a client can
//! neither name nor reach another device's clocks or resets (§256 (a server that keeps windows for
//! many clients scopes each by the caller's badge)). No operation turns a device off (§304 (who
//! may turn a device off)). See notes/jh7110-clock-and-reset.md.
//!
//! # BUGS
//!
//! - The reset poll is a count of reads, not a duration, as the kernel's own walk is.
//! - A server that dies is not restarted. Nothing it did is undone by its death, so a client whose
//!   call fails on a dead server keeps whatever clocks it already had.
//!
//! Name: provisional.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107).
#![allow(missing_docs)]
#![no_main]

use driver_process_contract::jh7110_clock_and_reset::{
    ALWAYS_ON_KIND, REQUEST_SLOT, SYSTEM_CONTROLLER_KIND, SYSTEM_KIND, SYSTEM_TOP_GROUP_KIND,
};
use jh7110_clock_and_reset::device::{Device, RateRefusal, USED_WINDOW_BYTES, WindowName, Windows};
use jh7110_clock_and_reset_contract::{self as wire, Status, Summary};
use startup_page_contract::{StartupPage, WindowKind};
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{receive_request, reply};

/// How many times a bring-up reads a reset's status word before it counts the reset held: the
/// kernel walk's bound, far longer than the hardware needs and far shorter than a hung client.
const POLL_LIMIT: u32 = 1_000_000;

/// The windows the startup page lists, each the first page of its block.
struct Held {
    windows: [Option<MappedWindow>; 4],
}

/// Where each window sits in [`Held::windows`], and its kind on the startup page.
const KINDS: [(WindowName, WindowKind); 4] = [
    (WindowName::SystemTopGroup, SYSTEM_TOP_GROUP_KIND),
    (WindowName::System, SYSTEM_KIND),
    (WindowName::AlwaysOn, ALWAYS_ON_KIND),
    (WindowName::SystemController, SYSTEM_CONTROLLER_KIND),
];

fn slot(window: WindowName) -> usize {
    KINDS
        .iter()
        .position(|(w, _)| *w == window)
        .expect("every window has a kind")
}

impl Held {
    /// The windows this process's spawner listed. A window it did not list is one this machine's
    /// tree named nowhere, and every device drawing on it is a device this server does not serve.
    fn from_startup_page() -> Held {
        // SAFETY: the spawner (`kernel/src/user/jh7110_clock_and_reset_service.rs`) writes a
        // startup page at its fixed address before `_start` runs and never changes it.
        let page = unsafe { StartupPage::mapped() }.expect("no startup page");
        let windows = KINDS.map(|(_, kind)| {
            page.window(kind).map(|w| {
                assert!(u64::from(w.pages) * address_space_map::PAGE >= USED_WINDOW_BYTES);
                // SAFETY: the spawner maps every window it lists for the life of the process, the
                // syscon read-only and the three domains read/write, and this server writes only
                // what `Windows::write` is handed, which is never the syscon.
                unsafe { MappedWindow::new(w.virtual_address, USED_WINDOW_BYTES) }
            })
        });
        Held { windows }
    }

    fn window(&self, window: WindowName) -> &MappedWindow {
        self.windows[slot(window)]
            .as_ref()
            .expect("a device is served only where its windows are held")
    }
}

impl Windows for Held {
    fn is_held(&self, window: WindowName) -> bool {
        self.windows[slot(window)].is_some()
    }

    fn read(&mut self, window: WindowName, offset: u64) -> u32 {
        self.window(window).r32(offset)
    }

    fn write(&mut self, window: WindowName, offset: u64, value: u32) {
        assert_ne!(
            window,
            WindowName::SystemController,
            "the syscon is read-only"
        );
        self.window(window).w32(offset, value);
    }
}

/// **Answer one request** from the device `badge` names: the status and reply word 1.
fn answer(held: &mut Held, badge: u64, w0: u64, w1: u64) -> (Status, u64) {
    let Some(device) = Device::at(badge).filter(|d| d.is_served_by(held)) else {
        return (Status::UnknownDevice, 0);
    };
    match wire::operation(w0) {
        wire::BRING_UP => {
            let c = device.bring_up(held, POLL_LIMIT);
            let summary = Summary {
                clocks_in_plan: c.clocks_in_plan,
                clocks_running: c.clocks_running,
                resets_in_plan: c.resets_in_plan,
                resets_released: c.resets_released,
                already_up: c.already_up,
                refused: c.refused,
            };
            (summary.status(), summary.pack())
        }
        wire::READ_CLOCK => device
            .read_clock(held, wire::index(w0))
            .map_or((Status::IndexOutOfRange, 0), |w| {
                (Status::Done, u64::from(w))
            }),
        wire::READ_RESET => device
            .read_reset(held, wire::index(w0))
            .map_or((Status::IndexOutOfRange, 0), |w| {
                (Status::Done, u64::from(w))
            }),
        wire::SET_RATE => match device.set_rate(held, w1) {
            Ok(rate) => (Status::Done, rate.hertz),
            Err(RateRefusal::NoRatedClock) => (Status::NoRatedClock, 0),
            Err(RateRefusal::NoRateAtOrBelow) => (Status::NoRateAtOrBelow, 0),
        },
        // `VERSION_QUERY` among them, until its reply exists (the contract crate's BUGS).
        _ => (Status::UnservedOperation, 0),
    }
}

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    let mut held = Held::from_startup_page();
    loop {
        let request = receive_request(REQUEST_SLOT);
        let Some(capability) = request.delivered.into_reply() else {
            // A plain SEND on a CALL-only wire: nobody waits for an answer.
            continue;
        };
        let (status, word) = answer(&mut held, request.badge, request.w0, request.w1);
        reply(capability, status.to_word(), word);
    }
}

user_mode_runtime::panic_handler!();
