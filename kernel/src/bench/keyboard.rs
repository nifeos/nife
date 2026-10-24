//! **The virtio keyboard driver's cost**: `keyboard_up` and `keyboard_event`, reported only by
//! `script/bench --keyboard`, which attaches the keyboard and presses its keys. Never gated: an
//! ordinary bench boot has no keyboard and prints neither row. See
//! notes/benchmarks/kernel-bench-harness.md, "The keyboard rows".
//!
//! Name: provisional (milestone 947, 2026-10-24 UTC), for the module, the file and both rows.

use super::*;
use crate::user::keyboard_service;

/// Key presses `keyboard_event` sums over. Each is one interrupt carrying an `EV_KEY` and an
/// `EV_SYN`; the host waits for the cue line before pressing the next, so no two share a drain.
const KEY_PRESSES: u64 = 16;

/// The line the host's actor answers with one `sendkey` (`xtask/src/bench.rs`). Printed before
/// each press; the host never presses unprompted.
const PRESS_CUE: &str = "bench-keyboard: press";

/// Time the keyboard driver's bring-up and its per-key path, or return at once when the boot has
/// no keyboard or no `keyboard_driver` program.
///
/// `keyboard_up` runs from the driver's spawn being queued to its `KEYBOARD_UP` report: loading
/// the image, feature negotiation, queue setup and the first buffers, with the host not involved.
/// `keyboard_event` runs, per press, from the kernel's delivery of the keyboard's interrupt
/// (`sched::watched_irq_at`) to the driver's `COMMIT` reaching this thread, which stands in for
/// the compositor; it excludes the wait for the host, which under icount is idle time the counter
/// skips past. No `TickWindow`: the windows open on a device's interrupt, not on a tick, and the
/// 1 Hz bench tick landing in one would move one press by a tick handler's cost, which
/// `--keyboard` runs report rather than gate.
pub(super) fn keyboard() {
    let Some(image) = crate::trust::require_program("keyboard_driver") else {
        return;
    };
    let Some(mut wiring) = keyboard_service::start(image) else {
        return; // no virtio keyboard on this boot
    };
    // After the wiring (finding the device, its DMA page, the IOMMU domain), which is the
    // kernel's and not the driver's. The spawned thread has not run yet: it loads the image once
    // this thread blocks below.
    let t0 = crate::arch::timer::now();
    let [tag, ..] = sched::ipc_receive(wiring.report);
    let t1 = crate::arch::timer::now();
    if tag != video_terminal::status::KEYBOARD_UP {
        println!("bench-probe: keyboard_driver reported {tag:#x}, not KEYBOARD_UP; no rows");
        return;
    }
    println!("bench: keyboard_up {} 1", t1 - t0);

    sched::watch_irq(wiring.irq);
    let mut total = 0;
    let mut typed = [0u8; 16];
    for i in 0..KEY_PRESSES {
        println!("{PRESS_CUE} {i}");
        let arrived = wiring.answer_doorbell();
        total += arrived - sched::watched_irq_at();
        if wiring.take_typed(&mut typed) == 0 {
            println!("bench-probe: press {i} rang the doorbell with nothing typed; no row");
            return;
        }
    }
    println!("bench: keyboard_event {total} {KEY_PRESSES}");
}
