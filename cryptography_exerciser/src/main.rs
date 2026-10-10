//! **Published test vectors, run on nife, through the crates a `rustls` crypto provider is made
//! of**, for milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets).
//!
//! DECISIONS §196 (nife carries TLS: `rustls` for the protocol, and a crypto provider we make
//! work) ruled that the provider is work rather than a purchase, and milestone 442's block asks
//! first for a measurement and then for one provider that builds and passes its own test vectors
//! on all three architectures. `script/crypto-probes` answers the build half. This answers the
//! other half, and it answers it the only way that counts: by running.
//!
//! # Why a building crypto crate is not an answer
//!
//! `script/crypto-probes`' own `BUGS` section says it: it measures compile and link and nothing
//! else. That caveat is worse here than it was for the fifty crates in
//! notes/crates-io-on-nife.md, for two reasons that are specific to this milestone.
//!
//! **Every one of these crates is being used on a code path almost nobody uses.**
//! `x86_64-unknown-nife` sets `+soft-float` and switches off every SSE level, so the SIMD
//! implementations of `sha2`, `polyval`, `poly1305` and `curve25519-dalek` do not merely go
//! unused, they do not compile, and the build only succeeds once each crate's portable fallback
//! is forced on. A portable fallback is ordinary tested code upstream; what is not ordinary is
//! this compiler, this target specification and this `std`. A vector is what distinguishes
//! "it built" from "it computes the right number here".
//!
//! **And the randomness is genuinely new.** `entropy_backend` learned `getrandom` 0.2's hook at
//! this milestone, and a linker resolving `__getrandom_custom` says nothing about whether bytes
//! arrive from the entropy service. The `rand_core` draw below is the only thing in this tree
//! that exercises that path end to end.
//!
//! # Where the vectors come from, and why that matters
//!
//! Every expected value here is **transcribed from the specification that publishes it**, named
//! in the comment beside it, and not from running the code. That is the whole point:
//! `crates/measured_boot` makes the same distinction in so many words about its own SHA-256
//! ("the published FIPS 180-4 vectors, **not** self-consistency checks"), and a vector taken from
//! the implementation under test proves only that it is consistent with itself.
//!
//! The rule that goes with it, for whoever maintains this: **if a vector fails, do not adjust the
//! vector.** Either the transcription is wrong, in which case check it against the specification
//! and not against the output, or the implementation is wrong on this target, which is exactly
//! what this program exists to find out.
//!
//! # BUGS
//!
//! - **This does not speak TLS.** It builds this tree's own provider and asks what it can
//!   negotiate; no
//!   handshake happens, no certificate is verified, and no peer exists. Milestone 442's clause 3
//!   (a client that speaks TLS 1.3 to one peer, holding that peer's root as a capability) is not
//!   attempted here and there is no network in this program at all.
//! - **The coverage is the algorithms a TLS 1.3 handshake uses, not everything the provider
//!   offers.** There is no ECDSA or RSA signature verification vector, which means certificate
//!   verification, the largest remaining piece, is unexercised. `rustls-webpki` is not called.
//! - **A crate that picks its implementation by asking the CPU is a hazard on
//!   `x86_64-unknown-nife`, and this one cost most of a day.** The other flags in
//!   `.cargo/config.toml` exist because LLVM cannot compile a SIMD path for this target at all,
//!   which fails loudly at build time. `chacha20` compiles fine and then **executes an AVX2
//!   instruction in ring 3**, where this target has no SSE or AVX state (its own description says
//!   "softfloat ring 3, no SSE state"), and the program dies with `vector 6 (invalid opcode)`
//!   before printing a byte. `--cfg chacha20_force_soft` is the fix here; the general case is that
//!   run-time feature detection and this target disagree, and nothing warns.
//! - **A vector proves the answer, not the manner.** Nothing here measures timing, so a portable
//!   fallback that is correct and not constant-time would pass every line below. On
//!   `x86_64-unknown-nife` the fallbacks are exactly what is being run.
//! - **`cargo xtask test` builds it for every leg it boots**, through
//!   `helpers/build-cryptography-exerciser.sh`, since milestone 855 (the TLS graph enters the gated
//!   build). §196 (nife carries TLS) and §198 (the glue is ours, the primitives are not) ruled the
//!   dependencies that DECISIONS §46 (thin primitives or whole subsystems) had made an architect's.
//!   The kernel test skips only in a boot that did not build it.

