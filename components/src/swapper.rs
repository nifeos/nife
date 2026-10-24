//! **The operator: replace a running component under a talking client** (milestone 23,
//! DECISIONS §41).
//!
//! An unprivileged process: the kernel has no notion of a component or a swap, and what it does
//! supply is listed in notes/live-replacement/the-operator.md with each role's design. The entry
//! role picks the system (`hot_swap_contract`'s `ROLE_*`). The swap's five steps and their order
//! are in notes/live-replacement.md.
//!
//! Name: ratified 2026-07-30 (calef, DECISIONS §39, landed by milestone 46), replacing `swapd`.
//! Refused `swapd` (the `-d` claim).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// Two shared modules: the swap system's protocol and the supervision tree's loader. Each binary
// uses a different slice of both, so the unused halves are expected (§38).
use capability_plan::Provisions;
use hot_swap_contract::log_checks as lc;
use supervision_protocol::{ChildEndowment, Retention};
use user_mode_runtime::{
    address_space_map_into, cap_delete, cntfrq, map_page_frame, now, receive, receive_bound,
    receive_fault, revoke_page_frame, send, timer_arm,
};

/// What the kernel grants us, and nothing else.
const ROOT_UT: u64 = 0; // the construction budget: what every process here is built out of
const REPORT: u64 = 1; // WRITE|GRANT, so each child gets its own narrowed view
const DEVICE: u64 = 2; // the UART's registers, WRITE|GRANT: ours to lend, and ours to take back
/// The notification bound to this thread by the spawner, and the timer that arms it (milestone 800
/// (a non-Anthropic model attacks the confinement claim), fourth pass): the pair that makes the
/// plant's un-park bounded, exactly `net_stack`'s retransmit-wait pattern. The bound is the
/// spawner's to make, because a running thread holds no capability to its own TCB.
const WAKE: u64 = 3;
const TIMER: u64 = 4;

// Pages per child come from each contract (`hot_swap_contract::INSTANCE_PAGES`, through
// `capability_plan::Plan::pages`) and are a peak: only the hung channel destroys a region, so
// otherwise all five splits are live at once and the budget covers them together.

/// The channels this operator owns, created once out of its own budget.
struct Wiring {
    /// The stable name. One endpoint object; every client capability to it is handed out before
    /// either component exists, and nothing ever creates a second one.
    svc: u64,
    /// Our children's deaths, kernel-stamped (DECISIONS §26).
    faultep: u64,
    /// Our own coordination channel. Separate from the report endpoint on purpose: the report
    /// endpoint belongs to the test, and an operator that read it would be stealing the record.
    note: u64,
    /// How we speak to a component that has quiesced, and so is no longer listening on `svc`.
    poke: u64,
    /// The witness page, mapped read/write into us and into every component.
    log_page_frame: u64,
}

user_mode_runtime::program_entry!(start);

fn start(role: u64, initial_ramdisk_len: u64, _a2: u64) -> ! {
    // SAFETY: forwarded from user_mode_runtime::initial_ramdisk::bytes's own contract.
    let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };
    let Ok(fs) = nifefs::Fs::parse(archive) else {
        bail(1)
    };

    let w = Wiring {
        svc: obj(abi::objtype::RENDEZVOUS, 5),
        faultep: obj(abi::objtype::RENDEZVOUS, 6),
        note: obj(abi::objtype::RENDEZVOUS, 7),
        poke: obj(abi::objtype::RENDEZVOUS, 8),
        log_page_frame: page_frame(9),
    };
    if !map_page_frame(w.log_page_frame, hot_swap_contract::LOG_VA, true, ROOT_UT) {
        bail(10)
    }
    for i in 0..hot_swap_contract::PAGE {
        let slot = hot_swap_contract::LOG_VA as *mut u8;
        // SAFETY: a page we just mapped read/write into our own address space.
        unsafe { core::ptr::write_volatile(slot.add(i as usize), 0) };
    }

    match role {
        hot_swap_contract::ROLE_QUEUED => queued(&fs, &w, Warn::OnTime),
        hot_swap_contract::ROLE_UNWARNED => queued(&fs, &w, Warn::Never),
        hot_swap_contract::ROLE_LATE_WARNING => queued(&fs, &w, Warn::Late),
        hot_swap_contract::ROLE_HUNG => hung(&fs, &w, true),
        hot_swap_contract::ROLE_HUNG_WITHOUT_DEVICE => hung(&fs, &w, false),
        hot_swap_contract::ROLE_HANDOFF => handoff(&fs, &w),
        hot_swap_contract::ROLE_DEPUTY => deputy(&fs, &w),
        _ => direct(&fs, &w),
    }
}

// ===============================================================================================
// The confused-deputy channel (milestone 633 (an outside agent attacks the confinement claim), third pass): one incumbent holding the device and the
// log, and a client that drives it to write past the log into the device page. No swap; the point is
// what a client can make a server do on its behalf, not continuity across a replacement.
// ===============================================================================================

fn deputy(fs: &nifefs::Fs, w: &Wiring) -> ! {
    let v1 = image(fs, "rust_swappable", 2);
    let client_img = image(fs, "swap_boundary_witness", 4);

    // The server's routing includes the device (`uart`) and the witness page; the client's does not,
    // exactly as on the direct channel. This is what makes "the client was never granted the device"
    // a fact about the wiring rather than a comment.
    let to_component = Provisions {
        held: &[
            ("service", w.svc),
            ("report", REPORT),
            ("operator", w.note),
            ("control", w.poke),
            ("witness", w.log_page_frame),
            ("uart", DEVICE),
        ],
    };
    let to_client = Provisions {
        held: &[("service", w.svc), ("report", REPORT), ("operator", w.note)],
    };
    let Ok(component) = capability_plan::plan(&hot_swap_contract::CONSOLE, &to_component) else {
        bail(61)
    };
    let Ok(client) = capability_plan::plan(&hot_swap_contract::CLIENT, &to_client) else {
        bail(62)
    };

    // The incumbent: holds the device and the log, receiving on the stable endpoint. `log_base = 0`,
    // so the attacker's offset would reach `DEV_VA` through `log_put(log_base + arg, ..)` if
    // `log_put` did not bound it.
    start_child(&v1, &component, w.faultep, [1, 0, 0], 11);

    // The attacker: the honest client's capabilities and no device. It drives the server to write
    // past the log, then reports what the server answered.
    start_child(
        &client_img,
        &client,
        w.faultep,
        [hot_swap_contract::ROLE_CONFUSED, 0, 0],
        30,
    );
    expect_note(w.note, hot_swap_contract::NOTE_ATTACK_DONE, 33);

    let mut corpses = 0u64;
    reap_to(w.faultep, &mut corpses, 1); // the attacker
    retire(w, &mut corpses, 2, 34); // quiesce the incumbent and collect it

    send(
        REPORT,
        hot_swap_contract::RPT_LOG,
        verdict_from_log(0, true),
        0,
    );
    user_mode_runtime::exit()
}

// ===============================================================================================
// The default rung: the endpoint is the stable name and nothing stands in the data path.
// ===============================================================================================

