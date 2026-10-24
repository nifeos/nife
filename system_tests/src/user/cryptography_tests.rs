//! **A TLS crypto provider's primitives, run against published test vectors, on nife**
//! for milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets), from
//! DECISIONS §196 (nife carries TLS: `rustls` for the protocol, and a crypto provider we make work).
//!
//! A green build is not a computation: the x86_64 target sets `+soft-float` with every SSE level
//! off, so each crate runs its portable fallback, and only the vectors show that the fallback
//! computes what the specifications print. All three ISAs run it, DECISIONS §19 (architectural parity is a tenet; the targets are aarch64, riscv64 and x86_64).
//! `helpers/build-cryptography-exerciser.sh` builds the program, and `cargo xtask test` runs it for
//! every leg it boots; a boot that skips that build (`cargo xtask run`, a bench) carries no program,
//! and this skips. See notes/cryptography-provider/the-exerciser-test.md.

use super::*;

/// The reason this test gives when nobody built the program.
const NO_CRYPTOGRAPHY_EXERCISER: &str = "no cryptography_exerciser in this archive: \
     `cargo xtask test` builds it, as does helpers/build-cryptography-exerciser.sh (milestone 442)";

/// Every line the program prints before its last, in order, one per vector.
///
/// A containment check rather than a whole-transcript match, unlike `std_tests::EXPECTED`: one line
/// carries the provider's cipher-suite and key-exchange counts, which move with the provider's
/// version, while a specification's vector cannot drift, so the vectors are pinned exactly. A
/// failing vector is legible because the exerciser's panic hook prints and exits cleanly; a program
/// that aborts never sends the sink's end-of-stream marker, and `drain_sink` blocks waiting for it.
/// See that program's module docs.
const VECTORS: &[&str] = &[
    "sha256 ok",
    "sha256 empty ok",
    "sha384 ok",
    "hmac-sha256 ok",
    "hkdf-sha256 ok",
    "aes-128-gcm ok",
    "poly1305 ok",
    "chacha20-poly1305 ok",
    "x25519 ok",
    "p256 generator ok",
    "entropy 0.2 ok",
    // **The two real chains, which are why `rsa` is in the graph at all** (calef, 2026-09-20:
    // "Take rsa"). `github.com` is ECDSA P-256 throughout and would have needed none of it; the
    // hosts that actually serve a release asset are RSA 2048 and RSA 4096, so a client without
    // these can reach the site and not the file.
    "chain objects.githubusercontent.com ok",
    "chain ghcr.io ok",
    // A verifier that returns `Ok` unconditionally would pass every line above. This one does not:
    // a single flipped signature bit, and a certificate presented for another host's name, are
    // both refused.
    "chain refusals ok",
];

/// **A `rustls` crypto provider's algorithms compute what their specifications say, here.**
///
/// `entropy 0.2 ok` has no published vector: the program draws twice through `getrandom` 0.2 and
/// asserts the draws differ, the claim milestone 56 (secrets, credentials, and the entropy to make them safe) made for `std::random`, which a
/// silently stubbed backend fails. `provider ok` is the object §196's ruling is about, this tree's
/// own `cryptography_provider`: three TLS 1.3 suites, two key exchange groups with X25519 first,
/// three signature algorithms, and a key provider that refuses. No handshake happens. See
/// notes/cryptography-provider/the-exerciser-test.md.
#[test_case]
fn a_tls_crypto_provider_computes_what_the_specifications_say() {
    if program("cryptography_exerciser").is_none() {
        crate::testing::skip!(NO_CRYPTOGRAPHY_EXERCISER);
    }
    use core::sync::atomic::Ordering;

    use crate::arch::exceptions::USER_FAULTS;

    let image = program("cryptography_exerciser")
        .expect("no cryptography_exerciser in the initial ramdisk");
    let clock = program("clock").expect("no clock program in the initial ramdisk");
    let entropy = program("entropy").expect("no entropy program in the initial ramdisk");
    let faults_before = USER_FAULTS.load(Ordering::Relaxed);
    // `start_reclaimable` rather than `start`, because this program is in the archive only when
    // somebody ran the build script: a permanent charge for its heap would make the suite's frame
    // ledger fail for exactly that person and pass for everyone else. Milestone 121 (`ripgrep` on nife: enumeration as a capability, and what the walk costs) made the same call for `ripgrep` and `user::holding` carries the reasoning.
    let run = std_service::start_reclaimable(image, clock, entropy);
    let (report, tid) = (run.report, run.thread);

    let mut got = [0u8; 2048];
    let len = super::std_tests::drain_sink(report, &mut got, "cryptography_exerciser");
    let text = core::str::from_utf8(&got[..len]).unwrap_or("<not utf-8>");
    crate::println!("    cryptography_exerciser printed {len} bytes:\n{text}");

    for line in VECTORS {
        assert!(
            text.contains(line),
            "cryptography_exerciser never printed `{line}`: either a vector failed on this \
             architecture, or the program stopped before reaching it",
        );
    }
    assert!(
        text.contains("provider ok "),
        "the provider was never constructed, or offered no TLS 1.3 suite",
    );

    // The exit, on `std_tests`' reasoning, which milestone 64 (enough `std` to run somebody else's crate) learned the hard way: a program that
    // printed a perfect transcript and then trapped looks identical from here without this. It
    // matters more than usual for this one, because every failure above is an `assert!` inside the
    // program, and a panic would end it with a missing line **and** a fault. Checking both
    // separates "a vector is wrong" from "the program died on the way".
    assert!(
        super::wait_for(|| !crate::sched::is_thread_present(tid)),
        "cryptography_exerciser never left: it is neither exited nor faulted",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults_before,
        "cryptography_exerciser trapped instead of exiting: a vector failed, or the provider \
         panicked",
    );

    // **Give the heap back** (`user::holding`'s reasoning, and `ripgrep_tests`' closing line). The
    // thread is already gone by the assertion above, so one call is enough.
    run.give_back("cryptography_exerciser");
}
