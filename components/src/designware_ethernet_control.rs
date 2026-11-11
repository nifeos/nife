//! **The JH7110 Ethernet port's control process** (milestone 1062 (the device control planes leave
//! the kernel), step 3): the GMAC's bring-up, out of the kernel. It selects RGMII in the always-on
//! syscon, then runs `designware_ethernet::bring_up` over the controller's first two register
//! pages and the DMA region: the YT8531 PHY, the controller reset, the coherence probe, both
//! rings, the link and the start. It reports what it saw to its spawner and exits; the spawner
//! then starts `net_stack` over the data-plane pages. The clocks and resets are already up, through
//! the clock and reset server. See notes/designware-ethernet.md.
//!
//! # Capability contract
//!
//! Held, the complete list: the report endpoint at [`CONTROL_REPORT_SLOT`], `WRITE` only; and three
//! windows its spawner maps and lists on the startup page and no capability names, so it cannot
//! delegate them: the controller's first two register pages and the always-on syscon page, both
//! device-typed, and the DMA region. Denied: the controller's pages past `0x2000`, an `Interrupt`,
//! and everything else.
//!
//! # BUGS
//!
//! - The JH7110 has no IOMMU, so the rings it programs can aim the device's DMA anywhere: this
//!   process runs trusted, as §86 (whether an NVMe driver can leave the kernel) says for NVMe.
//! - The syscon page holds other blocks' words beside `gmac0`'s, and a fault here could write them.
//! - Waiting for a link is a bounded spin, up to `bring_up::LINK_WAIT_MILLISECONDS`.
//!
//! Name: provisional.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates: one lint
// policy)'s ratchet tracks (§107 (`missing_docs` moves to `workspace.lints.rust`, opt-out rather
// than opt-in)).
#![allow(missing_docs)]
#![no_main]

use designware_ethernet::bring_up::{self, Account, Port};
use designware_ethernet::{DirectMemoryAccessRegion, Hardware, Tails, jh7110, layout, registers};
use driver_process_contract::designware_ethernet::{
    ACCOUNT_RECORDS, ALWAYS_ON_SYSTEM_CONTROLLER_KIND, CONTROL_REGISTER_KIND,
    CONTROL_REGISTER_PAGES, CONTROL_REPORT_SLOT, ControlHandoff, ControlRecord, ControlReport,
    DATA_PLANE_KIND,
};
use startup_page_contract::StartupWindow;
use user_mode_runtime::mapped_window::{MappedWindow, PAGE, doorbell_barrier};

// SAFETY: the spawner (`kernel/src/user/designware_ethernet_service.rs`) lists all three windows
// on the startup page and maps them before `_start` runs, for the life of the process: the
// registers and the syscon page device-typed, the region read/write normal memory.
static REGISTERS: StartupWindow =
    unsafe { StartupWindow::new(CONTROL_REGISTER_KIND, CONTROL_REGISTER_PAGES * PAGE) };
// SAFETY: as above.
static SYSTEM_CONTROLLER: StartupWindow =
    unsafe { StartupWindow::new(ALWAYS_ON_SYSTEM_CONTROLLER_KIND, PAGE) };
// SAFETY: as above.
static REGION: StartupWindow = unsafe {
    StartupWindow::new(
        DATA_PLANE_KIND,
        layout::DIRECT_MEMORY_ACCESS_REGION_PAGES * PAGE,
    )
};

/// A window from the startup page, as a bounds-checked [`MappedWindow`] of its static's length.
fn mapped(window: &StartupWindow) -> MappedWindow {
    // SAFETY: each `StartupWindow` here was declared on the spawner's promise that it maps the
    // window for the life of the process, which is what `MappedWindow::new` asks.
    unsafe { MappedWindow::new(window.virtual_address(), window.length_bytes()) }
}

/// The controller, through its first two pages: every offset `MappedWindow` bounds.
struct Controller;

