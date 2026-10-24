use credential_service as cs;

use super::*;

/// Wire the whole system once: the entropy service, the credential service, and the provisioner
/// that fills and seals the store. Returns the wiring, the provisioner's report and the service's
/// readiness report, or `None` when the mmio virtio-rng device is not attached (correct on a bare
/// board boot, milestone 145). `None` is memoized too, so the failed probe runs once per boot.
///
/// Once per boot, because the seal is irreversible: a second provisioner would `CALL` an endpoint
/// whose receiver is gone and block forever. `login_tests` and `login_user_schedule_tests` share
/// this sealed store, and the latch makes every caller safe in any order. The readiness message is
/// drained here, before any client `CALL`: the service sends it between the seal and its serve
/// loop, and an undrained `SEND` deadlocks the boot (notes/credentials/what-is-proven.md).
pub(super) fn provisioned() -> Option<(cs::Wiring, [u64; 3], [u64; 3])> {
    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    // Plain atomics rather than a lock or a `Once`: the only writer is the test thread, and
    // the tree's `spin` is built without the `once` feature. Release/Acquire on `DONE` is what
    // publishes the nine words next to it (rule 4: assume weak ordering).
    static DONE: AtomicBool = AtomicBool::new(false);
    static OK: AtomicBool = AtomicBool::new(false);
    #[allow(clippy::declare_interior_mutable_const)]
    const ZERO: AtomicU64 = AtomicU64::new(0);
    static SAVED: [AtomicU64; 11] = [ZERO; 11];

    if !DONE.load(Ordering::Acquire) {
        let rng = program("entropy").expect("no entropy program in the initial ramdisk");
        let Some(e) = entropy_service::ensure(rng, entropy_service::Bus::Mmio) else {
            DONE.store(true, Ordering::Release);
            return None;
        };
        if let Some(r) = e.wait_for_ready() {
            assert_eq!(
                r[0],
                entropy_contract::READY,
                "the entropy service did not come up, so no salt could be drawn",
            );
        }
        let svc = program("credentialer").expect("no credentialer program in the initial ramdisk");
        let cli = program("credentialer_test_client")
            .expect("no credentialer_test_client program in the initial ramdisk");
        let w = cs::start(svc, e.request);
        let report = cs::provisioner(cli, &w);
        let ready = crate::sched::ipc_receive(w.ready);
        for (slot, v) in SAVED.iter().zip([
            w.ready,
            w.verify,
            w.provision,
            report[0],
            report[1],
            report[2],
            ready[0],
            ready[1],
            ready[2],
            w.verify_page_frame,
            w.provision_page_frame,
        ]) {
            slot.store(v, Ordering::Relaxed);
        }
        OK.store(true, Ordering::Release);
        DONE.store(true, Ordering::Release);
    }
    if !OK.load(Ordering::Acquire) {
        return None;
    }
    let v = |i: usize| SAVED[i].load(Ordering::Relaxed);
    Some((
        cs::Wiring {
            ready: v(0),
            verify: v(1),
            provision: v(2),
            verify_page_frame: v(9),
            provision_page_frame: v(10),
        },
        [v(3), v(4), v(5)],
        [v(6), v(7), v(8)],
    ))
}

/// **Phase one lands, and the store's capacity is real.** Three identities go in, the fourth is
/// refused with `FULL` rather than quietly replacing somebody, and the seal is accepted.
///
/// The service's own readiness message is checked here too, because it is sent *after* the
/// seal: receiving it is the kernel's evidence that the provisioning loop was left, not merely
/// that a `SEAL` was answered.
#[test_case]
fn provisioning_fills_the_store_and_the_seal_closes_it() {
    let Some((_, report, ready)) = provisioned() else {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    };
    assert_eq!(report[0], cs::RPT_DONE, "the provisioner did not report");
    let codes = report[1];
    // Three logins, then three shares. Six secrets, not three people with two credentials each:
    // a secret here is scoped to the resource it authenticates to (milestone 65).
    for k in 0..3 {
        assert_eq!(
            cs::nth(codes, k),
            credential_contract::OK,
            "identity {k} was not stored (reply {}), codes {codes:#018x}",
            cs::nth(codes, k),
        );
    }
    for k in 3..6 {
        assert_eq!(
            cs::nth(codes, k),
            credential_contract::OK,
            "share {} was not stored (reply {}), codes {codes:#018x}",
            k - 3,
            cs::nth(codes, k),
        );
    }
    assert_eq!(
        cs::nth(codes, 6),
        credential_contract::FULL,
        "a seventh secret in a six-slot store must be refused, not silently accepted",
    );
    assert_eq!(
        cs::nth(codes, 7),
        credential_contract::OK,
        "the seal was not accepted"
    );
    assert_eq!(
        report[2] & cs::F_CLEAN,
        cs::F_CLEAN,
        "the provisioner's page still held bytes after the seal: a plaintext secret survived \
         in a frame the provisioner maps",
    );

    assert_eq!(
        ready[0],
        cs::RPT_READY,
        "the credential service did not reach phase two (it reported {:#x}; a 0xDEAD_.. word's \
         low byte names the step, see components/src/credentialer.rs)",
        ready[0],
    );
    assert_eq!(
        ready[1], 6,
        "the sealed store does not hold three logins and three shares"
    );
    assert!(
        ready[2] >= 1024,
        "the wired Argon2id cost is {} KiB, which is not memory-hard in any useful sense",
        ready[2],
    );
}

