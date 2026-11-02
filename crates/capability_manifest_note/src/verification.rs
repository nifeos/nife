//! **Machine-checked proofs (Kani) of the manifest descriptor**: whatever `decode` accepts,
//! `encode` spells back byte for byte, and `decode` never panics. Behind `#[cfg(kani)]`, so only
//! `cargo kani` (script/verify) compiles them. The `INTERFACES` note's own harnesses stay in
//! `lib.rs` beside its code. See notes/verification.md.
//!
//! Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats,
//! after the `verification` module every other Kani crate here keeps out of line.

use super::*;

/// **Whatever `decode` accepts, `encode` spells back byte for byte**, over every descriptor the
/// solver can choose. This is the "one manifest, one encoding" claim: no two byte strings
/// decode to one manifest, so no two readers can disagree about which bytes meant what. The
/// unwind bound is the 56-byte comparison, plus one.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.a_decoded_manifest_encodes_back_to_its_bytes.patch`
#[kani::proof]
#[kani::unwind(57)]
fn a_decoded_manifest_encodes_back_to_its_bytes() {
    let d: [u8; DESCRIPTOR_LEN] = kani::any();
    if let Ok(m) = decode(&d) {
        assert!(encode(&m) == d);
    }
}

/// **`decode` never panics, at any length up to one past the layout.** The bytes come from a
/// file anyone could have written.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.decode_never_panics.patch`
#[kani::proof]
#[kani::unwind(18)]
fn decode_never_panics() {
    let d: [u8; DESCRIPTOR_LEN + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= d.len());
    let _ = decode(&d[..len]);
}