fn direct(fs: &nifefs::Fs, w: &Wiring) -> ! {
    let v1 = image(fs, "rust_swappable", 2);
    let v2 = image(fs, "c_swappable", 3);
    let client_img = image(fs, "swap_boundary_witness", 4);

    // What this operator routes, per child, by the name the component uses for it; rights,
    // addresses and slot order come from each declaration. Two tables, because the manifest bounds
    // what a component may ask for and a routing table what this operator offers: a client that
    // could be routed the device is one manifest edit away from holding it.
    let to_component = Provisions {
        held: &[
            ("service", w.svc),
            ("report", REPORT),
            ("operator", w.note),
            ("control", w.poke),
            ("witness", w.log_page_frame),
            ("uart", DEVICE),
        ],
    };
    let to_client = Provisions {
        held: &[("service", w.svc), ("report", REPORT), ("operator", w.note)],
    };

    // The control that must fail, before anything exists: the broker's declaration names
    // `requests` and `backend`, which this channel routes neither of, so it is refused as a value
    // with nothing built. A run in which it succeeds fails loudly.

    match capability_plan::plan(&hot_swap_contract::BROKER, &to_component) {
        Ok(_) => bail(60),
        Err(refusal) => send(REPORT, hot_swap_contract::RPT_REFUSED, refusal.code(), 0),
    };

    let Ok(component) = capability_plan::plan(&hot_swap_contract::CONSOLE, &to_component) else {
        bail(61)
    };
    let Ok(client) = capability_plan::plan(&hot_swap_contract::CLIENT, &to_client) else {
        bail(62)
    };

    // The incumbent.
    start_child(&v1, &component, w.faultep, [1, 0, 0], 11); // a device, log entries from 0

    // Step 1: build the replacement before anyone is talking, endowed with everything except the
    // device and not configured, so it cannot race the incumbent for requests. A build is a few
    // hundred syscalls; done after the trigger it lost the race on RISC-V, and done here it keeps
    // the down window four syscalls wide.

    let replacement = build_without_devices(&v2, &component, w.faultep, 20);

    // The client that talks across the swap. The incumbent notes us after SWAP_TRIGGER requests,
    // so the swap lands mid-conversation rather than at a moment picked by counting yields.

    start_child(
        &client_img,
        &client,
        w.faultep,
        [hot_swap_contract::ROLE_CLIENT, 0, 0],
        14,
    );
    expect_note(w.note, hot_swap_contract::NOTE_SWAP_NOW, 17);

    // Who depends on `console`, asked before step 2 acts: an empty set here, since nothing this
    // operator runs declares it in `depends_on` (`CLIENT` is a pure consumer, by the sender-queue
    // argument of §41 (the endpoint is the broker, and a device is revoked by taking it back)).
    // Reported so the test can check the graph agrees.

    let live = [capability_plan::LiveInstance {
        id: 1,
        reqs: &hot_swap_contract::CONSOLE,
    }];
    report_dependents("console", &live, 64);

    // Step 2: drain. The quiesce travels on the endpoint being drained, so FIFO does the waiting.

    drain(w, 22);

    // Step 3: take the registers back. Every other holder loses the capability and the mapping; we
    // keep ours, which is what makes this a transfer rather than a demolition (DECISIONS §41).

    revoke_device(23);

    // Step 4: endow the replacement with the registers and start it. It drains whatever parked on
    // the service endpoint's sender queue while nobody was receiving.

    endow_devices_and_start(replacement, &v2, &component, 24);

    // Step 5: the receipt for the revoke. The incumbent is quiesced and alive, holding an address
    // that used to be a UART; told to read one register, it faults if step 3 was real.

    send(w.poke, hot_swap_contract::POKE_PROBE, 0, 0);
    let mut corpses = 0u64;
    let revoke_enforced = wait_for_fault(w.faultep, &mut corpses, hot_swap_contract::DEV_VA);

    // The conversation is still running: the client is somewhere in its sixties, being served by the
    // replacement. Wait for it to finish before reading the witness page, or the requests it has not
    // made yet would read as requests nobody served.
    expect_note(w.note, hot_swap_contract::NOTE_CLIENT_DONE, 28);
    reap_to(w.faultep, &mut corpses, 2); // the incumbent and the client

    attack_the_service(w, &client_img, &client, &mut corpses);

    // Our own witness: the log page, read in our address space, after every writer is gone.
    // Independent of the client's verdict, which was computed from replies in a different address
    // space and reaches the test on its own.
    send(
        REPORT,
        hot_swap_contract::RPT_LOG,
        verdict_from_log(0, revoke_enforced),
        changed_at(0),
    );
    user_mode_runtime::exit()
}

/// **The attacker, after the swap**: retire the replacement, start the plant and the usurper on
/// the client's own declaration, and un-park the plant within a bound; `direct`'s last act.
fn attack_the_service(
    w: &Wiring,
    client_img: &elf::Elf,
    client: &capability_plan::Plan,
    corpses: &mut u64,
) {
    // The replacement is retired first, and the order is the point: once it has answered QUIESCE
    // it never receives again, so the plant's message is the only thing in the queue and a
    // let-open `RECEIVE_CAP` returns it. With a live server receiving, the queue is always empty
    // and a break would surface as a watchdog hang. The test asserts on its region coming home.
    retire(w, corpses, 3, 34);

    // The plant, holding nothing the honest client does not: it parks one message on the endpoint
    // and exits when anyone takes it. No handshake: if the attacker already parked as a receiver,
    // the send delivers to it directly, so whoever the kernel's gate allowed takes the message.
    start_child(
        client_img,
        client,
        w.faultep,
        [hot_swap_contract::ROLE_PLANT, 0, 0],
        30,
    );

    // The attacker, wired from the client's own declaration and the client's own routing table, so
    // "exactly the honest client's capabilities" is a property of the code rather than of two
    // arrays a reader has to compare.
    start_child(
        client_img,
        client,
        w.faultep,
        [hot_swap_contract::ROLE_USURPER, 0, 0],
        31,
    );
    // The attacker's verdict, taken from the note rather than the report stream: `w1` is the
    // refusal code (positive) or `-PLANT_MARKER` on a let-through, mirroring `RPT_ATTACK`.
    let refused = {
        let w1 = expect_note(w.note, hot_swap_contract::NOTE_ATTACK_DONE, 33);
        (w1 as i64) > 0
    };

    // Un-park the plant within a bound. A refused attacker leaves it parked and this returns its
    // marker; a let-through one leaves the queue empty, and the bound notification ends the wait
    // so the run fails at the test's assertion rather than a watchdog. The bound can also fire
    // before the plant has sent (spawn order is not run order), and then the plant never exits and
    // the reap would hang, so with a refusal reported that case bails on its own stage.
    if timer_arm(TIMER, now() + cntfrq() / 2, WAKE, 1) < 0 {
        bail(35);
    }
    match receive_bound(w.svc) {
        user_mode_runtime::Received::Message(word, _, _)
            if word == hot_swap_contract::PLANT_MARKER => {}
        user_mode_runtime::Received::Notification(_) if refused => bail(37),
        user_mode_runtime::Received::Notification(_) => {}
        _ => bail(36),
    }
    reap_to(w.faultep, corpses, 5); // the attacker and the plant
}

