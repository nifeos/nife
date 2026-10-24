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
/// both the input driver and the application on the one terminal endpoint. That double role is
/// exactly what the real terminal contract already allows: nobody on the other end of `TERM` can
/// tell who is calling (DECISIONS §21's endpoint-only naming), so a test standing in for both is
/// not a shortcut around the contract, it is the contract.
pub struct Wiring {
    /// The terminal endpoint. `CALL` here for every opcode this milestone added or reused:
    /// `OPERATION_RAWMODE`, `OPERATION_READRAW`, `OPERATION_BYTES` (as the input driver would), `OPERATION_READLINE`,
    /// `OPERATION_WRITE`.
    pub term: RendezvousId,
    /// The console's shared page, physical. This is what `line_editor` echoes into
    /// (`components/src/line_editor.rs`'s `Con`). A test sentinel-fills it before an exchange and checks
    /// it after: that is how "no echo happened" gets proven rather than assumed, the same
    /// byte-for-byte witness-page discipline `c_seam`'s confiner tests use for a different claim.
    pub console_phys: u64,
    /// The client output page: a test writes an `OPERATION_READLINE` prompt or an `OPERATION_WRITE` payload
    /// here before the request that names it.
    pub app_out_phys: u64,
    /// The client input page: `line_editor` writes a completed line here for `OPERATION_READLINE`.
    pub app_in_phys: u64,
    /// `line_editor`'s control endpoint, when [`start_replaceable`] built it with one: the test
    /// plays the supervisor and `SEND`s `CTL_RESUME` or `CTL_QUIT` here after an `OPERATION_QUIESCE`.
    /// `None` from [`start`], which wires `line_editor` with no supervisor.
    pub control: Option<RendezvousId>,
}

/// **Spawn a fake console and a real `line_editor`**, wired exactly as the boot path wires them
/// except that the input driver and the application are both played by the caller.
///
/// The fake console speaks only the one protocol `line_editor`'s `Con::flush` needs: `SEND` a
/// count, `RECEIVE` an ack (not a `CALL`; `components/src/line_editor.rs`'s own module doc explains why
/// that hop is safe with exactly one client). It never inspects the shared page; a test checks
/// that directly through [`Wiring::console_phys`], which is the point: a fake that graded its own
/// homework would prove nothing about echo suppression.
///
/// **Returns a [`Holding`] alongside the wiring, and a caller must release it.** Both the fake
/// console and `line_editor` block forever (the console in `RECEIVE`, `line_editor` waiting on its
/// next request): neither ever exits on its own, exactly `virtio_service`'s own `wire_net_server`
/// shape (see its doc comment on `ep_region`). So `TERM`/`CONREQ`/`CONREP` are minted from a
/// region the `Holding` also carries, not from `sched::create_rendezvous`'s kernel-global pool:
/// reclaiming that region is what actually wakes a `Blocked` thread (`kill_thread`'s armed flag is
/// only spent in `schedule()`, which a thread parked in `RECEIVE` never reaches). Found the hard way
/// on 2026-08-27: six tests calling this and none releasing it left twelve threads permanently
/// `Blocked` at 128/128 in the whole-suite thread table, so the thirteenth spawn anywhere in the
/// suite (an unrelated FS client, `fs_service.rs`) failed with `could not spawn`. See
/// `notes/frames.md` and this module's own `BUGS`.
pub fn start() -> (Wiring, Holding) {
    start_with(false)
}

/// [`start`], plus a control endpoint in slot 3 whose number is `line_editor`'s second start
/// argument: the wiring a supervisor that can replace it would use (milestone 23 (a
/// capability-routed component OS with live replacement)). The only difference `line_editor` can
/// see is that `OPERATION_QUIESCE` is honoured rather than refused.
pub fn start_replaceable() -> (Wiring, Holding) {
    start_with(true)
}

fn start_with(replaceable: bool) -> (Wiring, Holding) {
    let image = program("line_editor").expect("no line_editor program in the initial ramdisk");

    // Four pages for three endpoints, one page each and one spare: `virtio_service`'s own
    // `wire_net_server` carves the same shape for the same reason.
    let ep_region =
        crate::memory_region::create(4).expect("no endpoint region for raw_mode_service");
    let term = sched::create_rendezvous_from(ep_region).expect("no TERM endpoint");
    let conreq = sched::create_rendezvous_from(ep_region).expect("no CONREQ endpoint");
    let conrep = sched::create_rendezvous_from(ep_region).expect("no CONREP endpoint");
    // The spare page. Minted either way so both wirings spend the same region; granted only when
    // the caller asked for a replaceable terminal.
    let control = sched::create_rendezvous_from(ep_region).expect("no control endpoint");

    // Zeroed, so no stale RAM leaks into the test.
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
// directly, not from a region this `Holding` knows about, so `release` does not reclaim them; a
// released call still costs three page frames for the life of the boot. Same shape and same
// judgment as `virtio_service`'s DMA page ("twenty page frames is not worth the hazard"): three
// pages times six raw_mode_tests plus two rmle_tests call sites is eight tests' worth (`[that test
// kept N frames]` on this suite shows 31 or 33, not the ~112 that leaving the *threads*
// unreclaimed would have cost per call). Worth fixing only if a future caller adds enough call
// sites to matter.
//
// Corrected 2026-08-28: an earlier version of this note said the residue was "already inside
// `SUITE_PAGE_FRAME_BUDGET`'s headroom", and it was not. Measured on the merged tree, these eight
// tests cost the suite +108 frames on aarch64 over the trunk they landed on, past a budget with 32
// frames of margin, and that constant was raised to 22249 to account for them. The residue is
// still a deliberate, recorded cost rather than a leak; what was wrong was the claim that somebody
// else's margin would absorb it.
