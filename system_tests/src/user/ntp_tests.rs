use core::sync::atomic::Ordering;

use clock_contract::{NANOS_PER_SEC, policy, state, status};
use ntp_service::{ATTEMPTS, reject, rpt, srv};

use super::*;
use crate::arch::exceptions::USER_FAULTS;
use crate::user::holding::Holding;

/// Bounded by wall clock rather than by a yield count: since DECISIONS §28 the work a test
/// spawns runs on other cores, so a yield on an idle core elapses no real time. Two seconds is
/// far under the 60 s hang watchdog, so a genuine lost wakeup still fails loudly.
fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        crate::sched::yield_now();
    }
    cond()
}

/// The three images milestone 290 split the one `ntp` binary into. Named separately because after
/// the split each test says which program it is spawning, and a test that spawns the witness where
/// it meant the client now fails to compile rather than passing for the wrong reason.
fn client_image() -> &'static [u8] {
    program("network_time_client").expect("no network_time_client program in the initial ramdisk")
}

fn test_server_image() -> &'static [u8] {
    program("network_time_test_server")
        .expect("no network_time_test_server program in the initial ramdisk")
}

fn witness_image() -> &'static [u8] {
    program("unwritable_clock_witness")
        .expect("no unwritable_clock_witness program in the initial ramdisk")
}

/// **A fresh clock service per test.** Each one allocates its own page and reads the RTC again,
/// so a step taken in one test cannot bias the next one's bounds, and each test's "before" is
/// genuinely before.
///
/// Built in a region of its own and returned with the [`Holding`] that ends it, which the test
/// releases last: the service parks in `RECEIVE` on `propose` for good, so without the holding
/// each test would leave a thread and two endpoints behind (milestone 608 (kernel tests give back
/// their rendezvous points)).
fn clock() -> (clock_service::Wiring, Holding) {
    let image = program("clock").expect("no clock program in the initial ramdisk");
    let region =
        crate::memory_region::create(clock_service::REGION_PAGES).expect("no region for a clock");
    let (w, thread) = clock_service::start_in(image, region);
    let mut held = Holding::new();
    held.add_thread(thread);
    held.add_region(region);
    let report = crate::sched::ipc_receive(w.report);
    assert!(
        state::is_known(report[1]),
        "the clock service does not know the time, so there is nothing for NTP to correct",
    );
    (w, held)
}

/// The entropy service's request endpoint: the client's whole authority over randomness, and it
/// names no device. `ensure` wires the service once per boot; whichever test asks first pays.
fn entropy() -> Option<crate::sched::RendezvousId> {
    let image = program("entropy").expect("no entropy program in the initial ramdisk");
    let w = entropy_service::ensure(image, entropy_service::Bus::Mmio)?;
    if let Some(report) = w.wait_for_ready() {
        assert_eq!(
            report[0],
            entropy_contract::READY,
            "the entropy service did not come up (it reported {:#x})",
            report[0],
        );
    }
    Some(w.request)
}

/// Whether this machine has no source of unguessable bytes for the client's nonce, checked before
/// any test that needs a complete exchange: the client refuses the network without one
/// (`without_entropy_the_client_refuses_rather_than_guessing`), the convention `disk_tests`,
/// `credential_tests` and `entropy_tests` share. x86_64 is the case today: its runner attaches no
/// virtio-rng (notes/x86-port.md), where both QEMU `virt` boards have one on mmio.
///
/// It goes through [`entropy`], never `ensure` directly, and that is load-bearing: the first
/// `ensure` caller gets the service's `ready` endpoint, the service announces itself with a
/// blocking send, and a discarded `Wiring` parks it in its startup for good, so the client blocks
/// in `call(ENTROPY, ...)` and this reads as "the test server never saw a request".
fn machine_has_no_entropy() -> bool {
    entropy().is_none()
}

/// Where the client is told to send. Nothing listens there; the test server is behind the
/// endpoint, not behind the address. The address is asserted anyway, because a client that
/// invented its own destination would be one that ignores its wiring.
const SERVER_IP: u32 = 0x0a00_0202; // 10.0.2.2, slirp's gateway
const SERVER_PORT: u16 = network_time_protocol::PORT;