// `entropy_backend` defines `getrandom`'s two custom-backend symbols and nothing references them
// from Rust, so an rlib nobody names is not linked. Same shape as a panic handler. See that
// crate's docs for the hour this cost the first time.
use entropy_backend as _;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use hmac::Mac;
use rand_core::RngCore;
use sha2::Digest;

/// Decode a hex literal written out of a specification, at run time, so the vectors below can be
/// read as the spec prints them rather than as Rust byte arrays.
///
/// It panics on a malformed literal, which is correct for this program: a typo in a vector must
/// stop the transcript rather than quietly compare against something else.
fn hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd-length hex literal: {s}");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("bad hex digit"))
        .collect()
}

/// Report one vector. The transcript is what the kernel test asserts on, so every line is
/// `<name> ok` and a mismatch panics instead of printing a different line: a program that printed
/// `sha256 bad` and exited would need the test to know every possible failure word.
fn check(name: &str, got: &[u8], want_hex: &str) {
    let want = hex(want_hex);
    assert_eq!(
        got,
        &want[..],
        "{name}: this target computed something the specification does not say"
    );
    println!("{name} ok");
}


/// **When the pinned chains were captured**, as seconds since the Unix epoch: 2026-09-20 00:00 UTC.
///
/// A fixed instant rather than the clock, and that is a deliberate trade with a cost. These are
/// real certificates with real expiry dates (2026-10-31 and 2026-11-07), so verifying them against
/// "now" would turn this test red on a Tuesday for a reason that has nothing to do with this
/// system. Pinning the instant makes the assertion historical: these bytes verified at this time,
/// which is what a signature test should say. **It also means expiry is never exercised here**,
/// which is recorded in this program's `BUGS`.
const CAPTURED_AT: u64 = 1_789_516_800;

/// Walk one chain, returning webpki's own verdict.
///
/// The trust anchor is the **last certificate the host served**, not a system root store, which is
/// DECISIONS §196's clause 4 rather than a shortcut: the client holds one root for the one source
/// it talks to. Both of these chains are cross-signed, so the served top is itself signed by a
/// root that never appears on the wire, and a client with a pinned anchor neither needs nor sees
/// it.
fn chain_result(
    leaf: &[u8],
    intermediate: &[u8],
    root: &[u8],
    dns: &str,
) -> Result<(), webpki::Error> {
    let provider = cryptography_provider::provider();
    let root = pki_types::CertificateDer::from(root);
    let anchor = webpki::anchor_from_trusted_cert(&root)?;
    let leaf = pki_types::CertificateDer::from(leaf);
    let cert = webpki::EndEntityCert::try_from(&leaf)?;
    let intermediates = [pki_types::CertificateDer::from(intermediate)];
    cert.verify_for_usage(
        provider.signature_verification_algorithms.all,
        &[anchor],
        &intermediates,
        pki_types::UnixTime::since_unix_epoch(core::time::Duration::from_secs(CAPTURED_AT)),
        webpki::KeyUsage::server_auth(),
        None,
        None,
    )?;
    let name = pki_types::ServerName::try_from(dns)
        .map_err(|_| webpki::Error::UnsupportedNameType)?;
    cert.verify_is_valid_for_subject_name(&name)
}

/// The same, asserting success and printing the line the kernel test looks for.
fn verify_chain(what: &str, leaf: &[u8], intermediate: &[u8], root: &[u8], dns: &str) {
    match chain_result(leaf, intermediate, root, dns) {
        Ok(()) => println!("{what} ok"),
        Err(e) => panic!("{what}: this chain did not verify here: {e:?}"),
    }
}