/// Build the replacement with everything except the device, not configured, and report it built:
/// step 1 of [`direct`] and [`hung`]. Bails `stage` or `stage + 1`.
fn build_without_devices(
    elf: &elf::Elf,
    component: &capability_plan::Plan,
    faultep: u64,
    stage: u64,
) -> Unstarted {
    // The one endowment this operator still writes by hand, and it is the deferral rather than the
    // authority: `maps_without_devices` is the declaration minus what a revoke is about to take, and
    // `devices` below is the rest of it. `capability_plan` sorts device mappings last so both
    // halves are slices of one plan and neither can be built by hand.
    let Ok(region) = supervision_protocol::memory_region_split(ROOT_UT, component.pages()) else {
        bail(stage)
    };
    let Ok((child, aspace)) = supervision_protocol::build_child_space(
        ROOT_UT,
        region,
        elf,
        &ChildEndowment {
            caps: component.caps(),
            maps: component.maps_without_devices(),
            blobs: &[],
            fault: Some(faultep),
            ..ChildEndowment::new(Retention::Nothing)
        },
    ) else {
        bail(stage + 1)
    };
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::BUILT,
        hot_swap_contract::V2,
    );
    Unstarted {
        child,
        aspace,
        region,
    }
}

/// Take the device registers back from every other holder, and report it. Bails `stage`.
fn revoke_device(stage: u64) {
    if revoke_page_frame(DEVICE) != 0 {
        bail(stage)
    }
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::REVOKED,
        0,
    );
}

/// Map the device into a replacement built by [`build_without_devices`], start it, and report it
/// started. Bails `stage` through `stage + 2`.
fn endow_devices_and_start(
    u: Unstarted,
    elf: &elf::Elf,
    component: &capability_plan::Plan,
    stage: u64,
) {
    for &(va, slot, mode) in component.devices() {
        if address_space_map_into(u.aspace, va, slot, mode) != 0 {
            bail(stage)
        }
    }
    // The device flag is the plan's: a component that declares no registers is not told to probe any.
    let device = !component.devices().is_empty();
    launch(u, elf, [device as u64, 0, 0], stage + 1);
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::STARTED,
        hot_swap_contract::V2,
    );
}

/// Ask the dependency graph who depends on `target` among `live`, and report how many and the
/// first. Bails `stage` on a refusal.
fn report_dependents(
    target: &str,
    live: &[capability_plan::LiveInstance],
    stage: u64,
) -> capability_plan::Dependents {
    let Ok(deps) = capability_plan::dependents(target, live) else {
        bail(stage)
    };
    let order = deps.quiesce_order();
    send(
        REPORT,
        hot_swap_contract::RPT_DEPENDENTS,
        order.len() as u64,
        order.first().copied().unwrap_or(0),
    );
    deps
}

// ===============================================================================================
// State handoff (DECISIONS §209): the component carries state across the swap, and a swap whose
// state was not absorbed does not commit.
// ===============================================================================================

/// **The swap, for a component that declared a handoff page** (§209 (state handoff is an opaque
/// blob over a granted frame, and it is optional)): no device, one handoff frame routed to every
/// instance and never read here, replacements on a `control` endpoint distinct from the
/// incumbent's, and a commit only on a replacement's `NOTE_ABSORBED`. A replacement that refuses
/// exits and is collected, which returns its region, and the incumbent resumes with the state it
/// never lost. See notes/live-replacement/the-operator.md.
fn handoff(fs: &nifefs::Fs, w: &Wiring) -> ! {
    let v1 = image(fs, "rust_swappable", 2);
    let v2 = image(fs, "c_swappable", 3);
    let client_img = image(fs, "swap_boundary_witness", 4);

    // The handoff run, as many pages as the contract declares, minted as one frame capability
    // (`MemoryRegion::RETYPE`'s page count). A contract with no handoff would not reach this role.
    let Some(declared) = hot_swap_contract::STATEFUL_BACKEND.state_handoff else {
        bail(90)
    };
    let state = match user_mode_runtime::retype_page_frame_run(ROOT_UT, declared.pages) {
        s if s >= 0 => s as u64,
        _ => bail(90),
    };
    let replacement_control = obj(abi::objtype::RENDEZVOUS, 91);

    let (incumbent, replacement, client) = plan_handoff(w, declared, state, replacement_control);

    start_child(
        &v1,
        &incumbent,
        w.faultep,
        [
            hot_swap_contract::START_FRESH,
            0,
            hot_swap_contract::LAYOUT_1,
        ],
        11,
    );

    // Both replacements built before anyone is talking, for `direct`'s reason: the down window is
    // then a start and an absorb wide, not a build.
    let refuser = build_unstarted(&v2, &replacement, w.faultep, 20);
    let absorber = build_unstarted(&v2, &replacement, w.faultep, 22);
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::BUILT,
        hot_swap_contract::V2,
    );

    start_child(
        &client_img,
        &client,
        w.faultep,
        [hot_swap_contract::ROLE_CLIENT, 0, 0],
        14,
    );
    expect_note(w.note, hot_swap_contract::NOTE_SWAP_NOW, 17);

    // Attempt one: a replacement that cannot read the incumbent's layout.

    let mut corpses = 0u64;
    attempt_refused(w, refuser, &v2, &mut corpses);

    // Attempt two: the incumbent asks again once it has served another SWAP_TRIGGER requests, and
    // this replacement understands the layout. Only its `NOTE_ABSORBED` commits the swap.

    attempt_absorbed(w, absorber, &v2);

    expect_note(w.note, hot_swap_contract::NOTE_CLIENT_DONE, 107);
    reap_to(w.faultep, &mut corpses, 3); // the refuser, the incumbent, and the client

    // Retire the replacement on its own control endpoint, which is not `w.poke`.
    let (verdict, _) = user_mode_runtime::call(w.svc, hot_swap_contract::OPERATION_QUIESCE, 0);
    if verdict != hot_swap_contract::QUIESCED {
        bail(108)
    }
    send(replacement_control, hot_swap_contract::POKE_QUIT, 0, 0);
    reap_to(w.faultep, &mut corpses, 4);

    send(
        REPORT,
        hot_swap_contract::RPT_LOG,
        verdict_from_log(0, false),
        changed_at(0),
    );
    user_mode_runtime::exit()
}