/// Run one exchange: a fresh server at `claimed_nanos`, a fresh client, both reports drained in
/// the order the blocking `send` requires. Returns `(the server's report, the client's report)`.
///
/// Both programs and their three endpoints are handed back before this returns. The server never
/// exits on its own (it serves until killed), so the holding is what ends it.
fn exchange(
    clock: &clock_service::Wiring,
    variant: u64,
    claimed_nanos: u64,
) -> ([u64; 5], [u64; 5]) {
    // Counted before either program exists, so a fault during the exchange is told apart from a
    // client that is merely slow or blocked. Merge-group run 38284891615 (2026-10-12 UTC) failed
    // the first assertion below because the client had been killed by a page fault, and the
    // message alone read as a network problem.
    let faults_before = USER_FAULTS.load(Ordering::Acquire);
    let region = crate::memory_region::create(ntp_service::EXCHANGE_PAGES)
        .expect("no endpoint region for an ntp exchange");
    let mut held = Holding::new();
    held.add_region(region);
    let server = ntp_service::start_server(
        test_server_image(),
        variant,
        claimed_nanos,
        region,
        &mut held,
    );
    let client = ntp_service::start_client(
        client_image(),
        server.stack,
        clock.propose,
        entropy(),
        SERVER_IP,
        SERVER_PORT,
        region,
        &mut held,
    );
    // The server blocks in its one `send` until this returns, and the client's RECEIVE is queued
    // behind it, so the order here is load-bearing rather than stylistic. The bounded wait is
    // what turns "the client never sent a request" into a failure that names itself instead of
    // a sixty-second watchdog hang: `ipc_receive` on an endpoint nobody will ever send on does not
    // come back.
    assert!(
        wait_for(|| crate::sched::rendezvous_waiting_senders(server.report) > 0),
        "the test server never saw a request: the client failed before it reached the network \
         ({} user faults during the exchange, the last {:?}; a fault here is a dead client, not \
         a network problem, and its kill report is in the transcript above)",
        USER_FAULTS.load(Ordering::Acquire) - faults_before,
        crate::arch::exceptions::last_user_fault(),
    );
    let served = crate::sched::ipc_receive(server.report);
    assert!(
        wait_for(|| crate::sched::rendezvous_waiting_senders(client) > 0),
        "the client never reported: it is still blocked somewhere in the exchange ({} user \
         faults during the exchange, the last {:?})",
        USER_FAULTS.load(Ordering::Acquire) - faults_before,
        crate::arch::exceptions::last_user_fault(),
    );
    let reported = crate::sched::ipc_receive(client);
    held.release_or_fail("an ntp exchange");
    (served, reported)
}