impl Hardware for Controller {
    fn read(&mut self, off: u32) -> u32 {
        mapped(&REGISTERS).r32(u64::from(off))
    }
    fn write(&mut self, off: u32, v: u32) {
        mapped(&REGISTERS).w32(u64::from(off), v);
    }
    fn delay_microseconds(&mut self, microseconds: u64) {
        let until = now_microseconds() + microseconds;
        while now_microseconds() < until {
            core::hint::spin_loop();
        }
    }
}

/// The DMA region, through its window: ordinary cached memory, which is the point of the probe.
struct Plane;

impl DirectMemoryAccessRegion for Plane {
    fn read_u64(&self, off: u64) -> u64 {
        mapped(&REGION).r64(off)
    }
    fn write_u64(&mut self, off: u64, v: u64) {
        mapped(&REGION).w64(off, v);
    }
    fn read_bytes(&self, off: u64, out: &mut [u8]) {
        mapped(&REGION).copy_to_slice(off, out);
    }
    fn write_bytes(&mut self, off: u64, src: &[u8]) {
        mapped(&REGION).copy_from_slice(off, src);
    }
}

/// The two tail pointers, on the DMA page of [`REGISTERS`].
struct Channel;

impl Tails for Channel {
    fn set_receive_tail(&mut self, tail_pointer_low_bits: u32) {
        Controller.write(
            registers::CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER,
            tail_pointer_low_bits,
        );
    }
    fn set_transmit_tail(&mut self, tail_pointer_low_bits: u32) {
        Controller.write(
            registers::CHANNEL_TRANSMIT_DESCRIPTOR_TAIL_POINTER_REGISTER,
            tail_pointer_low_bits,
        );
    }
    fn barrier(&mut self) {
        doorbell_barrier();
    }
}

impl Port for Controller {
    type Region<'a> = Plane;
    type Tails<'a> = Channel;
    fn with_region<R>(&mut self, f: impl FnOnce(&mut Plane, &mut Channel) -> R) -> R {
        f(&mut Plane, &mut Channel)
    }
    fn now_microseconds(&mut self) -> u64 {
        now_microseconds()
    }
}

fn now_microseconds() -> u64 {
    user_mode_runtime::monotonic_nanoseconds() / 1000
}

/// Select RGMII for `gmac0`: a read-modify-write of its three bits, the word's other bits kept.
fn select_interface() -> (u32, u32) {
    let syscon = mapped(&SYSTEM_CONTROLLER);
    let offset = jh7110::GMAC0_SYSTEM_CONTROLLER_OFFSET;
    let before = syscon.r32(offset);
    syscon.w32(
        offset,
        jh7110::select_reduced_gigabit_media_independent_interface(before),
    );
    (before, syscon.r32(offset))
}

/// Send `report` in its parts, then `outcome`, to the spawner.
fn report(report: &ControlReport, outcome: Result<bool, bring_up::Error>) {
    let words = report.to_words();
    for index in 0..ACCOUNT_RECORDS {
        let record = ControlRecord::Part {
            index: index as u8,
            words: [words[2 * index], words[2 * index + 1]],
        };
        let [w0, w1, w2] = record.pack();
        user_mode_runtime::send(CONTROL_REPORT_SLOT, w0, w1, w2);
    }
    let [w0, w1, w2] = ControlRecord::Done(outcome).pack();
    user_mode_runtime::send(CONTROL_REPORT_SLOT, w0, w1, w2);
}

user_mode_runtime::program_entry!(start);

fn start(arg0: u64, arg1: u64, arg2: u64) -> ! {
    // A spawn this process cannot read is no spawner's: say nothing and touch nothing.
    let Some(handoff) = ControlHandoff::unpack(arg0, arg1, arg2) else {
        user_mode_runtime::exit()
    };
    let syscon = select_interface();
    let mut account = Account::default();
    let outcome = bring_up::bring_up(&mut Controller, &handoff.inputs, &mut account)
        .map(|(_, link)| link.is_some());
    report(&ControlReport { account, syscon }, outcome);
    user_mode_runtime::exit()
}

user_mode_runtime::panic_handler!();
