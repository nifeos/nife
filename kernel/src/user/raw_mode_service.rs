use super::*;
use crate::cap::{Rights, rendezvous_cap};
use crate::sched::{self, RendezvousId};
use crate::user::holding::Holding;

/// The VAs `components/src/line_editor.rs` hardcodes for its console and application pages. Must match
/// that file's `CONOUT_VA`, `APP_OUT_VA`, `APP_IN_VA`.
const CONOUT_VA: u64 = address_space_map::pair_page(0x0060_0000);
const APP_OUT_VA: u64 = address_space_map::pair_page(0x0080_0000);
const APP_IN_VA: u64 = address_space_map::pair_page(0x0090_0000);

/// A running `line_editor` and the raw physical pages a test needs to drive it directly, playing
/// both the input driver and the application on the one terminal endpoint, which the contract
/// allows: nobody on the other end of `TERM` can tell who is calling (§21 (the terminal is a
/// userspace component)'s endpoint-only naming).
pub struct Wiring {
    /// The terminal endpoint, for `OPERATION_RAWMODE`, `OPERATION_READRAW`, `OPERATION_BYTES` (as
    /// the input driver would), `OPERATION_READLINE` and `OPERATION_WRITE`.
    pub term: RendezvousId,
    /// The console's shared page, physical, which `line_editor` echoes into. A test
    /// sentinel-fills it before an exchange and checks it after, so "no echo happened" is proven.
    pub console_phys: u64,
    /// The client output page: a test writes an `OPERATION_READLINE` prompt or an `OPERATION_WRITE` payload
    /// here before the request that names it.
    pub app_out_phys: u64,
    /// The client input page: `line_editor` writes a completed line here for `OPERATION_READLINE`.
    pub app_in_phys: u64,
    /// `line_editor`'s control endpoint, when [`start_replaceable`] built it with one: the test
    /// plays the supervisor and `SEND`s `CTL_RESUME` or `CTL_QUIT` here after an `OPERATION_QUIESCE`.
    /// `None` from [`start`], which wires `line_editor` exactly as every boot does today.
    pub control: Option<RendezvousId>,
}

/// Spawn a fake console and a real `line_editor`, wired as the boot path wires them except that
/// the caller plays the input driver and the application.
///
/// The fake console answers `Con::flush`'s `SEND` with an ack and never inspects the shared page,
/// so a test checks echo itself through [`Wiring::console_phys`]. Returns a [`Holding`] the caller
/// must release: both threads block forever, and only reclaiming the region their endpoints come
/// from wakes them to die. An unreleased call holds two threads of the suite's fixed table.
pub fn start() -> (Wiring, Holding) {
    start_with(false)
}

/// [`start`], plus a control endpoint in slot 3 whose number is `line_editor`'s second start
/// argument: the wiring a supervisor that can replace it would use. `line_editor` then honors
/// `OPERATION_QUIESCE` rather than refusing it.
pub fn start_replaceable() -> (Wiring, Holding) {
    start_with(true)
}

fn start_with(replaceable: bool) -> (Wiring, Holding) {
    let image = program("line_editor").expect("no line_editor program in the initial ramdisk");

    // One page per endpoint, so reclaiming the region wakes both threads to die.
    let ep_region =
        crate::memory_region::create(4).expect("no endpoint region for raw_mode_service");
    let term = sched::create_rendezvous_from(ep_region).expect("no TERM endpoint");
    let conreq = sched::create_rendezvous_from(ep_region).expect("no CONREQ endpoint");
    let conrep = sched::create_rendezvous_from(ep_region).expect("no CONREP endpoint");
    // The spare page. Minted either way so both wirings spend the same region; granted only when
    // the caller asked for a replaceable terminal.
    let control = sched::create_rendezvous_from(ep_region).expect("no control endpoint");

    let console_phys = crate::memory::alloc_zeroed()
        .expect("no frame for the fake console's shared page")
        .addr();
    let app_out_phys = crate::memory::alloc_zeroed()
        .expect("no frame for line_editor's app-output page")
        .addr();
    let app_in_phys = crate::memory::alloc_zeroed()
        .expect("no frame for line_editor's app-input page")
        .addr();

    let console_tid = sched::spawn(move || {
        loop {
            sched::ipc_receive(conreq);
            sched::ipc_send(conrep, [0, 0, 0]);
        }
    })
    .expect("could not spawn the fake console");

    let line_editor_tid = sched::spawn(move || {
        let all = [
            rendezvous_cap(term, Rights::READ),    // slot 0: TERM, RECEIVE_CAP
            rendezvous_cap(conreq, Rights::WRITE), // slot 1: CONREQ, SEND
            rendezvous_cap(conrep, Rights::READ),  // slot 2: CONREP, RECEIVE
            rendezvous_cap(control, Rights::READ), // slot 3: control, RECEIVE (replaceable only)
        ];
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: if replaceable { 3 } else { 0 },
                arg2: 0,
                grants: if replaceable { &all } else { &all[..3] },
                maps: &[
                    Mapping {
                        va: CONOUT_VA,
                        phys: console_phys,
                        flags: Flags::user_data(),
                    },
                    Mapping {
                        va: APP_OUT_VA,
                        phys: app_out_phys,
                        flags: Flags::user_rodata(),
                    },
                    Mapping {
                        va: APP_IN_VA,
                        phys: app_in_phys,
                        flags: Flags::user_data(),
                    },
                ],
            },
        )
    })
    .expect("could not spawn line_editor");

    let mut held = Holding::new();
    held.add_thread(console_tid);
    held.add_thread(line_editor_tid);
    held.add_region(ep_region);

    (
        Wiring {
            term,
            console_phys,
            app_out_phys,
            app_in_phys,
            control: replaceable.then_some(control),
        },
        held,
    )
}

// BUGS: `console_phys`, `app_out_phys` and `app_in_phys` come from `crate::memory::alloc`
// directly, not from a region this `Holding` knows about, so `release` does not reclaim them: a
// released call still costs three page frames for the boot. The eight call sites cost the aarch64
// suite 108 frames, which `SUITE_PAGE_FRAME_BUDGET` counts. Worth fixing only if callers grow.