/// **The exchange lands as a proposal, and the service is what decides.**
///
/// The whole path in one test: eight bytes of real entropy become a nonce, a well-formed NTPv4
/// client packet goes out to port 123, a server reply comes back, `Query::accept` produces a
/// sample, and the correction reaches the clock **through the propose endpoint**. The proof that
/// it went that way rather than by a write is the state word: an accepted proposal publishes
/// `SYNCED`, which nothing but the service can produce.
#[test_case]
fn an_ntp_exchange_reaches_the_clock_as_a_proposal() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    if machine_has_no_entropy() {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    }
    let (clock, clock_held) = clock();
    let before = clock.page().read();
    let step = NANOS_PER_SEC / 2;
    let wall_before = clock.wall_nanos();
    let claimed = wall_before + step;

    let (served, reported) = exchange(&clock, srv::GOOD, claimed);

    // What the server saw on the wire: the packet is ours and it is addressed where it was told.
    assert_eq!(served[0], rpt::SERVED);
    assert_eq!(
        served[2] >> 32,
        network_time_protocol::PORT as u64,
        "the client did not address UDP 123",
    );
    assert_eq!(
        (served[2] >> 8) & 0xff,
        network_time_protocol::VERSION as u64,
        "not an NTPv4 request",
    );
    assert_eq!(
        served[2] & 0xff,
        network_time_protocol::mode::CLIENT as u64,
        "not a client-mode request: a server would take this for an unsolicited packet",
    );

    assert_eq!(
        reported[0],
        rpt::SYNCED,
        "the client did not synchronise (report {:?})",
        reported,
    );
    assert_eq!(
        reported[1],
        status::ACCEPTED,
        "the clock service refused a half-second correction",
    );

    let after = clock.page().read();
    assert_eq!(
        after.state,
        state::SYNCED,
        "the clock is {} rather than SYNCED: an accepted proposal is not a SET, and only the \
         service can publish this",
        after.state,
    );
    assert_eq!(
        after.generation,
        before.generation + 1,
        "exactly one publish: the proposal",
    );

    // And the clock really moved, by about what the server claimed. The lower bound is what
    // fails if the proposal was dropped on the floor; the upper bound is what fails if the
    // client believed a server it should have bounded.
    let moved = clock.wall_nanos() - wall_before;
    assert!(
        moved >= step * 4 / 5,
        "the wall clock moved {moved} ns, less than the {step} ns the server claimed",
    );
    assert!(
        moved < step + 3 * NANOS_PER_SEC,
        "the wall clock moved {moved} ns, far past the {step} ns the server claimed",
    );
    clock_held.release_or_fail("the clock service");
}

/// **A reply that fails validation moves nothing.**
///
/// Three replies a real client meets, each a different situation: an origin that is not our nonce
/// is the off-path spoof the check exists for, a kiss-o'-death is an instruction, and twenty bytes
/// is something not NTP arriving on our socket. `network_time_protocol` proves the checks over
/// 2^384 packets; this proves the client honours the verdict. A rejected reply is retried (it may
/// have been a spoof that beat the real server); a kiss-o'-death is not, because retrying into one
/// is the abusive behaviour the packet exists to stop, and the attempt count asserts both.
#[test_case]
fn a_reply_that_fails_validation_never_becomes_a_proposal() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    if machine_has_no_entropy() {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    }
    let (clock, clock_held) = clock();
    let before = clock.page().read();
    let claimed = clock.wall_nanos() + NANOS_PER_SEC / 2;

    for (variant, code, attempts, what) in [
        (
            srv::BAD_ORIGIN,
            reject::ORIGIN_MISMATCH,
            ATTEMPTS,
            "an origin that is not the nonce we sent",
        ),
        (
            srv::KISS_OF_DEATH,
            reject::KISS_OF_DEATH,
            1,
            "a kiss-o'-death, which must not be retried",
        ),
        (
            srv::SHORT,
            reject::LENGTH,
            ATTEMPTS,
            "twenty bytes, which is not an NTP packet",
        ),
    ] {
        let (_, reported) = exchange(&clock, variant, claimed);
        assert_eq!(
            reported[0],
            rpt::REJECTED,
            "the client accepted {what} (report {reported:?})",
        );
        assert_eq!(reported[1], code, "the wrong check refused {what}");
        assert_eq!(
            reported[2], attempts,
            "the client made {} requests against {what}, not {attempts}",
            reported[2],
        );
        assert_eq!(
            clock.page().read(),
            before,
            "the clock page changed after {what}: a refused reply reached the offset",
        );
    }
    clock_held.release_or_fail("the clock service");
}

