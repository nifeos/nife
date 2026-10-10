//! **A TLS crypto provider's primitives, run against published test vectors, on nife**
//! for milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets), from
//! DECISIONS §196 (nife carries TLS: `rustls` for the protocol, and a crypto provider we make work).
//!
//! §196's ruling rested on a table measured against the *stock* bare-metal targets on a stable
//! host toolchain, and said so in its own limits paragraph: every candidate provider failed, at
//! `getrandom` for want of a backend or in SIMD paths on soft-float x86_64. `script/crypto-probes`
//! re-measured it against `targets/*-unknown-nife.json` on the pinned nightly, and the answer is
//! different. The two failures were a `getrandom` major this tree's backend did not answer and a
//! set of per-crate flags, not a wall.
//!
//! **This test is the other half of that**, because a build is not a computation. The x86_64
//! target sets `+soft-float` and switches off every SSE level, so each of these crates is running
//! its portable fallback rather than the path almost every other machine takes, and nothing about
//! a green build says the fallback produces the numbers the specifications print.
//!
//! **The program here is ours, and its dependencies are not this repository's**, under DECISIONS
//! §46 (thin primitives or whole subsystems; we write everything in between). It is built by
//! `helpers/build-cryptography-exerciser.sh`, which `cargo xtask test` runs for every leg it boots
//! since milestone 855 (the TLS graph enters the gated build), so `script/test` and CI run this.
//! A boot that skips that build (`cargo xtask run`, a bench) carries no program, and this skips.
//!
//! **All three ISAs run it**, which is DECISIONS §19 (architectural parity is a tenet; the targets are aarch64, riscv64 and x86_64) rather than thoroughness, and here parity is not a formality: x86_64 is the only
//! one of the three whose build forced a different implementation, so it is the one whose answers
//! were least predictable from the other two.

use super::*;

/// The reason this test gives when nobody built the program.
const NO_CRYPTOGRAPHY_EXERCISER: &str = "no cryptography_exerciser in this archive: \
     `cargo xtask test` builds it, as does helpers/build-cryptography-exerciser.sh (milestone 442)";

/// Every line the program prints before its last, in order, one per vector.
///
/// Asserted as a **containment** check rather than against the whole transcript, unlike
/// `std_tests::EXPECTED`, and the difference is deliberate. One line the program prints carries
/// the provider's cipher-suite and key-exchange counts, which move when the provider's version
/// moves; pinning them would turn an upstream release into a failure here that says nothing. The
/// vectors themselves cannot drift, because a specification does not have versions in that sense,
/// so those are pinned exactly.
///
/// **`poly1305 ok` was missing from this list for a day, for a reason worth keeping.** It was read
/// as an unexplained silent abort and withdrawn; it was a wrong call (`update_padded` instead of
/// `compute_unpadded`) whose assertion message nobody could see, because a program that aborts
/// never sends the sink's end-of-stream marker and `drain_sink` blocks waiting for it. The
/// exerciser now installs a panic hook that prints and exits cleanly, which is what made the
/// failure legible in one run. See that program's module docs.
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
/// The `entropy 0.2 ok` line is the one with no published vector and the one that is new at this
/// milestone. `rand_core` 0.6 reaches `getrandom` **0.2**, whose custom-backend hook is a
/// different symbol with a different signature from the 0.3/0.4 one, and `entropy_backend`
/// answered only the latter until milestone 442. That single `compile_error!` was the whole reason
/// no pure-Rust provider built on any nife target. A linker resolving the symbol proves nothing
/// about where bytes come from, so the program draws twice and asserts the draws differ, which is
/// the claim milestone 56 (secrets, credentials, and the entropy to make them safe) made for `std::random` and is what a silently-stubbed backend would
/// fail.
///
/// The `provider ok` line is the object §196's ruling is actually about, and since 2026-09-20 it
/// is **this tree's own provider** rather than a dependency: calef refused `rustls-rustcrypto`
/// ("doesn't seem like a high quality dependency"), so `cryptography_provider` assembles one over
/// primitives chosen deliberately. The program asserts what it offers rather than only that it
/// exists: three TLS 1.3 suites, two key exchange groups with X25519 first, three signature
/// algorithms, and a key provider that refuses. **No handshake happens**, and the milestone's
/// block is honest that a client speaking TLS 1.3 to a peer is a separate piece of work.
#[test_case]
fn a_tls_crypto_provider_computes_what_the_specifications_say() {
    if program("cryptography_exerciser").is_none() {
        crate::testing::skip!(NO_CRYPTOGRAPHY_EXERCISER);
    }
    use core::sync::atomic::Ordering;

    use crate::arch::exceptions::USER_FAULTS;

    let image =
        program("cryptography_exerciser").expect("no cryptography_exerciser in the initrd archive");
    let clock = program("clock").expect("no clock program in the initrd archive");
    let entropy = program("entropy").expect("no entropy program in the initrd archive");
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
