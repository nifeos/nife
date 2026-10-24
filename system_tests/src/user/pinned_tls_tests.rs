//! **A TLS 1.3 handshake on nife, against a peer that is not ours, trusting one pinned root**, for
//! milestone 501 (a TLS client that speaks to one pinned peer).
//!
//! Milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets) proved the
//! provider's primitives with no handshake; this is the handshake. `pinned_tls_exerciser` runs
//! `pinned_tls_client` through `std::net` and `net_stack` to `helpers/tls-peer`, OpenSSL through
//! Python's `ssl`, which slirp runs per connection at 10.0.2.9:8443. Every runner attaches the
//! `e1000e` (the family of xenon's card), so parity is one code path; virtio would have left x86_64
//! a second. It skips only where the program was not built, as `cryptography_tests` does, and
//! `cargo xtask test` builds it per leg (milestone 855 (the TLS graph enters the gated build)).

use super::*;

/// The reason this test gives when nobody built the program.
const NO_PINNED_TLS_EXERCISER: &str = "no pinned_tls_exerciser in this archive: \
     `cargo xtask test` builds it, as does helpers/build-pinned-tls-exerciser.sh (milestone 501)";

/// Every line the program prints that a pass requires, in order of printing. The `cost` lines are
/// left out: they are a measurement and change with the host, so they are printed to the log and
/// asserted nowhere.
const REQUIRED: &[&str] = &[
    "pinned_tls_exerciser start",
    "handshake ok ",
    "index ok 59 bytes",
    "bulk ok 262144 bytes",
    // The two refusals. A client that refused everything prints these and not the three above; a
    // client that accepted everything prints the three above and panics here. Only the pin prints
    // all of them.
    "refused stranger.test: unknown issuer",
    "refused elsewhere.test: not valid for name",
    "pinned_tls_exerciser done",
];

/// **The pinned peer answers over TLS 1.3, and a chain from any other root, or for any other
/// name, is refused**, here, through this tree's provider and `net_stack`.
///
/// Falsification: unfalsified in the guest. On the host, `pinned_tls_client`'s
/// `a_chain_from_another_root_is_refused` has a control beside it showing the stranger chain is
/// well formed (pinning its own root admits it), so the refusal this test asserts is the pin and
/// not a broken fixture.
#[test_case]
fn a_tls_client_speaks_to_its_pinned_peer_and_refuses_every_other() {
    let Some(image) = program("pinned_tls_exerciser") else {
        crate::testing::skip!(NO_PINNED_TLS_EXERCISER);
    };
    use core::sync::atomic::Ordering;

    use crate::arch::exceptions::USER_FAULTS;

    let net_stack = program("net_stack").expect("no net_stack program in the initial ramdisk");
    let clock = program("clock").expect("no clock program in the initial ramdisk");
    let entropy = program("entropy").expect("no entropy program in the initial ramdisk");

    let w = wired_or_skip!(e1000e_service::start_net_server(
        net_stack,
        socket_contract::NO_LISTEN_GRANT
    ));
    // The lease first, so `net_stack` is in its serve loop before the program's first connect.
    let lease = crate::sched::ipc_receive(w.report)[0] as u32;
    assert_eq!(
        lease & 0xffff_ff00,
        0x0A00_0200,
        "no slirp lease over the e1000e NIC: {lease:#010x}"
    );

    let faults_before = USER_FAULTS.load(Ordering::Relaxed);
    // Reclaimable, for `start_reclaimable`'s reason: a program present only when somebody ran a
    // build script must not leave a charge on the suite's frame ledger for that person alone.
    let spawned = std_service::start_networked(image, clock, entropy, w.stack);
    let run = &spawned.run;

    let mut got = [0u8; 2048];
    let len = super::std_tests::drain_sink(run.report, &mut got, "pinned_tls_exerciser");
    let text = core::str::from_utf8(&got[..len]).unwrap_or("<not utf-8>");
    crate::println!("    pinned_tls_exerciser printed {len} bytes:\n{text}");

    let mut from = 0;
    for line in REQUIRED {
        match text[from..].find(line) {
            Some(at) => from += at + line.len(),
            None => panic!(
                "pinned_tls_exerciser never printed `{line}` (after what came before it): a \
                 handshake, a body or a refusal went wrong on this architecture"
            ),
        }
    }

    // The exit and the fault count, on `cryptography_tests`' reasoning: every failure above is an
    // `assert!` inside the program, so a panic ends it with a missing line **and** a fault, and
    // checking both separates "the TLS was wrong" from "the program died on the way".
    assert!(
        super::wait_for(|| !crate::sched::is_thread_present(run.thread)),
        "pinned_tls_exerciser never left: it is neither exited nor faulted",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults_before,
        "pinned_tls_exerciser trapped instead of exiting",
    );

    let _ = crate::sched::reclaim_region(spawned.frames);
    run.give_back("pinned_tls_exerciser");
    w.held
        .release_or_fail("net_stack over the e1000e NIC, for the pinned TLS client");
}