/// **The three plans [`handoff`] builds from**, after the refusal it must see first: the
/// incumbent and the replacements share a declaration and differ only in their `control` endpoint.
fn plan_handoff(
    w: &Wiring,
    declared: capability_plan::StateHandoff,
    state: u64,
    replacement_control: u64,
) -> (
    capability_plan::Plan,
    capability_plan::Plan,
    capability_plan::Plan,
) {
    let to_incumbent = Provisions {
        held: &[
            ("service", w.svc),
            ("report", REPORT),
            ("operator", w.note),
            ("control", w.poke),
            ("witness", w.log_page_frame),
            (capability_plan::STATE_HANDOFF_ROLE, state),
        ],
    };
    let to_replacement = Provisions {
        held: &[
            ("service", w.svc),
            ("report", REPORT),
            ("operator", w.note),
            ("control", replacement_control),
            ("witness", w.log_page_frame),
            (capability_plan::STATE_HANDOFF_ROLE, state),
        ],
    };
    let to_client = Provisions {
        held: &[("service", w.svc), ("report", REPORT), ("operator", w.note)],
    };

    // The control that must fail: the client's routing offers no handoff page, and a stateful
    // component wired without one would be a kill-and-replace that reads like a swap while the
    // tally went to zero.

    match capability_plan::plan(&hot_swap_contract::STATEFUL_BACKEND, &to_client) {
        Ok(_) => bail(92),
        Err(refusal) => send(REPORT, hot_swap_contract::RPT_REFUSED, refusal.code(), 0),
    };

    let Ok(incumbent) = capability_plan::plan(&hot_swap_contract::STATEFUL_BACKEND, &to_incumbent)
    else {
        bail(93)
    };
    let Ok(replacement) =
        capability_plan::plan(&hot_swap_contract::STATEFUL_BACKEND, &to_replacement)
    else {
        bail(94)
    };
    let Ok(client) = capability_plan::plan(&hot_swap_contract::CLIENT, &to_client) else {
        bail(95)
    };
    if incumbent.state_handoff() != Some(declared) {
        bail(96)
    }
    (incumbent, replacement, client)
}

/// Attempt one: a replacement that cannot read the incumbent's layout refuses, exits, and is
/// collected, and the incumbent is told to resume.
fn attempt_refused(w: &Wiring, refuser: Unstarted, v2: &elf::Elf, corpses: &mut u64) {
    drain(w, 97);
    launch(
        refuser,
        v2,
        [
            hot_swap_contract::START_ABSORB,
            0,
            hot_swap_contract::LAYOUT_2,
        ],
        98,
    );
    let (kind, _, found) = receive(w.note);
    if kind != hot_swap_contract::NOTE_NOT_ABSORBED {
        bail(100)
    }
    // The refuser's exit is the only death possible here: the incumbent is quiesced and alive, and
    // the client is parked on the service endpoint's sender queue with nobody receiving.
    let (event, _) = collect_corpse(w.faultep, corpses);
    if event == abi::fault::EVENT_FAULT {
        bail(101)
    }
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::ROLLED_BACK,
        found,
    );
    send(w.poke, hot_swap_contract::POKE_RESUME, 0, 0);
}

/// Attempt two: drain again, start the replacement that understands the layout, and commit the
/// swap on its `NOTE_ABSORBED` by telling the incumbent to quit.
fn attempt_absorbed(w: &Wiring, absorber: Unstarted, v2: &elf::Elf) {
    expect_note(w.note, hot_swap_contract::NOTE_SWAP_NOW, 102);
    drain(w, 103);
    launch(
        absorber,
        v2,
        [
            hot_swap_contract::START_ABSORB,
            0,
            hot_swap_contract::LAYOUT_1,
        ],
        104,
    );
    let (kind, _, carried) = receive(w.note);
    if kind != hot_swap_contract::NOTE_ABSORBED {
        bail(106)
    }
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::ABSORBED,
        carried,
    );
    send(w.poke, hot_swap_contract::POKE_QUIT, 0, 0);
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::STARTED,
        hot_swap_contract::V2,
    );
}

/// Quiesce whoever is serving the stable endpoint, and report the drain.
fn drain(w: &Wiring, stage: u64) {
    let (verdict, served) = user_mode_runtime::call(w.svc, hot_swap_contract::OPERATION_QUIESCE, 0);
    if verdict != hot_swap_contract::QUIESCED {
        bail(stage)
    }
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::DRAINED,
        served,
    );
}

/// A child laid out and endowed but not configured, so it cannot run: `direct`'s step 1, for a
/// component with nothing to defer.
struct Unstarted {
    child: supervision_protocol::Child,
    aspace: u64,
    region: u64,
}

fn build_unstarted(
    elf: &elf::Elf,
    plan: &capability_plan::Plan,
    faultep: u64,
    stage: u64,
) -> Unstarted {
    let Ok(region) = supervision_protocol::memory_region_split(ROOT_UT, plan.pages()) else {
        bail(stage)
    };
    let Ok((child, aspace)) = supervision_protocol::build_child_space(
        ROOT_UT,
        region,
        elf,
        &ChildEndowment {
            caps: plan.caps(),
            maps: plan.maps(),
            blobs: &[],
            fault: Some(faultep),
            ..ChildEndowment::new(Retention::Nothing)
        },
    ) else {
        bail(stage + 1)
    };
    Unstarted {
        child,
        aspace,
        region,
    }
}

fn launch(u: Unstarted, elf: &elf::Elf, args: [u64; 3], stage: u64) {
    cap_delete(launch_keeping_region(u, elf, args, stage));
}

/// [`launch`], returning the child's region capability instead of dropping it.
fn launch_keeping_region(u: Unstarted, elf: &elf::Elf, args: [u64; 3], stage: u64) -> u64 {
    if supervision_protocol::configure_child(u.child.tcb, u.aspace, elf.entry()).is_err() {
        bail(stage)
    }
    if !supervision_protocol::start_child(u.child, args[0], args[1], args[2]) {
        bail(stage + 1)
    }
    u.region
}

// ===============================================================================================
// The opt-in rung: a queue broker between producer and backend, so the producer never blocks on an
// absent consumer.
// ===============================================================================================

/// **When the broker is warned** (DECISIONS §231 (a swap's warning to a dependent is advisory, and
/// the supervisor never waits for it)). The warning is a page the broker reads and a signal that
/// wakes it, and this operator never waits for either to be seen, so the three cases differ only in
/// when the page is written.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Warn {
    /// Down before the backend goes, up once its replacement runs: the queued rung as designed.
    OnTime,
    /// Never: what a supervisor is left with when a dependent will not answer. The measurement
    /// §231 rests on (`ROLE_UNWARNED`).
    Never,
    /// Down and up only after the replacement is already serving: the warning arrives when there
    /// is nothing left to warn about, and must still cost nothing but latency.
    Late,
}

/// Write the broker's warning page and wake it. Never blocks, never waits for an answer.
fn warn_broker(signal: u64, down: bool) {
    let want = if down {
        hot_swap_contract::WANT_DOWN
    } else {
        hot_swap_contract::WANT_UP
    };
    // SAFETY: the warning page is mapped read/write at BROKER_WARN_VA in this operator (and
    // read-only in the broker), by `queued` before the broker was built.
    unsafe { core::ptr::write_volatile(hot_swap_contract::BROKER_WARN_VA as *mut u64, want) };
    let _ = user_mode_runtime::notification_signal(signal, 1);
}

