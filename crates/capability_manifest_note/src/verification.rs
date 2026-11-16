//! **Machine-checked proofs (Kani) of the manifest descriptor**: whatever `decode` accepts,
//! `encode` spells back byte for byte; whatever `encode` builds, `decode` reads back as the same
//! manifest; and `decode`, `decode_verbs` and `decode_subtree_grants` never panic. Behind
//! `#[cfg(kani)]`, so only `cargo kani` compiles them.
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
    let d: [u8; DESCRIPTOR_BYTES] = kani::any();
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
    let d: [u8; DESCRIPTOR_BYTES + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= d.len());
    let _ = decode(&d[..len]);
}

/// **`decode` never panics through a version 3 descriptor with one three-byte zone**, at any
/// length up to its 62 bytes, and neither do `zones` and `asks_service` after it. That length walks
/// both zone checks, the service byte and the exact-length check, which the 57-byte harness above
/// stops short of.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.decode_never_panics_through_a_serviced_descriptor.patch`
#[kani::proof]
#[kani::unwind(18)]
fn decode_never_panics_through_a_serviced_descriptor() {
    let d: [u8; DESCRIPTOR_BYTES + 1 + 1 + 3 + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= d.len());
    let d = &d[..len];
    let _ = decode(d);
    let _ = zones(d);
    let _ = asks_service(d);
}

/// **`decode_verbs` never panics on a table of up to two verbs, and `grants_for` finds every verb
/// it decoded** with the grants the iterator gives. The binary search is correct only on a table
/// sorted strictly, which is what `decode_verbs` refuses to let through otherwise.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.a_decoded_verb_table_finds_each_verb.patch`
#[kani::proof]
#[kani::unwind(18)]
fn a_decoded_verb_table_finds_each_verb() {
    let d: [u8; verbs_descriptor_bytes(2)] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= d.len());
    if let Ok(verbs) = decode_verbs(&d[..len]) {
        for (word, grants) in verbs.iter() {
            assert!(verbs.grants_for(word) == Some(grants));
        }
    }
}

/// **`decode_subtree_grants` never panics**, at any length up to one past its 8 bytes.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.decode_subtree_grants_never_panics.patch`
#[kani::proof]
#[kani::unwind(10)]
fn decode_subtree_grants_never_panics() {
    let d: [u8; SUBTREE_GRANTS_BYTES + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= d.len());
    let _ = decode_subtree_grants(&d[..len]);
}

/// Any manifest a version 1 descriptor can spell, chosen field by field: letters `ShortOptions`
/// accepts, a subtree option that is one of them, the diagnostics slot, a memory range with its
/// minimum no greater than its maximum, and none of the grants only the owner's console gives.
fn any_manifest() -> Manifest {
    let letters: [u8; MAXIMUM_DECLARED_SHORT_OPTIONS] = kani::any();
    let count: usize = kani::any();
    kani::assume(count <= MAXIMUM_DECLARED_SHORT_OPTIONS);
    let short_options = ShortOptions::try_new(&letters[..count]);
    kani::assume(short_options.is_some());
    let short_options = short_options.unwrap();
    let program_argument = match kani::any::<u8>() % 5 {
        0 => ProgramArgumentDeclaration::Forbidden,
        1 => ProgramArgumentDeclaration::Required,
        2 => ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        3 => ProgramArgumentDeclaration::Words(WordGrant::ReadWrite),
        _ => ProgramArgumentDeclaration::Words(WordGrant::Create),
    };
    let memory_grant = if kani::any() {
        let (minimum, maximum): (u64, u64) = (kani::any(), kani::any());
        kani::assume(minimum <= maximum);
        MemoryGrantDeclaration::Required { minimum, maximum }
    } else {
        MemoryGrantDeclaration::Forbidden
    };
    let file = if kani::any() {
        FileGrantDeclaration::Required {
            writable: kani::any(),
        }
    } else {
        FileGrantDeclaration::Forbidden
    };
    let directory = if kani::any() {
        let subtree_option = if kani::any() {
            let pick: usize = kani::any();
            kani::assume(pick < count);
            Some(letters[pick])
        } else {
            None
        };
        DirectoryGrantDeclaration::Required { subtree_option }
    } else {
        DirectoryGrantDeclaration::Forbidden
    };
    let output = match kani::any::<u8>() % 4 {
        0 => OutputDeclaration::Silent,
        1 => OutputDeclaration::Words,
        2 => OutputDeclaration::Bytes,
        _ => OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
    };
    let input = if kani::any() {
        InputDeclaration::Required {
            writes_while_reading: kani::any(),
        }
    } else {
        InputDeclaration::Forbidden
    };
    Manifest {
        program_argument,
        memory_grant,
        file,
        directory,
        short_options,
        output,
        input,
        reports: kani::any(),
        interruptible: kani::any(),
        clock: kani::any(),
        domain: kani::any(),
        configuration: kani::any(),
        entropy: kani::any(),
        network: kani::any(),
        machine: kani::any(),
        share: kani::any(),
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        startup: if kani::any() {
            StartupContract::Std
        } else {
            StartupContract::Native
        },
    }
}

/// **Whatever `encode` builds, `decode` reads back as the same manifest**, over every manifest a
/// descriptor can spell. The other direction of the harness above: together they are the claim
/// that one manifest has exactly one encoding, so a note the compiler put in a program is a note
/// the reader accepts.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.an_encoded_manifest_decodes_to_itself.patch`
#[kani::proof]
#[kani::unwind(18)]
fn an_encoded_manifest_decodes_to_itself() {
    let m = any_manifest();
    assert!(decode(&encode(&m)) == Ok(m));
}

/// **`encode` refuses a memory range whose minimum is past its maximum**, which `decode` would
/// refuse, so a program cannot build a note its reader rejects. A panic in `encode` is a compile
/// error in the constant `carry!` evaluates.
///
/// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.an_inverted_memory_grant_does_not_encode.patch`
#[kani::proof]
#[kani::should_panic]
fn an_inverted_memory_grant_does_not_encode() {
    let (minimum, maximum): (u64, u64) = (kani::any(), kani::any());
    kani::assume(minimum > maximum);
    let m = Manifest {
        memory_grant: MemoryGrantDeclaration::Required { minimum, maximum },
        ..PLAIN
    };
    let _ = encode(&m);
}

/// A manifest that asks for nothing, written out so the harness depends on no program's.
const PLAIN: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Forbidden,
    file: FileGrantDeclaration::Forbidden,
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: ShortOptions::new(b""),
    output: OutputDeclaration::Silent,
    input: InputDeclaration::Forbidden,
    reports: false,
    interruptible: false,
    clock: false,
    domain: false,
    configuration: false,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    startup: StartupContract::Native,
};