fn main() {
    // **Say what went wrong, and leave through the front door.** With `panic = "abort"` a panic
    // runs this hook and then executes a trap, so the process never reaches the runtime's
    // `cleanup` and never sends the sink's end-of-stream marker. The kernel-side reader blocks on
    // that marker, so without this hook every byte the program printed, **including the panic
    // message**, is sent and never read: the transcript looks empty and the test hangs.
    //
    // Milestone 442's lane spent hours reading that as "the program dies before its first line".
    // It does not. It dies saying exactly why, into an endpoint nobody is still reading.
    std::panic::set_hook(Box::new(|info| {
        println!("PANIC {info}");
        // Exiting rather than falling through to the trap: `process::exit` runs `cleanup`, which
        // sends end-of-stream, which is what lets the reader see the line above.
        std::process::exit(101);
    }));

    // A line before any work, so a program that dies during start-up is distinguishable from one
    // that dies on its first vector. `check` asserts before it prints, deliberately, so a failed
    // vector produces silence and nothing else would tell these two apart.
    println!("cryptography_exerciser start");

    // ---------------------------------------------------------------------------------------
    // Hashing. FIPS 180-4's own examples, the same two this tree already pins for its
    // hand-written SHA-256 in `crates/measured_boot`.
    // ---------------------------------------------------------------------------------------
    check(
        "sha256",
        &sha2::Sha256::digest(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    );
    check(
        "sha256 empty",
        &sha2::Sha256::digest(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
    // SHA-384 is TLS 1.3's other hash: `TLS_AES_256_GCM_SHA384` names it.
    check(
        "sha384",
        &sha2::Sha384::digest(b"abc"),
        "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed\
         8086072ba1e7cc2358baeca134c825a7",
    );

    // ---------------------------------------------------------------------------------------
    // HMAC and HKDF, which are what TLS 1.3's key schedule is built out of. RFC 4231 test case 1
    // and RFC 5869 test case 1.
    // ---------------------------------------------------------------------------------------
    let mut mac = <hmac::Hmac<sha2::Sha256> as Mac>::new_from_slice(&[0x0b; 20])
        .expect("HMAC takes a key of any length");
    mac.update(b"Hi There");
    check(
        "hmac-sha256",
        &mac.finalize().into_bytes(),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
    );

    let hk = hkdf::Hkdf::<sha2::Sha256>::new(
        Some(&hex("000102030405060708090a0b0c")),
        &[0x0b; 22],
    );
    let mut okm = [0u8; 42];
    hk.expand(&hex("f0f1f2f3f4f5f6f7f8f9"), &mut okm)
        .expect("42 bytes is well inside HKDF's output limit");
    check(
        "hkdf-sha256",
        &okm,
        "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
         34007208d5b887185865",
    );

    // ---------------------------------------------------------------------------------------
    // The record-layer ciphers. `TLS_AES_128_GCM_SHA256` and `TLS_CHACHA20_POLY1305_SHA256` are
    // two of TLS 1.3's three mandatory-to-implement suites.
    // ---------------------------------------------------------------------------------------
    // AES-128-GCM, the GCM specification's test case 2 (all-zero key, all-zero 96-bit IV, one
    // all-zero block of plaintext, no additional data). `encrypt` returns ciphertext with the
    // 16-byte tag appended, which is why the expected value is 32 bytes rather than 16.
    let gcm = aes_gcm::Aes128Gcm::new((&[0u8; 16]).into());
    let sealed = gcm
        .encrypt(
            (&[0u8; 12]).into(),
            Payload {
                msg: &[0u8; 16],
                aad: b"",
            },
        )
        .expect("AES-GCM seals a 16-byte message");
    check(
        "aes-128-gcm",
        &sealed,
        "0388dace60b6a392f328c2b971b2fe78ab6e47d42cec13bdf53a67b21257bddf",
    );

    // Poly1305 on its own, RFC 8439 section 2.5.2. Withdrawn on 2026-09-19 as an unexplained
    // silent abort; restored on 2026-09-20 once the panic hook above made the abort speak, at
    // which point it said the vector had failed and printed both values.
    //
    // **`compute_unpadded`, not `update_padded`, and that was the whole bug.** `update_padded`
    // zero-fills the last partial block, which is what the AEAD construction does to its
    // ciphertext and additional data, and is not what section 2.5.2's standalone example does to
    // its 34-byte message. The vector was right and the crate was right; the call was wrong.
    {
        use poly1305::universal_hash::KeyInit as _;
        let key = hex(
            "85d6be7857556d337f4452fe42d506a8\
             0103808afb0db2fd4abff6af4149f51b",
        );
        let tag = poly1305::Poly1305::new(key.as_slice().into())
            .compute_unpadded(b"Cryptographic Forum Research Group");
        check("poly1305", &tag, "a8061dc1305136c6c22b8baf0c0127a9");
    }

    // ChaCha20-Poly1305, RFC 8439 section 2.8.2. This is the AEAD vector rather than the cipher
    // one, so it covers the whole construction the record layer uses.
    {
        use chacha20poly1305::aead::{Aead as _, KeyInit as _};
        let key: Vec<u8> = (0x80u8..=0x9f).collect();
        let nonce = hex("070000004041424344454647");
        let aad = hex("50515253c0c1c2c3c4c5c6c7");
        let plaintext: &[u8] = b"Ladies and Gentlemen of the class of '99: \
If I could offer you only one tip for the future, sunscreen would be it.";
        let aead = chacha20poly1305::ChaCha20Poly1305::new(key.as_slice().into());
        let sealed = aead
            .encrypt(
                nonce.as_slice().into(),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .expect("ChaCha20-Poly1305 seals a 114-byte message");
        check(
            "chacha20-poly1305",
            &sealed,
            "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6\
             3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b36\
             92ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc\
             3ff4def08e4b7a9de576d26586cec64b61161ae10b594f09e26a7e902ecbd060\
             0691",
        );
    }

    // ---------------------------------------------------------------------------------------
    // Key exchange. TLS 1.3 negotiates X25519 or one of the NIST curves, and both go through
    // large-integer arithmetic that a soft-float target has no vector unit to help with.
    // ---------------------------------------------------------------------------------------
    // RFC 7748 section 5.2, the first X25519 vector: one scalar, one u-coordinate, one answer.
    {
        let mut scalar = [0u8; 32];
        scalar.copy_from_slice(&hex(
            "a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4",
        ));
        let mut point = [0u8; 32];
        point.copy_from_slice(&hex(
            "e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c",
        ));
        let shared = x25519_dalek::x25519(scalar, point);
        check(
            "x25519",
            &shared,
            "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552",
        );
    }

    // P-256's base point, from FIPS 186-4's D.1.2.3 curve parameters: the scalar 1 times the
    // generator has to be the generator, in uncompressed SEC 1 form (`04` then x then y). It is a
    // small claim and it exercises the whole field-arithmetic backend to make it.
    {
        use p256::elliptic_curve::sec1::ToEncodedPoint;
        let one = p256::SecretKey::from_slice(&{
            let mut b = [0u8; 32];
            b[31] = 1;
            b
        })
        .expect("1 is a valid P-256 scalar");
        check(
            "p256 generator",
            one.public_key().to_encoded_point(false).as_bytes(),
            "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296\
             4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
        );
    }

    // ---------------------------------------------------------------------------------------
    // The randomness, which is the one thing here with no published vector and the one thing that
    // is genuinely new on this system.
    // ---------------------------------------------------------------------------------------
    // `rand_core` 0.6's `OsRng` goes to `getrandom` **0.2**, whose `__getrandom_custom` hook
    // `entropy_backend` gained at this milestone, over `std::random::SystemRng`, over the entropy
    // service's one endpoint, to virtio-rng. Two draws differing is the same claim milestone 56 (secrets, credentials, and the entropy to make them safe)
    // made for `std::random` and is as much as a test can assert about an RNG without a vector:
    // it proves the bytes are not a constant, which is what a silently-stubbed backend would give.
    // A draw with no entropy capability granted panics rather than weakening, deliberately.
    {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        rand_core::OsRng.fill_bytes(&mut a);
        rand_core::OsRng.fill_bytes(&mut b);
        assert_ne!(a, b, "two entropy draws were identical: this is not an RNG");
        assert!(
            a.iter().any(|&x| x != 0),
            "an entropy draw was all zeroes: a stub, not a source"
        );
        println!("entropy 0.2 ok");
    }


    // ---------------------------------------------------------------------------------------
    // **Two real certificate chains, from the two hosts that made the case for taking `rsa`.**
    // calef ruled "take rsa" on 2026-09-20 because a client that cannot verify these cannot
    // download anything: DECISIONS §196 (nife carries TLS: `rustls` for the protocol, and a crypto
    // provider we make work) chose GitHub, `github.com` is ECDSA throughout, and the hosts that
    // actually serve the bytes are RSA. So the proof is the hosts themselves rather than a
    // synthetic key.
    // ---------------------------------------------------------------------------------------
    verify_chain(
        "chain objects.githubusercontent.com",
        include_bytes!("../fixtures/objects-githubusercontent-leaf.der"),
        include_bytes!("../fixtures/objects-githubusercontent-intermediate.der"),
        include_bytes!("../fixtures/objects-githubusercontent-root.der"),
        "objects.githubusercontent.com",
    );
    verify_chain(
        "chain ghcr.io",
        include_bytes!("../fixtures/ghcr-leaf.der"),
        include_bytes!("../fixtures/ghcr-intermediate.der"),
        include_bytes!("../fixtures/ghcr-root.der"),
        "ghcr.io",
    );

    // **A chain has to fail when it should.** A positive result alone cannot tell a verifier from
    // a function that returns `Ok`, and that is not a hypothetical: an RSA verifier that ignored
    // the padding would pass every line above.
    {
        let mut tampered = include_bytes!("../fixtures/ghcr-leaf.der").to_vec();
        // The signature is the last structure in a certificate, so the final byte is inside it.
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(
            chain_result(
                &tampered,
                include_bytes!("../fixtures/ghcr-intermediate.der"),
                include_bytes!("../fixtures/ghcr-root.der"),
                "ghcr.io",
            )
            .is_err(),
            "a chain with one flipped signature bit was accepted"
        );
        // And the wrong name must fail too, which is a different check in a different place.
        assert!(
            chain_result(
                include_bytes!("../fixtures/ghcr-leaf.der"),
                include_bytes!("../fixtures/ghcr-intermediate.der"),
                include_bytes!("../fixtures/ghcr-root.der"),
                "objects.githubusercontent.com",
            )
            .is_err(),
            "ghcr.io's certificate was accepted for another host's name"
        );
        println!("chain refusals ok");
    }

    // ---------------------------------------------------------------------------------------
    // And the provider itself, which is the object §196's ruling is about and which this tree now
    // assembles rather than depends on. Building it forces every cipher-suite table, key-exchange
    // group and signature algorithm to be linked and constructed.
    // ---------------------------------------------------------------------------------------
    {
        let provider = cryptography_provider::provider();
        assert_eq!(
            provider.cipher_suites.len(),
            3,
            "the provider should offer exactly TLS 1.3's three AEAD suites"
        );
        assert_eq!(
            provider.kx_groups.len(),
            2,
            "the provider should offer X25519 and P-256"
        );
        for suite in &provider.cipher_suites {
            assert_eq!(
                suite.version().version,
                rustls::ProtocolVersion::TLSv1_3,
                "this provider is TLS 1.3 only, and a suite says otherwise"
            );
        }
        // X25519 first is a real claim rather than an accident: it is the key share a client sends
        // in its first flight, so the order decides whether the common case costs one round trip
        // or two.
        assert_eq!(
            provider.kx_groups[0].name(),
            rustls::NamedGroup::X25519,
            "X25519 should be the default key share"
        );
        assert_eq!(
            provider.signature_verification_algorithms.all.len(),
            9,
            "the provider should verify three ECDSA and Ed25519 algorithms plus RSA PKCS#1 v1.5 \
             and PSS at three digest sizes"
        );
        // **The key provider must refuse**, and asserting it is the point rather than tidiness: a
        // provider that silently accepted a private key would be claiming a client-authentication
        // path this one does not have.
        // Built by naming the variant rather than through `try_from`, which sniffs the encoding
        // and rejects bytes that are not a key at all. The point here is that a **well-formed**
        // request is refused, not that a malformed one is.
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(vec![0u8; 32].into());
        assert!(
            provider.key_provider.load_private_key(key).is_err(),
            "this provider should load no private keys"
        );
        println!(
            "provider ok {} suites {} groups {} signature algorithms",
            provider.cipher_suites.len(),
            provider.kx_groups.len(),
            provider.signature_verification_algorithms.all.len(),
        );
    }

    println!("exiting through process::exit");
    std::process::exit(0);
}