fn queued(fs: &nifefs::Fs, w: &Wiring, warn: Warn) -> ! {
    let v1 = image(fs, "rust_swappable", 2);
    let v2 = image(fs, "c_swappable", 3);
    let client_img = image(fs, "swap_boundary_witness", 4);
    let broker_img = image(fs, "broker", 40);

    // `svc` is the *back* endpoint here: what the broker forwards to and what a backend receives
    // on. `front` is what the producer holds, and it is the stable name on this channel.
    let front = obj(abi::objtype::RENDEZVOUS, 41);
    let base = hot_swap_contract::BROKER_LOG_BASE;

    let QueuedPlans {
        backend,
        broker,
        producer,
        signal,
    } = plan_queued(w, front);

    start_child(&v1, &backend, w.faultep, [0, base, 0], 42);
    start_bound(&broker_img, &broker, w.faultep, signal, 43);
    start_child(
        &client_img,
        &producer,
        w.faultep,
        [hot_swap_contract::ROLE_PRODUCER, 0, 0],
        44,
    );

    expect_note(w.note, hot_swap_contract::NOTE_SWAP_NOW, 45);

    // Whether the broker is warned is the graph's answer, not this function's memory of what it
    // built: `hot_swap_contract::BROKER` declares `depends_on: &["backend"]`, checked against a
    // two-instance live registry by `capability_plan::dependents`.

    let live = [
        capability_plan::LiveInstance {
            id: 1,
            reqs: &hot_swap_contract::BACKEND,
        },
        capability_plan::LiveInstance {
            id: 2,
            reqs: &hot_swap_contract::BROKER,
        },
    ];
    let deps = report_dependents("backend", &live, 64);
    let order = deps.quiesce_order();

    // Tell every dependent the graph named to take custody, without waiting for any (§231): one
    // that never looks costs its callers the down window and nothing else, which `ROLE_UNWARNED`
    // measures. This registry's only dependent is `broker`, id 2.
    for &id in order {
        if id == 2 && warn == Warn::OnTime {
            warn_broker(signal, true);
        }
    }

    // Quiesce the backend and let it die, exactly as on the direct channel, minus the device.
    drain(w, 47);
    send(w.poke, hot_swap_contract::POKE_QUIT, 0, 0);
    let mut corpses = 0u64;
    reap_to(w.faultep, &mut corpses, 1);

    // The replacement, in a different language, on the same back endpoint, wired from the same
    // declaration the outgoing one was.
    start_child(&v2, &backend, w.faultep, [0, base, 0], 48);
    send(
        REPORT,
        hot_swap_contract::RPT_STEP,
        hot_swap_contract::step::STARTED,
        hot_swap_contract::V2,
    );

    // Release the backlog, in the reverse of the order they were warned: the graph's own resume
    // order. The broker drains in arrival order the next time it wakes, before it serves anything
    // newer. On the late channel both warnings go only now, after the swap has finished.
    for &id in order.iter().rev() {
        if id == 2 {
            match warn {
                Warn::OnTime => warn_broker(signal, false),
                Warn::Late => {
                    warn_broker(signal, true);
                    warn_broker(signal, false);
                }
                Warn::Never => {}
            }
        }
    }

    expect_note(w.note, hot_swap_contract::NOTE_CLIENT_DONE, 50);
    reap_to(w.faultep, &mut corpses, 2); // and the producer

    // Shut the channel down so the run leaves nothing running and nothing spent: the broker exits,
    // then the backend quiesces and exits, and both corpses are collected.
    let _ = user_mode_runtime::call(front, hot_swap_contract::OPERATION_QUIESCE, 0);
    expect_note(w.note, hot_swap_contract::NOTE_BROKER_DONE, 51);
    reap_to(w.faultep, &mut corpses, 3); // and the broker
    retire(w, &mut corpses, 4, 52); // and the replacement backend

    send(
        REPORT,
        hot_swap_contract::RPT_LOG,
        verdict_from_log(base, false),
        changed_at(base),
    );
    user_mode_runtime::exit()
}

/// What [`queued`] builds its three children from, and the signal that wakes the broker.
struct QueuedPlans {
    backend: capability_plan::Plan,
    broker: capability_plan::Plan,
    producer: capability_plan::Plan,
    signal: u64,
}

/// **The queued channel's routing**: the broker's warning page and signal, the refusal it must see
/// first, and the three plans.
fn plan_queued(w: &Wiring, front: u64) -> QueuedPlans {
    // The producer's declaration is `CLIENT`, the direct channel's client's, and here its `service`
    // resolves to the broker's front endpoint: one declaration, two routings, and the program
    // cannot tell which it got.
    let to_backend = Provisions {
        held: &[
            ("service", w.svc),
            ("report", REPORT),
            ("operator", w.note),
            ("control", w.poke),
            ("witness", w.log_page_frame),
        ],
    };
    // The warning page and the signal that wakes the broker to read it (§231). The page is mapped
    // here read/write so this operator can write it, and routed to the broker read-only by its
    // declaration.
    let warning = page_frame(53);
    if !map_page_frame(warning, hot_swap_contract::BROKER_WARN_VA, true, ROOT_UT) {
        bail(54)
    }
    let signal = obj(abi::objtype::NOTIFICATION, 55);
    let to_broker = Provisions {
        held: &[
            ("requests", front),
            ("backend", w.svc),
            ("report", REPORT),
            ("operator", w.note),
            ("warning", warning),
        ],
    };
    let to_producer = Provisions {
        held: &[("service", front), ("report", REPORT), ("operator", w.note)],
    };

    // The control that must fail, the direct channel's mirror: no device is routed here, so the
    // console's declaration is refused before anything is built.
    match capability_plan::plan(&hot_swap_contract::CONSOLE, &to_backend) {
        Ok(_) => bail(60),
        Err(refusal) => send(REPORT, hot_swap_contract::RPT_REFUSED, refusal.code(), 0),
    };

    // No device on this channel: the backend behind a broker is a plain service, and mixing the
    // device story into the queue story would make it unclear which mechanism carried which claim.
    let Ok(backend) = capability_plan::plan(&hot_swap_contract::BACKEND, &to_backend) else {
        bail(61)
    };
    let Ok(broker) = capability_plan::plan(&hot_swap_contract::BROKER, &to_broker) else {
        bail(62)
    };
    let Ok(producer) = capability_plan::plan(&hot_swap_contract::CLIENT, &to_producer) else {
        bail(63)
    };
    QueuedPlans {
        backend,
        broker,
        producer,
        signal,
    }
}

// ===============================================================================================
// The third residual: a component that stops answering **without dying**. It holds its endpoint
// and its device, sits `Blocked` like a healthy server waiting for work, and sends no death
// message. The operator notices by its quiesce going unanswered within a bound, then restores the
// service and destroys the hung incumbent's region. See notes/hung-component.md.
// ===============================================================================================

/// **How long the operator waits for `OPERATION_QUIESCE` to be answered before it treats the
/// incumbent as hung**, in milliseconds of the counter `user_mode_runtime::now` reads.
///
/// About 38 times the slowest healthy answer measured on three ISAs on a loaded host, timed as
/// [`quiesce_within`] times it; the deadline fires at the first 10 ms tick at or after it, so the
/// real wait is 250 to 260 ms. Wall time under TCG, and this fixture's bound only: too short
/// destroys a healthy component mid-work, too long costs only a longer down window. The
/// measurement is in notes/hung-component/what-counts-as-hung.md.
const QUIESCE_BOUND_MS: u64 = 250;

