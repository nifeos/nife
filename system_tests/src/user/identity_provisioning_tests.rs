use credential_service as cs;
use identity_provisioner_service as ips;

use super::*;

/// The identity this suite provisions: the roster's first person, so `credentialer_test_client`'s
/// `ROLE_HONEST` can be reused unmodified as the verification half. Its first check (the right
/// password for that person) becomes a real proof that what this tool `PUT` is what a client can
/// later verify, and its other three checks (a wrong password, an unknown identity, another
/// identity's password against this one) are unaffected by which store they run against.
///
/// Taken from `credential_contract::fixture` rather than matched to it by hand (milestone 293 (a
/// role that differs only in its credentials is an argument)), so the two cannot drift apart.
const IDENTITY: &[u8] =
    credential_contract::fixture::PEOPLE[credential_contract::fixture::ALICE as usize].0;
/// The same identity as a `&str`, because `fs_service::narrow_dir` names a path component rather
/// than a credential. Derived rather than re-typed (milestone 293), so the two spellings cannot
/// disagree at all.
const IDENTITY_STR: &str = match core::str::from_utf8(IDENTITY) {
    Ok(s) => s,
    Err(_) => panic!("the roster's identity is not UTF-8, so it cannot name a directory"),
};
const SECRET: &[u8] =
    credential_contract::fixture::PEOPLE[credential_contract::fixture::ALICE as usize].1;

/// A second `PUT` for the same identity, with a different secret, used only to provoke the refusal
/// this suite's second test asks for. Never expected to be stored. Another person's, which is what
/// makes it certainly not this one's.
const DUPLICATE_SECRET: &[u8] =
    credential_contract::fixture::PEOPLE[credential_contract::fixture::BOB as usize].1;

/// What [`wired`] hands each test: the sealed store's own wiring (for a real `VERIFY`) and both
/// provisioning attempts' raw reports. The file service's root that both attempts were run against
/// is not carried past setup: every test reaches the filesystem through `fs_service`'s own memoized
/// globals instead (`fs_service::narrow_dir` re-derives the same root `ensure` already wired).
struct Wired {
    cs: cs::Wiring,
    /// The first attempt's report: a fresh identity, expected [`ips::RPT_OK`].
    fresh: [u64; 2],
    /// The second attempt's report: the same identity again, expected [`ips::RPT_CRED_FAILED`].
    duplicate: [u64; 2],
}

/// The name `redoxfs_server`'s image travels under; present only when `test` built it.
fn redoxfs_server_image() -> &'static [u8] {
    program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk")
}

/// Wire fresh entropy, an unsealed credential service and the file service, then run
/// `identity_provisioner` twice before sealing: a new identity, then the same one again, for the
/// tool's report on a genuine duplicate. Memoized with the wiring, once per boot.
///
/// Its own store, not `credential_tests::provisioned()`'s, which is sealed (irreversibly) and full
/// by the time it returns. Both attempts use `cs::Wiring::provision_page_frame`, this instance's
/// own; `verify_page_frame` is carried past setup in [`Wired`]'s `cs` field, because
/// `credential_service::client` reads it directly.
fn wired() -> Option<Wired> {
    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    static OK: AtomicBool = AtomicBool::new(false);
    #[allow(clippy::declare_interior_mutable_const)]
    const ZERO: AtomicU64 = AtomicU64::new(0);
    static SAVED: [AtomicU64; 8] = [ZERO; 8];

    if !DONE.load(Ordering::Acquire) {
        let result = (|| {
            let rng = program("entropy").expect("no entropy program in the initial ramdisk");
            let e = entropy_service::ensure(rng, entropy_service::Bus::Mmio)?;
            if let Some(r) = e.wait_for_ready() {
                assert_eq!(
                    r[0],
                    entropy_contract::READY,
                    "the entropy service did not come up, so no salt could be drawn",
                );
            }
            let svc =
                program("credentialer").expect("no credentialer program in the initial ramdisk");
            let w = cs::start(svc, e.request);
            let (fs_ep, fs_page_frame) =
                fs_service::root_directory(fs_service::blk_server_image(), redoxfs_server_image())?;

            let prov_image = program("identity_provisioner")
                .expect("no identity_provisioner program in the initial ramdisk");

            // Attempt one: a fresh identity. Expected `RPT_OK`.
            let r_a = ips::provision(
                prov_image,
                w.provision,
                w.provision_page_frame,
                fs_ep,
                fs_page_frame,
                IDENTITY,
                SECRET,
            );
            // Attempt two: the same identity, unsealed. Its `MKDIR` finds attempt one's subtree
            // (`EEXIST`, tolerated), isolating the credential half's duplicate refusal.
            let r_b = ips::provision(
                prov_image,
                w.provision,
                w.provision_page_frame,
                fs_ep,
                fs_page_frame,
                IDENTITY,
                DUPLICATE_SECRET,
            );

            // Seal, as an operator would: the provisioner role fills `bob`, `carol` and three
            // shares (its `PUT` for `alice` is refused, not asserted) and seals the six-slot store
            // the verification below depends on.
            let cli = program("credentialer_test_client")
                .expect("no credentialer_test_client program in the initial ramdisk");
            cs::provisioner(cli, &w);
            let ready = crate::sched::ipc_receive(w.ready);
            assert_eq!(
                ready[0],
                cs::RPT_READY,
                "the credential service did not reach phase two after the seal",
            );

            Some((w, r_a, r_b))
        })();

        if let Some((w, r_a, r_b)) = &result {
            for (slot, v) in SAVED.iter().zip([
                w.ready,
                w.verify,
                w.provision,
                r_a[0],
                r_a[1],
                r_b[0],
                r_b[1],
                w.verify_page_frame,
            ]) {
                slot.store(v, Ordering::Relaxed);
            }
            OK.store(true, Ordering::Release);
        }
        DONE.store(true, Ordering::Release);
    }
    if !OK.load(Ordering::Acquire) {
        return None;
    }
    let v = |i: usize| SAVED[i].load(Ordering::Relaxed);
    Some(Wired {
        cs: cs::Wiring {
            ready: v(0),
            verify: v(1),
            provision: v(2),
            // `credential_service::client` reads this directly, so it must not be 0.
            // `provision_page_frame` is dead past the seal, so it is not carried.
            verify_page_frame: v(7),
            provision_page_frame: 0,
        },
        fresh: [v(3), v(4)],
        duplicate: [v(5), v(6)],
    })
}