/// **The bounds are the service's, and the client cannot argue with them.**
///
/// The client does everything right here: a valid sample, an honest proposal, a truthful report.
/// It is the *service* that says no, twice, and asymmetrically: two hours forward is refused for
/// walking past an expiry, and ten seconds backward is refused because moving backward makes
/// instants happen twice (DECISIONS §43). A compromised client can lie inside those bounds and
/// can do nothing else, which is the sentence this test is here to make true rather than
/// asserted.
#[test_case]
fn a_proposal_outside_the_policy_is_refused_by_the_service() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    if machine_has_no_entropy() {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    }
    let (clock, clock_held) = clock();
    let before = clock.page().read();

    for (claimed, want, what) in [
        (
            clock.wall_nanos() + 2 * policy::MAX_STEP_FORWARD_NANOS,
            status::REFUSED_TOO_FAR_FORWARD,
            "two hours forward",
        ),
        (
            clock.wall_nanos() - 10 * NANOS_PER_SEC,
            status::REFUSED_TOO_FAR_BACKWARD,
            "ten seconds backward",
        ),
    ] {
        let (_, reported) = exchange(&clock, srv::GOOD, claimed);
        assert_eq!(
            reported[0],
            rpt::SYNCED,
            "the client should have accepted the sample and proposed it ({what}, report \
             {reported:?}); the refusal under test is the service's, not the client's",
        );
        assert_eq!(
            reported[1], want,
            "the service answered {} to a server claiming {what}",
            reported[1],
        );
        assert_eq!(
            clock.page().read(),
            before,
            "the clock page changed on a refused proposal ({what})",
        );
    }
    clock_held.release_or_fail("the clock service");
}

/// **The client holds no writable clock page, and knowing where one would be buys it nothing.**
///
/// `unwritable_clock_witness`, given the same five slots an NTP client is given plus the address at
/// which a holder of the set authority maps the clock page, reports the address, writes there, and
/// dies. The boundary is the mapping, not the layout, so no address works; this is the one that
/// would matter. The fault comes from the capability set, not the binary: `start_client` and
/// `start_witness` share `ntp_service::spawn_with_client_endowment`, the single grant list, so a
/// sixth slot for the client is a sixth for the witness. `ntpd` runs as root, and no address in a
/// Unix system is out of its `settimeofday`'s reach.
#[test_case]
fn an_ntp_client_holds_no_writable_clock_page() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    let (clock, clock_held) = clock();
    let before = clock.page().read();
    // An endpoint nobody serves: the witness never sends a request, and giving it a real server
    // would only add a process to the boot. It is granted anyway, because the point is that this
    // process holds everything a client holds.
    let endpoints =
        crate::memory_region::create(ntp_service::EXCHANGE_PAGES).expect("no endpoint region");
    let mut held = Holding::new();
    held.add_region(endpoints);
    let stack = crate::sched::create_rendezvous_from(endpoints).expect("no stack rendezvous");

    let faults = USER_FAULTS.load(Ordering::Relaxed);
    let report = ntp_service::start_witness(
        witness_image(),
        stack,
        clock.propose,
        entropy(),
        clock_service::CLOCK_VA,
        endpoints,
        &mut held,
    );

    let [tag, va, ..] = crate::sched::ipc_receive(report);
    assert_eq!(tag, rpt::PROBING, "the witness never reached its write");
    assert_eq!(va, clock_service::CLOCK_VA);

    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "an NTP client wrote the clock page at {va:#x} and was NOT stopped",
    );
    // The exact address, on both ISAs, from milestone 19 (run a real workload)'s portable
    // last-fault record. The kind is deliberately not asserted: the witness holds no mapping of the
    // clock page at all, so the fault is a translation fault, and the claim here is about the
    // address the client aimed at.
    assert_eq!(
        crate::arch::exceptions::last_user_fault().map(|(_, addr)| addr),
        Some(va),
        "something faulted, but not at the clock page's address",
    );
    assert_eq!(
        crate::sched::rendezvous_waiting_senders(report),
        0,
        "the witness reported past its write: the write did not fault, so an NTP client set the \
         clock by hand",
    );
    assert_eq!(
        clock.page().read(),
        before,
        "the clock page changed while an NTP client was writing at it",
    );
    held.release_or_fail("the unwritable clock witness");
    clock_held.release_or_fail("the clock service");
}