/// What a bounded quiesce found.
enum Quiesce {
    /// The incumbent answered within the bound: it is drained, exactly as [`drain`] leaves one.
    Answered,
    /// The bound passed with no answer. `waited` is in counter ticks; `stand_in` is the region of
    /// the instance still queued on the stable endpoint with the unanswered quiesce.
    Unanswered { waited: u64, stand_in: u64 },
}

fn hung(fs: &nifefs::Fs, w: &Wiring, device: bool) -> ! {
    let v1 = image(fs, "rust_swappable", 2);
    let v2 = image(fs, "c_swappable", 3);
    let client_img = image(fs, "swap_boundary_witness", 4);

    // The direct channel's routing: what differs is one start argument, which carries no
    // authority. Without a device the contract is `BACKEND`, the console's capability half with no
    // registers, and the routing offers no `uart`.
    let held_with_device: &[(&str, u64)] = &[
        ("service", w.svc),
        ("report", REPORT),
        ("operator", w.note),
        ("control", w.poke),
        ("witness", w.log_page_frame),
        ("uart", DEVICE),
    ];
    let to_component = Provisions {
        held: if device {
            held_with_device
        } else {
            &held_with_device[..5]
        },
    };
    let to_client = Provisions {
        held: &[("service", w.svc), ("report", REPORT), ("operator", w.note)],
    };
    let contract = if device {
        &hot_swap_contract::CONSOLE
    } else {
        &hot_swap_contract::BACKEND
    };
    let Ok(component) = capability_plan::plan(contract, &to_component) else {
        bail(70)
    };
    let Ok(client) = capability_plan::plan(&hot_swap_contract::CLIENT, &to_client) else {
        bail(71)
    };

    // The incumbent, told which one request it will never answer. **Its region is kept**, unlike
    // every other child this operator starts: the region capability is the authority that ends a
    // child that will never cooperate (milestone 133 (ending a permanently blocked thread, and
    // deciding who may)), and deleting it here is what left the hung incumbent unreclaimable.
    let incumbent = start_child_keeping_region(
        &v1,
        &component,
        w.faultep,
        [device as u64, 0, hot_swap_contract::WEDGE_SEQ],
        72,
    );

    // The replacement, built ahead and not configured, for the direct channel's reason; a hung
    // component gives no drain to hide a build behind. With no device in the plan, everything
    // except the device is everything.
    let replacement = build_without_devices(&v2, &component, w.faultep, 75);

    // The stand-in that will carry the quiesce, built now for the replacement's reason: starting it
    // is then one syscall, so the bound measures the incumbent and not a build.
    let stand_in = build_unstarted(&client_img, &client, w.faultep, 110);

    start_child(
        &client_img,
        &client,
        w.faultep,
        [hot_swap_contract::ROLE_CLIENT, 0, 0],
        77,
    );
    expect_note(w.note, hot_swap_contract::NOTE_SWAP_NOW, 80);

    // Step 2, bounded: nothing in the trigger says the incumbent has stopped, so the drain is
    // asked as on the direct channel with a deadline (milestone 106 (a wait that ends on either the
    // interrupt or the deadline)). No answer within the bound means hung.

    let bound = cntfrq() * QUIESCE_BOUND_MS / 1000;
    let (waited, stand_in) = match quiesce_within(w, stand_in, &client_img, bound, 112) {
        Quiesce::Unanswered { waited, stand_in } => (waited, stand_in),
        // This role exists to run against a hung incumbent, so an answer means the fixture did not
        // hang and the run tested nothing. A supervisor with a real healthy path would carry on as
        // `direct` does from its drain.
        Quiesce::Answered => bail(81),
    };
    send(REPORT, hot_swap_contract::RPT_UNANSWERED, bound, waited);

    // What the supervisor can see, two negative results: `SURVEY` shows every member `BLOCKED` and
    // none `DEAD`, as a healthy idle system does, and `Rendezvous::REAP` (§32 (a supervisor may
    // collect a corpse without being able to build one)) refuses every member `StillAlive`. The
    // verdict above came from the clock, not from this view.

    let (members, states) = survey_domain(w.faultep);
    send(REPORT, hot_swap_contract::RPT_SURVEY, members, states);
    let (asked, refused) = ask_the_domain_to_be_collected(w.faultep);
    send(REPORT, hot_swap_contract::RPT_UNCOLLECTABLE, asked, refused);

    // The stand-in goes first: parked on the stable endpoint's sender queue with a quiesce, it
    // would drain the replacement the moment it started. Destroying its region finishes it in place
    // and unlinks it from the queue (milestone 133), so the request is never delivered.

    end_region(stand_in, 117);

    // Recovery needs no authority this operator did not already hold: `PageFrame::REVOKE` is
    // `GRANT`-gated take-back (§41) that asks a wedged holder for nothing, there is nothing to
    // drain, and the replacement picks up whatever queued behind the silence.

    if device {
        revoke_device(82);
    }
    endow_devices_and_start(replacement, &v2, &component, 83);

    // Reclaim the hung incumbent with the region capability kept for it, §32's stronger right.
    // `DESTROY` finishes it without waking it and answers the caller it swallowed with `Gone`
    // (milestone 254 (a caller stranded by a server that died)), who asks the replacement again.
    // After `STARTED`, so that retry finds a server. The witness is the budget's live-children
    // page count, read on either side.

    let before = live_children_pages(118);
    end_region(incumbent, 119);
    let after = live_children_pages(118);
    send(REPORT, hot_swap_contract::RPT_RECLAIMED, before, after);

    // The incumbent leaves no corpse: it was finished, not killed, and a finished thread sends no
    // death message. So the deaths still to collect are the client's and the replacement's.
    let mut corpses = 0u64;
    expect_note(w.note, hot_swap_contract::NOTE_CLIENT_DONE, 86);
    reap_to(w.faultep, &mut corpses, 1); // the client
    retire(w, &mut corpses, 2, 87); // and the replacement

    // No fault receipt for the revoke on this channel: the holder it was taken from never runs
    // again, so nothing can touch the registers to prove they are gone. The direct channel carries
    // that receipt.
    send(
        REPORT,
        hot_swap_contract::RPT_LOG,
        verdict_from_log(0, false),
        changed_at(0),
    );
    user_mode_runtime::exit()
}