/// **The headline.** A userspace client holding one endpoint, no store, no entropy and no
/// budget gets the right answer to four questions: the right secret, the wrong secret, an
/// identity nobody provisioned, and one person's password against another person's account.
///
/// The last two are one assertion in spirit and two in fact, because they are the failures that
/// look alike from outside and come from different bugs: a lookup that ignores the identity,
/// and a comparison that ignores the pairing.
#[test_case]
fn a_client_gets_a_correct_yes_or_no_and_nothing_else() {
    let Some((w, _, _)) = provisioned() else {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    };
    let cli = program("credentialer_test_client")
        .expect("no credentialer_test_client program in the initial ramdisk");
    let r = cs::client(cli, &w, cs::ROLE_HONEST);
    assert_eq!(r[0], cs::RPT_DONE, "the client did not report");
    let codes = r[1];
    assert_eq!(
        cs::nth(codes, 0),
        credential_contract::MATCH,
        "the right secret for a provisioned identity was refused, codes {codes:#018x}",
    );
    assert_eq!(
        cs::nth(codes, 1),
        credential_contract::MISMATCH,
        "the wrong secret was accepted, codes {codes:#018x}",
    );
    assert_eq!(
        cs::nth(codes, 2),
        credential_contract::MISMATCH,
        "an identity nobody provisioned was accepted, codes {codes:#018x}",
    );
    assert_eq!(
        cs::nth(codes, 3),
        credential_contract::MISMATCH,
        "one identity's secret opened another's account, codes {codes:#018x}",
    );
    assert_eq!(
        r[2] & cs::F_CLEAN,
        cs::F_CLEAN,
        "the shared page was not empty after the last reply: either the client's presented \
         secret or something of the store's is still sitting in a frame two processes map",
    );
}

/// **The same endowment, used to attack.** The attacker holds exactly what the honest client
/// holds, and tries to write the store through it: `PUT`, `SEAL`, an undefined opcode, and a
/// request whose lengths are outside the contract. Every one is refused, and the credential it
/// tried to install does not work.
///
/// It is refused because there is nothing to refuse: the provision endpoint was deleted at both
/// ends before this program was spawned, so `PUT` here is not a privileged request arriving at
/// a guard, it is a word arriving at a loop that implements one opcode.
#[test_case]
fn the_same_endowment_cannot_write_the_store() {
    let Some((w, _, _)) = provisioned() else {
        crate::testing::skip!("no virtio-rng device on the mmio bus (NIFE_RNG not set?)");
    };
    let cli = program("credentialer_test_client")
        .expect("no credentialer_test_client program in the initial ramdisk");
    let r = cs::client(cli, &w, cs::ROLE_ATTACKER);
    assert_eq!(r[0], cs::RPT_DONE, "the attacker did not report");
    let codes = r[1];

    // `PUT` gets MISMATCH, not MALFORMED: `provision::PUT` and `verify::VERIFY` are both opcode 1,
    // and the endpoint gives a number its meaning, so the attacker sent a verify of an identity
    // nobody provisioned. The opcodes stay unrenumbered: renumbering would imply the service tells
    // a forbidden opcode from an unknown one, and it does not.
    assert_eq!(
        cs::nth(codes, 0),
        credential_contract::MISMATCH,
        "a PUT on the verify endpoint is a verify of an identity nobody provisioned, so the \
         answer must be MISMATCH; codes {codes:#018x}",
    );
    // `SEAL` is opcode 2, which the verify endpoint does not serve, so MALFORMED. Its answer moves
    // whenever the verify space grows or shrinks, and nothing breaks, because a word arriving at a
    // serve loop never carried authority (notes/credentials/what-is-proven.md).
    assert_eq!(
        cs::nth(codes, 1),
        credential_contract::MALFORMED,
        "a SEAL on the verify endpoint is an opcode this endpoint does not serve, so the answer \
         must be MALFORMED; codes {codes:#018x}",
    );
    for (k, what) in [
        (2, "an undefined opcode"),
        (3, "a request with lengths outside the contract"),
        (4, "an opcode from the provisioning space"),
    ] {
        assert_eq!(
            cs::nth(codes, k),
            credential_contract::MALFORMED,
            "{what} on the verify endpoint was answered {} rather than MALFORMED, codes \
             {codes:#018x}",
            cs::nth(codes, k),
        );
    }
    assert_eq!(
        cs::nth(codes, 5),
        credential_contract::MISMATCH,
        "the attacker installed a working credential for itself, codes {codes:#018x}",
    );
    assert_eq!(
        r[2] & cs::F_CLEAN,
        cs::F_CLEAN,
        "the attacker left bytes in the shared page",
    );
}