/// A fresh identity gets a working credential and a real, descendable subtree from one tool
/// invocation. Neither proof is the tool's own report: a real `VERIFY` against what was `PUT`
/// (`credentialer_test_client`'s `ROLE_HONEST`), and a subtree caretaker whose descent succeeds
/// (`fs_service::narrow_dir` asserts on a refused descent, so a missing subtree fails loudly).
#[test_case]
fn provisioning_creates_a_working_credential_and_a_real_subtree() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    assert_eq!(
        w.fresh,
        [ips::RPT_OK, 0],
        "provisioning a fresh identity did not report success",
    );

    let cli = program("credentialer_test_client")
        .expect("no credentialer_test_client program in the initial ramdisk");
    let honest = cs::client(cli, &w.cs, cs::ROLE_HONEST);
    assert_eq!(honest[0], cs::RPT_DONE, "the honest client did not report");
    assert_eq!(
        cs::nth(honest[1], 0),
        credential_contract::MATCH,
        "the identity this tool PUT did not verify with the secret it was given",
    );

    let caretaker = program("fs_subtree_caretaker")
        .expect("no fs_subtree_caretaker program in the initial ramdisk");
    // `narrow_dir` asserts the descent (`wait_for_caretaker`), so reaching here proves the subtree.
    let (_, _, _, held) = fs_service::narrow_dir_held(
        fs_service::blk_server_image(),
        redoxfs_server_image(),
        caretaker,
        IDENTITY_STR,
        filesystem_contract::dir::ALL,
    )
    .expect("the subtree identity_provisioner created did not open");
    held.release_or_fail("the caretaker over the provisioned subtree");
}

/// A genuine duplicate is refused, and nothing already there is disturbed. The second attempt
/// reused `alice`: its `MKDIR` found the subtree (`EEXIST`, tolerated) and its `PUT` was refused
/// (`credentialer::Store::put`: "a duplicate identity is refused"). Asserted: both halves, and that
/// attempt one's credential still verifies, the property `identity_provisioner.rs`'s ordering
/// argument exists for.
#[test_case]
fn a_duplicate_identity_is_refused_without_disturbing_the_original() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    assert_eq!(
        w.duplicate,
        [ips::RPT_CRED_FAILED, credential_contract::MALFORMED],
        "a genuine duplicate PUT was not refused the way credentialer::Store::put's own rule says it must \
         be (a duplicate identity answers MALFORMED, the same code a malformed request gets, \
         because neither is an authentication outcome)",
    );

    let cli = program("credentialer_test_client")
        .expect("no credentialer_test_client program in the initial ramdisk");
    let honest = cs::client(cli, &w.cs, cs::ROLE_HONEST);
    assert_eq!(honest[0], cs::RPT_DONE, "the honest client did not report");
    assert_eq!(
        cs::nth(honest[1], 0),
        credential_contract::MATCH,
        "the refused second PUT disturbed the identity the first, successful PUT already stored",
    );
}