/// **`OPERATION_QUIESCE` with a deadline**, which a `CALL` cannot have. Starts `stand_in` as
/// [`hot_swap_contract::ROLE_QUIESCER`] to make the call, and waits for its forwarded answer on
/// `w.note` in a `RECEIVE` that the bound notification ends at `bound` counter ticks (the timer
/// our spawner granted, `net_stack`'s retransmit-wait pattern). Bails `stage` through `stage + 4`:
/// the first two are [`launch_keeping_region`]'s.
///
/// Nothing else sends on `w.note` while this waits: the incumbent has sent its trigger, and the
/// client's next note comes only after its last request, which cannot be served before the drain.
fn quiesce_within(
    w: &Wiring,
    stand_in: Unstarted,
    elf: &elf::Elf,
    bound: u64,
    stage: u64,
) -> Quiesce {
    let started = now();
    let region = launch_keeping_region(
        stand_in,
        elf,
        [hot_swap_contract::ROLE_QUIESCER, 0, 0],
        stage,
    );
    if timer_arm(TIMER, started + bound, WAKE, 1) < 0 {
        bail(stage + 2)
    }
    match receive_bound(w.note) {
        user_mode_runtime::Received::Message(kind, answer, _)
            if kind == hot_swap_contract::NOTE_QUIESCE_ANSWERED =>
        {
            // A deadline that passed between the answer and this cancel has already signaled, and
            // its word waits on the notification for the next bound receive. At this bound against
            // a healthy answer's latency (see `QUIESCE_BOUND_MS`) that window is never reached.
            let _ = user_mode_runtime::timer_cancel(TIMER);
            if answer != hot_swap_contract::QUIESCED {
                bail(stage + 3)
            }
            Quiesce::Answered
        }
        user_mode_runtime::Received::Notification(_) => Quiesce::Unanswered {
            waited: now() - started,
            stand_in: region,
        },
        _ => bail(stage + 4),
    }
}

/// **Destroy a child's region, whatever its resident is doing**, and bail `stage` if it cannot be.
///
/// A `Blocked` resident is finished in place on the first try (milestone 133). A `Ready` or
/// `Running` one is marked killed and the `DESTROY` refused; it dies at its next preemption, and a
/// retry then finds a corpse and reclaims. The retries are that contract, not a wait for anything:
/// every resident this is used on has been parked far longer than a scheduling quantum.
fn end_region(region: u64, stage: u64) {
    for _ in 0..1000 {
        if supervision_protocol::memory_region_destroy(region) {
            return;
        }
        user_mode_runtime::yield_now();
    }
    bail(stage)
}

/// **Pages this operator's budget has in live child regions**, `MemoryRegion::USAGE`'s `CHILDREN`
/// record (milestone 126 (the `procps` package)). The budget arrives with every right, so it holds
/// `ENUMERATE`.
fn live_children_pages(stage: u64) -> u64 {
    let r = user_mode_runtime::memory_region_usage(ROOT_UT, abi::usage::CHILDREN);
    if r < 0 {
        bail(stage)
    }
    r as u64
}

/// **Walk the supervision domain and count what state its members are in** (milestone 126's
/// `abi::rendezvous::SURVEY`). Returns `(members, states)`, the second packed by
/// `hot_swap_contract::survey_counts`.
///
/// A refusal is reported as `(u64::MAX, error)` rather than as an empty domain, for the reason
/// milestone 126 built the method with: a monitor that reports nothing because it could not look is
/// the worst failure it has available. This operator holds the endpoint it retyped itself, so it
/// holds `ENUMERATE` and cannot be refused; saying so in code is cheaper than assuming it.
fn survey_domain(faultep: u64) -> (u64, u64) {
    let mut counts = [0u64; 5];
    let mut members = 0u64;
    let mut cursor = abi::survey::DONE;
    loop {
        let (next, _tid, state) = user_mode_runtime::survey(faultep, cursor);
        if next < 0 {
            return (u64::MAX, (-next) as u64);
        }
        let next = next as u64;
        if next == abi::survey::DONE {
            break;
        }
        // A cursor that did not advance would spin here forever. The kernel does not do this; the
        // check is here so this loop terminates for every reader, which is `ps::collect`'s rule too.
        if next <= cursor {
            break;
        }
        if (state as usize) < counts.len() {
            counts[state as usize] += 1;
        }
        members += 1;
        cursor = next;
    }
    (
        members,
        hot_swap_contract::survey_counts(
            counts[abi::survey::READY as usize],
            counts[abi::survey::RUNNING as usize],
            counts[abi::survey::BLOCKED as usize],
            counts[abi::survey::DEAD as usize],
        ),
    )
}

/// **Ask the kernel to collect every member of the domain, and count the refusals.** Returns
/// `(asked, refused_still_alive)`.
///
/// The assertion that a supervisor's whole vocabulary over a live domain is empty:
/// `Rendezvous::REAP` (DECISIONS §32) collects a corpse and refuses a thread still alive, because
/// killing is the stronger act and lives elsewhere. Side-effect free: `reap_supervised` decides
/// `StillAlive` before it looks up a region, so a refused reap moves nothing and frees nothing.
fn ask_the_domain_to_be_collected(faultep: u64) -> (u64, u64) {
    // The raw syscall return is the negative discriminant (`StillAlive = -9`), which `invoke`
    // hands back unchanged; a reporter that negates it (`swap_boundary_witness`) is the other case.
    let still_alive = abi::Error::StillAlive as i64;
    let mut asked = 0u64;
    let mut refused = 0u64;
    let mut cursor = abi::survey::DONE;
    loop {
        let (next, tid, _state) = user_mode_runtime::survey(faultep, cursor);
        if next < 0 {
            break;
        }
        let next = next as u64;
        if next == abi::survey::DONE || next <= cursor {
            break;
        }
        asked += 1;
        if user_mode_runtime::reap(faultep, tid) == still_alive {
            refused += 1;
        }
        cursor = next;
    }
    (asked, refused)
}

// ===============================================================================================
// The pieces every role shares.
// ===============================================================================================

/// Split a region, build a child in it, start it, and drop both capabilities. Neither is the thing
/// itself: a TCB capability is not the thread (dropping it leaves the thread running), and since
/// DECISIONS §32 the region capability is not the reap either.
///
/// **It takes a plan rather than an endowment**, which is milestone 23's manifest lane in one
/// signature: the caller says which component and which supervision endpoint, and every capability,
/// every mapping and the region size come from the component's own declaration. `args` stays a
/// caller's business because it is configuration and not authority (which log entries this instance
/// writes, and which of `swap_boundary_witness`'s three roles it is playing).
fn start_child(
    elf: &elf::Elf,
    plan: &capability_plan::Plan,
    faultep: u64,
    args: [u64; 3],
    stage: u64,
) {
    cap_delete(start_child_keeping_region(elf, plan, faultep, args, stage));
}

/// [`start_child`], returning the child's region capability instead of dropping it: the authority
/// to end the child whatever it is doing ([`end_region`]).
fn start_child_keeping_region(
    elf: &elf::Elf,
    plan: &capability_plan::Plan,
    faultep: u64,
    args: [u64; 3],
    stage: u64,
) -> u64 {
    let Ok(region) = supervision_protocol::memory_region_split(ROOT_UT, plan.pages()) else {
        bail(stage)
    };
    let endow = ChildEndowment {
        caps: plan.caps(),
        maps: plan.maps(),
        blobs: &[],
        fault: Some(faultep),
        ..ChildEndowment::new(Retention::Nothing)
    };
    let Ok(child) = supervision_protocol::build_child(ROOT_UT, region, elf, &endow) else {
        bail(stage + 1)
    };
    if !supervision_protocol::start_child(child, args[0], args[1], args[2]) {
        bail(stage + 2)
    }
    region
}