/// **The nonce is unpredictable, because it comes from the entropy service.**
///
/// Two exchanges, the server reporting what was on the wire each time: the nonces differ, and
/// neither is a time. `Query::new` would put the client's clock in the transmit field, within a
/// moment of now; a 64-bit draw lands in a random one of 2^32 seconds, so a false failure is a
/// 7201-in-2^32 event (about one boot in 600,000). `Query::with_nonce` is worth nothing if the bits
/// are guessable, as splitmix64 seeded off the virtual counter would be (notes/entropy.md).
#[test_case]
fn the_nonce_on_the_wire_is_random_and_is_not_the_clock() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    if machine_has_no_entropy() {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    }
    let (clock, clock_held) = clock();
    let claimed = clock.wall_nanos() + NANOS_PER_SEC / 2;

    // A time in the NTP era the transmit field would carry if it were a clock reading.
    let now_ntp_secs = clock.wall_nanos() / NANOS_PER_SEC + network_time_protocol::UNIX_DELTA;

    let mut nonces = [0u64; 2];
    for slot in nonces.iter_mut() {
        let (served, reported) = exchange(&clock, srv::GOOD, claimed);
        assert_eq!(served[0], rpt::SERVED);
        assert_eq!(reported[0], rpt::SYNCED, "report {reported:?}");
        *slot = served[1];
    }

    assert_ne!(
        nonces[0], nonces[1],
        "two exchanges put the same nonce on the wire: the transmit field is not random",
    );
    for (i, &nonce) in nonces.iter().enumerate() {
        let secs = nonce >> 32;
        assert!(
            secs.abs_diff(now_ntp_secs) > 3600,
            "nonce {i} decodes to {secs}, within an hour of the clock ({now_ntp_secs}): the \
             transmit field is a timestamp, not 64 random bits",
        );
    }
    clock_held.release_or_fail("the clock service");
}

/// **No entropy capability, no request.**
///
/// DECISIONS §42's no-silent-degradation rule where degrading quietly is worst. The client is
/// wired with slot 4 empty, which is what "no entropy service" actually is, and it stops before
/// it touches the network: the server's report endpoint has no waiting sender, so no request was
/// ever built. The alternative, a quiet fall back to the counter-seeded stream, would hand an
/// off-path attacker the twelve bits `Query::with_nonce` exists to take away from them, and
/// nothing in the report would say so. It is the call `SystemRng` makes when it panics.
#[test_case]
fn without_entropy_the_client_refuses_rather_than_guessing() {
    if clock_service::machine_has_no_rtc() {
        crate::testing::skip!(clock_service::NO_RTC);
    }
    let (clock, clock_held) = clock();
    let before = clock.page().read();
    let region = crate::memory_region::create(ntp_service::EXCHANGE_PAGES)
        .expect("no endpoint region for an ntp exchange");
    let mut held = Holding::new();
    held.add_region(region);
    let server = ntp_service::start_server(
        test_server_image(),
        srv::GOOD,
        clock.wall_nanos(),
        region,
        &mut held,
    );
    let report = ntp_service::start_client(
        client_image(),
        server.stack,
        clock.propose,
        None, // slot 4 empty
        SERVER_IP,
        SERVER_PORT,
        region,
        &mut held,
    );

    // Bounded, because the failure this test guards against is a client that carries on: it
    // would block in the network it should never have reached, and an unbounded `ipc_receive` here
    // would report that as a watchdog hang rather than as the refusal that did not happen.
    assert!(
        wait_for(|| crate::sched::rendezvous_waiting_senders(report) > 0),
        "the client neither refused nor reported: with no nonce it can trust, it went to the \
         network anyway and is blocked there",
    );
    let reported = crate::sched::ipc_receive(report);
    assert_eq!(
        reported[0],
        rpt::NO_ENTROPY,
        "the client proceeded without a source of unguessable bits (report {reported:?})",
    );
    assert_eq!(
        reported[1] as i64,
        abi::Error::NoSuchSlot as i64,
        "an ungranted slot must answer NoSuchSlot (-1), 'there is nothing there', not {}",
        reported[1] as i64,
    );
    assert_eq!(
        crate::sched::rendezvous_waiting_senders(server.report),
        0,
        "the server saw a request: the client sent one without a nonce it could trust",
    );
    assert_eq!(clock.page().read(), before, "the clock moved anyway");
    held.release_or_fail("the refusing client and its server");
    clock_held.release_or_fail("the clock service");
}