/// [`start_child`], for a child with a notification bound to its thread before it runs: `broker`,
/// whose §231 warning arrives as a signal while it is parked receiving on its front endpoint.
fn start_bound(
    elf: &elf::Elf,
    plan: &capability_plan::Plan,
    faultep: u64,
    signal: u64,
    stage: u64,
) {
    let Ok(region) = supervision_protocol::memory_region_split(ROOT_UT, plan.pages()) else {
        bail(stage)
    };
    let endow = ChildEndowment {
        caps: plan.caps(),
        maps: plan.maps(),
        blobs: &[],
        fault: Some(faultep),
        ..ChildEndowment::new(Retention::ThreadControlBlock {
            reason: "bind the broker's warning notification before it runs",
        })
    };
    let Ok(child) = supervision_protocol::build_child(ROOT_UT, region, elf, &endow) else {
        bail(stage + 1)
    };
    let tcb = child.tcb;
    if user_mode_runtime::notification_bind(signal, tcb) != 0 {
        bail(stage + 2)
    }
    if !supervision_protocol::start_child(child, 0, 0, 0) {
        bail(stage + 2)
    }
    cap_delete(tcb);
    cap_delete(region);
}

/// Wait for one death, report it, and collect the corpse through the supervision endpoint it
/// arrived on (DECISIONS §32). Returns `(event, fault address)`.
///
/// **Every child this operator starts is supervised and every corpse is collected**, so a run ends
/// with all five instance regions back in this budget. The one that is not tidiness is the one
/// below; the rest are here so the test can assert that a swap system reclaims itself.
fn collect_corpse(faultep: u64, collected: &mut u64) -> (u64, u64) {
    let (event, tid, _pc, addr, ..) = receive_fault(faultep);
    send(REPORT, hot_swap_contract::RPT_DEATH, tid, event);
    // We hold no capability to that region: we deleted it the moment the child was started, and the
    // authority for this is the supervision relationship, not the memory.
    if user_mode_runtime::reap(faultep, tid) != 0 {
        bail(27)
    }
    *collected += 1;
    (event, addr)
}

/// Collect corpses until `target` of them have been collected in total.
///
/// **Counted, not ordered.** Deaths arrive in whatever order the children happen to die in, and a
/// program that waited for a *particular* child's death would hang the first time two of them
/// finished the other way round. Counting is enough because the only thing done with a corpse here
/// is reap it, and the one death whose identity matters is picked out by [`wait_for_fault`].
fn reap_to(faultep: u64, collected: &mut u64, target: u64) {
    while *collected < target {
        collect_corpse(faultep, collected);
    }
}

/// **Wait for the receipt.** Collect corpses until one is a *fault*, and say whether it faulted on
/// the page `expect_addr` sits in.
///
/// A loop rather than a single receive because the client may finish and exit at any moment, and a
/// clean exit arriving first must not be mistaken for the answer. Telling them apart is exactly
/// what DECISIONS §26's two event codes are for.
///
/// The page, not the byte: the fault address the kernel reports is the *register* the component was
/// reading (`DEV_VA + 0x18` for a PL011's flag register), and what the revoke took away is the page.
/// Comparing the whole address would have been comparing the register layout.
fn wait_for_fault(faultep: u64, collected: &mut u64, expect_addr: u64) -> bool {
    loop {
        let (event, addr) = collect_corpse(faultep, collected);
        if event == abi::fault::EVENT_FAULT {
            send(REPORT, hot_swap_contract::RPT_SITE, addr, expect_addr);
            send(
                REPORT,
                hot_swap_contract::RPT_STEP,
                hot_swap_contract::step::REAPED,
                0,
            );
            return addr & !(hot_swap_contract::PAGE - 1) == expect_addr;
        }
    }
}

/// Retire the last live instance on a channel: quiesce it, tell it to go, collect its corpse. The
/// swap's own machinery, run once more with nothing to replace.
fn retire(w: &Wiring, collected: &mut u64, target: u64, stage: u64) {
    let (verdict, _) = user_mode_runtime::call(w.svc, hot_swap_contract::OPERATION_QUIESCE, 0);
    if verdict != hot_swap_contract::QUIESCED {
        bail(stage)
    }
    send(w.poke, hot_swap_contract::POKE_QUIT, 0, 0);
    reap_to(w.faultep, collected, target);
}

fn expect_note(note: u64, want: u64, stage: u64) -> u64 {
    let (kind, w1, _) = receive(note);
    if kind != want {
        bail(stage)
    }
    w1
}

fn image<'a>(fs: &nifefs::Fs<'a>, name: &str, stage: u64) -> elf::Elf<'a> {
    match fs.read(name).map(elf::Elf::parse) {
        Some(Ok(e)) => e,
        _ => bail(stage),
    }
}

fn obj(objtype: u64, stage: u64) -> u64 {
    match supervision_protocol::retype_obj_from(ROOT_UT, objtype) {
        Ok(s) => s,
        Err(()) => bail(stage),
    }
}

fn page_frame(stage: u64) -> u64 {
    match supervision_protocol::retype_page_frame_from(ROOT_UT) {
        Ok(s) => s,
        Err(()) => bail(stage),
    }
}

/// **The operator's verdict**, read out of the log page byte by byte.
fn verdict_from_log(base: u64, revoke_enforced: bool) -> u64 {
    let mut bits = lc::NO_GAP | lc::MONOTONE;
    let mut seen_v1 = false;
    let mut seen_v2 = false;
    let mut last = 0u64;
    for seq in 0..hot_swap_contract::REQUESTS {
        let v = hot_swap_contract::log_get(base + seq);
        if v == 0 {
            bits &= !lc::NO_GAP; // a request nobody served: lost in the down window
        }
        if v < last {
            bits &= !lc::MONOTONE; // the old instance answered after the new one: two owners
        }
        if v == hot_swap_contract::V1 {
            seen_v1 = true;
        }
        if v == hot_swap_contract::V2 {
            seen_v2 = true;
        }
        last = v;
    }
    if seen_v1 && seen_v2 {
        bits |= lc::BOTH_VERSIONS;
    }
    if revoke_enforced {
        bits |= lc::REVOKE_ENFORCED;
    }
    bits
}

/// The sequence number the version changed at, so the test can see the swap landed inside the
/// conversation rather than at one of its ends.
fn changed_at(base: u64) -> u64 {
    let mut last = 0u64;
    for seq in 0..hot_swap_contract::REQUESTS {
        let v = hot_swap_contract::log_get(base + seq);
        if last != 0 && v != last {
            return seq;
        }
        last = v;
    }
    0
}

/// Report which stage failed, then trap. A half-built system is not worth limping along, and the
/// stage code turns "nothing happened" into a legible failure.
fn bail(stage: u64) -> ! {
    send(REPORT, hot_swap_contract::RPT_FAILED, stage, 0);
    hot_swap_contract::fail()
}

user_mode_runtime::panic_handler!();
